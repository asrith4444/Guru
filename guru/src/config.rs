//! Guru home directory, user configuration and course storage.
//!
//! Layout:
//!   $GURU_HOME (default ~/.guru)/
//!     config.json
//!     active                      slug of the current course
//!     courses/<slug>/
//!       course.json               plan + learner model
//!       cards.json                spaced-retrieval deck
//!       issues.json               real-world problem bank
//!       journal.jsonl             one line per finished hour
//!       progress.json             where you are inside the current hour
//!       AGENTS.md                 tutor instructions Codex loads automatically
//!       lessons/hour-N.md         reading
//!       lab/hour-N/               practice environment
//!       capstone/                 the project that grows every hour
//!       research/                 raw sources (with attribution)

use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;

use crate::model::CardDeck;
use crate::model::Course;
use crate::model::HourPack;
use crate::model::Issue;
use crate::model::JournalEntry;
use crate::model::Progress;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Path to the codex binary (default: `codex` on PATH; env GURU_CODEX_BIN wins).
    pub codex_bin: Option<String>,
    /// Model override passed to Codex (default: Codex's own default).
    pub model: Option<String>,
    /// Reasoning effort passed to Codex (e.g. "medium", "high").
    pub reasoning_effort: Option<String>,
    /// Allow Codex to search the live web while planning/researching.
    pub web_search: bool,
    /// Fetch real questions from the Stack Exchange API (requires curl).
    pub stackexchange: bool,
    /// Optional Stack Exchange API key (raises the daily quota).
    pub stackexchange_key: Option<String>,
    /// Default number of sessions per course.
    pub hours: u32,
    /// Default minutes per session.
    pub minutes_per_hour: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            codex_bin: None,
            model: None,
            reasoning_effort: None,
            web_search: true,
            stackexchange: true,
            stackexchange_key: None,
            hours: 7,
            minutes_per_hour: 60,
        }
    }
}

pub fn guru_home() -> PathBuf {
    if let Some(h) = std::env::var_os("GURU_HOME") {
        return PathBuf::from(h);
    }
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".guru")
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
}

/// Atomic write: a crash mid-save never corrupts learner data.
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

