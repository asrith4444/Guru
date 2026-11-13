//! Guru home directory, settings and small JSON helpers.
//!
//! Layout:
//!   $GURU_HOME (default ~/.guru)/
//!     config.json
//!     courses/<slug>/          one downloaded course package (see course.rs)

use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Path to the codex binary (default: `codex` on PATH; env GURU_CODEX_BIN wins).
    pub codex_bin: Option<String>,
    /// Model override passed to Codex (default: Codex's own default).
    pub model: Option<String>,
    /// Reasoning effort for course generation (e.g. "medium", "high").
    pub reasoning_effort: Option<String>,
    /// Allow Codex to search the live web while building courses.
    pub web_search: bool,
    /// Mine Stack Exchange for real-world problems (requires curl).
    pub stackexchange: bool,
    /// Optional Stack Exchange API key (raises the daily quota).
    pub stackexchange_key: Option<String>,
    /// How many Codex workers build a course at once (None = decide from free RAM).
    pub parallel_jobs: Option<u32>,
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
            parallel_jobs: None,
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
pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(value)?)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

impl Config {
    pub fn path() -> PathBuf {
        guru_home().join("config.json")
    }

    pub fn load() -> Self {
        read_json(&Self::path()).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        write_json(&Self::path(), self)
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
    let out: String = out.trim_end_matches('-').chars().take(40).collect();
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() {
        "course".to_string()
    } else {
        out
    }
}

/// Pick a slug that does not collide with an existing directory under `root`.
pub fn unique_slug(root: &Path, topic: &str) -> String {
    let base = slugify(topic);
    if !root.join(&base).exists() {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|s| !root.join(s).exists())
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
    fn config_roundtrip_with_defaults() {
        let c: Config = serde_json::from_str(r#"{"model":"x"}"#).unwrap();
        assert_eq!(c.model.as_deref(), Some("x"));
        assert!(c.web_search && c.stackexchange);
        assert_eq!(c.parallel_jobs, None);
    }
}