impl Config {
    pub fn load() -> Self {
        let path = guru_home().join("config.json");
        if path.exists() {
            match read_json(&path) {
                Ok(c) => return c,
                Err(e) => eprintln!("warning: ignoring invalid {}: {e:#}", path.display()),
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<()> {
        write_json(&guru_home().join("config.json"), self)
    }

    pub fn set(&mut self, key: &str, value: &str) -> Result<()> {
        let opt = |v: &str| (!v.is_empty() && v != "none").then(|| v.to_string());
        let boolean = |v: &str| -> Result<bool> {
            match v {
                "true" | "on" | "yes" | "1" => Ok(true),
                "false" | "off" | "no" | "0" => Ok(false),
                _ => bail!("expected true/false"),
            }
        };
        match key {
            "codex_bin" => self.codex_bin = opt(value),
            "model" => self.model = opt(value),
            "reasoning_effort" => self.reasoning_effort = opt(value),
            "web_search" => self.web_search = boolean(value)?,
            "stackexchange" => self.stackexchange = boolean(value)?,
            "stackexchange_key" => self.stackexchange_key = opt(value),
            "hours" => self.hours = value.parse::<u32>().ok().filter(|h| (1..=40).contains(h)).context("hours must be 1-40")?,
            "minutes_per_hour" => {
                self.minutes_per_hour = value
                    .parse::<u32>()
                    .ok()
                    .filter(|m| (20..=180).contains(m))
                    .context("minutes_per_hour must be 20-180")?
            }
            _ => bail!("unknown key '{key}'. Keys: codex_bin, model, reasoning_effort, web_search, stackexchange, stackexchange_key, hours, minutes_per_hour"),
        }
        Ok(())
    }
}

/// A course on disk.
pub struct Store {
    pub dir: PathBuf,
}

impl Store {
    pub fn courses_dir() -> PathBuf {
        guru_home().join("courses")
    }

    pub fn open(slug: &str) -> Result<Self> {
        let dir = Self::courses_dir().join(slug);
        if !dir.join("course.json").exists() {
            bail!("no course named '{slug}'. Run `guru list` to see your courses.");
        }
        Ok(Self { dir })
    }

    pub fn create(slug: &str) -> Result<Self> {
        let dir = Self::courses_dir().join(slug);
        for sub in ["lessons", "lab", "capstone", "research", ".guru"] {
            std::fs::create_dir_all(dir.join(sub))?;
        }
        Ok(Self { dir })
    }

    pub fn active() -> Result<Self> {
        let slug = std::fs::read_to_string(guru_home().join("active"))
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        match slug {
            Some(slug) => Self::open(&slug),
            None => bail!("no active course. Start one with `guru learn \"<topic>\"`."),
        }
    }

    pub fn set_active(slug: &str) -> Result<()> {
        std::fs::create_dir_all(guru_home())?;
        std::fs::write(guru_home().join("active"), slug)?;
        Ok(())
    }

    pub fn list() -> Vec<String> {
        let mut out: Vec<String> = std::fs::read_dir(Self::courses_dir())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| e.path().join("course.json").exists())
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default();
        out.sort();
        out
    }

    pub fn course(&self) -> Result<Course> {
        read_json(&self.dir.join("course.json"))
    }
    pub fn save_course(&self, c: &Course) -> Result<()> {
        write_json(&self.dir.join("course.json"), c)
    }

    pub fn deck(&self) -> CardDeck {
        read_json(&self.dir.join("cards.json")).unwrap_or_default()
    }
    pub fn save_deck(&self, d: &CardDeck) -> Result<()> {
        write_json(&self.dir.join("cards.json"), d)
    }

    pub fn issues(&self) -> Vec<Issue> {
        read_json(&self.dir.join("issues.json")).unwrap_or_default()
    }
    pub fn save_issues(&self, issues: &[Issue]) -> Result<()> {
        write_json(&self.dir.join("issues.json"), &issues)
    }

    pub fn progress(&self) -> Option<Progress> {
        read_json(&self.dir.join("progress.json")).ok()
    }
    pub fn save_progress(&self, p: &Progress) -> Result<()> {
        write_json(&self.dir.join("progress.json"), p)
    }
    pub fn clear_progress(&self) {
        let _ = std::fs::remove_file(self.dir.join("progress.json"));
    }

    pub fn pack_path(&self, hour: u32) -> PathBuf {
        self.dir.join(".guru").join(format!("hour-{hour}.json"))
    }
    pub fn pack(&self, hour: u32) -> Option<HourPack> {
        read_json(&self.pack_path(hour)).ok()
    }
    pub fn save_pack(&self, pack: &HourPack) -> Result<()> {
        write_json(&self.pack_path(pack.hour), pack)
    }

    pub fn lab_dir(&self, hour: u32) -> PathBuf {
        self.dir.join("lab").join(format!("hour-{hour}"))
    }

    pub fn journal(&self) -> Vec<JournalEntry> {
        std::fs::read_to_string(self.dir.join("journal.jsonl"))
            .map(|t| {
                t.lines()
                    .filter_map(|l| serde_json::from_str(l).ok())
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn append_journal(&self, e: &JournalEntry) -> Result<()> {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.dir.join("journal.jsonl"))?;
        writeln!(f, "{}", serde_json::to_string(e)?)?;
        Ok(())
    }
}

/// Lowercase ASCII slug: "Spring Boot 3" -> "spring-boot-3".
pub fn slugify(topic: &str) -> String {
    let mut out = String::new();
    for ch in topic.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    let out: String = out.chars().take(40).collect();
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "course".to_string()
    } else {
        out
    }
}

/// Pick a slug that does not collide with an existing course.
pub fn unique_slug(topic: &str) -> String {
    let base = slugify(topic);
    let dir = Store::courses_dir();
    if !dir.join(&base).exists() {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|s| !dir.join(s).exists())
        .unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slugify("Spring Boot 3"), "spring-boot-3");
        assert_eq!(slugify("  C++ / Rust!! "), "c-rust");
        assert_eq!(slugify("日本語"), "course");
        assert_eq!(slugify(&"a".repeat(100)).len(), 40);
    }

    #[test]
    fn config_set_validates() {
        let mut c = Config::default();
        c.set("model", "gpt-x").unwrap();
        assert_eq!(c.model.as_deref(), Some("gpt-x"));
        c.set("model", "none").unwrap();
        assert!(c.model.is_none());
        c.set("web_search", "off").unwrap();
        assert!(!c.web_search);
        assert!(c.set("hours", "0").is_err());
        assert!(c.set("minutes_per_hour", "45").is_ok());
        assert!(c.set("nope", "1").is_err());
    }
}
