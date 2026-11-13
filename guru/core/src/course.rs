//! The downloadable course package. Once generated it is a self-contained
//! folder that plays fully offline, like a downloaded video course but made of
//! reading, quizzes, flashcards and interactive simulations.
//!
//!   courses/<slug>/
//!     course.json            manifest: plan, skills, modules, status
//!     modules/<id>.json      lesson (Markdown), key points, flashcards, quiz
//!     sims/<id>.json         simulations (see sim.rs)
//!     progress.json          what the learner has done, mastery per skill
//!     cards.json             spaced-repetition state
//!     research/              raw sources kept for attribution

use std::path::Path;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::config::guru_home;
use crate::config::read_json;
use crate::config::write_json;
use crate::sim::Simulation;
use crate::srs::Card;

pub const FORMAT_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CourseStatus {
    #[default]
    Planning,
    Downloading,
    Ready,
    /// Generation stopped with an error; it can be resumed.
    Failed,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub why: String,
    /// 1..=10, how much real-world performance this skill buys.
    pub weight: u8,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SimPlan {
    pub id: String,
    pub title: String,
    /// incident | tool | guided | roleplay
    pub kind: String,
    pub tool: String,
    pub premise: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ModuleInfo {
    pub id: String,
    pub title: String,
    pub minutes: u32,
    pub summary: String,
    pub objectives: Vec<String>,
    pub skill_ids: Vec<String>,
    pub sims: Vec<SimPlan>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ResearchHints {
    /// Stack Exchange site slug (e.g. "stackoverflow", "serverfault"), or "".
    pub stackexchange_site: String,
    pub tags: Vec<String>,
    pub queries: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Course {
    pub format: u32,
    pub slug: String,
    pub title: String,
    pub topic: String,
    pub goal: String,
    /// 0..=100 on the beginner → intermediate → advanced bar.
    pub level: u8,
    pub level_label: String,
    pub hours: f32,
    pub summary: String,
    pub target_performance: String,
    pub out_of_scope: Vec<String>,
    pub skills: Vec<Skill>,
    pub modules: Vec<ModuleInfo>,
    /// Final, multi-step scenario that integrates the whole course.
    pub capstone: SimPlan,
    pub research: ResearchHints,
    pub status: CourseStatus,
    pub status_detail: String,
    pub created_unix: u64,
    /// Codex version that generated the package.
    pub generator: String,
}

impl Course {
    /// Every simulation planned in the course, in study order (capstone last).
    pub fn all_sims(&self) -> Vec<&SimPlan> {
        let mut v: Vec<&SimPlan> = self.modules.iter().flat_map(|m| m.sims.iter()).collect();
        if !self.capstone.id.is_empty() {
            v.push(&self.capstone);
        }
        v
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Question {
    pub id: String,
    /// single | multi | text
    pub kind: String,
    pub prompt: String,
    pub options: Vec<String>,
    pub correct: Vec<usize>,
    /// For text questions: case-insensitive regexes; any match = correct.
    pub accept: Vec<String>,
    pub explanation: String,
    pub skill_id: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct CardSpec {
    pub skill_id: String,
    pub front: String,
    pub back: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Module {
    pub id: String,
    pub title: String,
    /// Markdown.
    pub lesson: String,
    pub key_points: Vec<String>,
    pub cards: Vec<CardSpec>,
    pub quiz: Vec<Question>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SimResult {
    pub completed: bool,
    pub objectives_done: u32,
    pub objectives_total: u32,
    pub hints_used: u32,
    pub seconds: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Progress {
    pub lessons_read: Vec<String>,
    /// module id -> best quiz score 0..1
    pub quiz_scores: Vec<(String, f32)>,
    pub sims: Vec<(String, SimResult)>,
    /// skill id -> mastery 0..1
    pub mastery: Vec<(String, f32)>,
    pub seconds_studied: u64,
    pub last_module: String,
}

fn upsert<T: Clone>(v: &mut Vec<(String, T)>, key: &str, val: T) {
    match v.iter_mut().find(|(k, _)| k == key) {
        Some(e) => e.1 = val,
        None => v.push((key.to_string(), val)),
    }
}

pub fn get<'a, T>(v: &'a [(String, T)], key: &str) -> Option<&'a T> {
    v.iter().find(|(k, _)| k == key).map(|(_, t)| t)
}

impl Progress {
    /// Blend new evidence into a skill's mastery (new evidence weighs 60%).
    pub fn evidence(&mut self, skill: &str, score: f32) {
        let score = score.clamp(0.0, 1.0);
        let new = match get(&self.mastery, skill) {
            Some(old) => 0.4 * old + 0.6 * score,
            None => score,
        };
        upsert(&mut self.mastery, skill, new);
    }

    pub fn record_quiz(&mut self, module: &str, per_skill: &[(String, f32)], score: f32) {
        let best = get(&self.quiz_scores, module)
            .copied()
            .unwrap_or(0.0)
            .max(score);
        upsert(&mut self.quiz_scores, module, best);
        for (skill, s) in per_skill {
            self.evidence(skill, *s);
        }
    }

    pub fn record_sim(&mut self, sim_id: &str, skills: &[String], r: SimResult) {
        let ratio = if r.objectives_total == 0 {
            0.0
        } else {
            r.objectives_done as f32 / r.objectives_total as f32
        };
        // Using hints is fine, but it is weaker evidence of independent skill.
        let score = (ratio - 0.1 * r.hints_used as f32).clamp(0.0, 1.0);
        for s in skills {
            self.evidence(s, score);
        }
        let keep_best = match get(&self.sims, sim_id) {
            Some(old) if old.objectives_done > r.objectives_done => old.clone(),
            _ => r,
        };
        upsert(&mut self.sims, sim_id, keep_best);
    }

    pub fn mark_read(&mut self, module: &str) {
        if !self.lessons_read.iter().any(|m| m == module) {
            self.lessons_read.push(module.to_string());
        }
        self.last_module = module.to_string();
    }
}

/// Weighted mastery across the skill map.
pub fn overall_mastery(course: &Course, progress: &Progress) -> f32 {
    let total: f32 = course
        .skills
        .iter()
        .map(|s| f32::from(s.weight.max(1)))
        .sum();
    if total == 0.0 {
        return 0.0;
    }
    course
        .skills
        .iter()
        .map(|s| f32::from(s.weight.max(1)) * get(&progress.mastery, &s.id).copied().unwrap_or(0.0))
        .sum::<f32>()
        / total
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Deck {
    pub cards: Vec<Card>,
}

/// One course on disk.
#[derive(Debug, Clone)]
pub struct Package {
    pub dir: PathBuf,
}

fn safe_id(id: &str) -> Result<&str> {
    if id.is_empty()
        || id.len() > 80
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("invalid id '{id}'");
    }
    Ok(id)
}

impl Package {
    pub fn root() -> PathBuf {
        guru_home().join("courses")
    }

    pub fn open(slug: &str) -> Result<Self> {
        let dir = Self::root().join(safe_id(slug)?);
        if !dir.join("course.json").exists() {
            bail!("course '{slug}' not found");
        }
        Ok(Self { dir })
    }

    pub fn create(slug: &str) -> Result<Self> {
        let dir = Self::root().join(safe_id(slug)?);
        for sub in ["modules", "sims", "research", ".guru"] {
            std::fs::create_dir_all(dir.join(sub))?;
        }
        Ok(Self { dir })
    }

    pub fn list() -> Vec<Self> {
        let mut v: Vec<Self> = std::fs::read_dir(Self::root())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.join("course.json").exists())
                    .map(|dir| Self { dir })
                    .collect()
            })
            .unwrap_or_default();
        v.sort_by_key(|p| p.dir.clone());
        v
    }

    pub fn course(&self) -> Result<Course> {
        read_json(&self.dir.join("course.json"))
    }
    pub fn save_course(&self, c: &Course) -> Result<()> {
        write_json(&self.dir.join("course.json"), c)
    }

    pub fn module_path(&self, id: &str) -> Result<PathBuf> {
        Ok(self
            .dir
            .join("modules")
            .join(format!("{}.json", safe_id(id)?)))
    }
    pub fn module(&self, id: &str) -> Result<Module> {
        read_json(&self.module_path(id)?)
    }
    pub fn save_module(&self, m: &Module) -> Result<()> {
        write_json(&self.module_path(&m.id)?, m)
    }

    pub fn sim_path(&self, id: &str) -> Result<PathBuf> {
        Ok(self.dir.join("sims").join(format!("{}.json", safe_id(id)?)))
    }
    pub fn sim(&self, id: &str) -> Result<Simulation> {
        read_json(&self.sim_path(id)?)
    }
    pub fn save_sim(&self, s: &Simulation) -> Result<()> {
        write_json(&self.sim_path(&s.id)?, s)
    }

    pub fn progress(&self) -> Progress {
        read_json(&self.dir.join("progress.json")).unwrap_or_default()
    }
    pub fn save_progress(&self, p: &Progress) -> Result<()> {
        write_json(&self.dir.join("progress.json"), p)
    }

    pub fn deck(&self) -> Deck {
        read_json(&self.dir.join("cards.json")).unwrap_or_default()
    }
    pub fn save_deck(&self, d: &Deck) -> Result<()> {
        write_json(&self.dir.join("cards.json"), d)
    }

    pub fn scratch(&self) -> PathBuf {
        self.dir.join(".guru")
    }

    pub fn delete(self) -> Result<()> {
        // Refuse to delete anything that is not a course directory under the root.
        if !self.dir.starts_with(Self::root()) || !self.dir.join("course.json").exists() {
            bail!("refusing to delete {}", self.dir.display());
        }
        std::fs::remove_dir_all(&self.dir)?;
        Ok(())
    }

    pub fn size_bytes(&self) -> u64 {
        dir_size(&self.dir)
    }
}

fn dir_size(p: &Path) -> u64 {
    let Ok(rd) = std::fs::read_dir(p) else {
        return 0;
    };
    rd.filter_map(|e| e.ok())
        .map(|e| match e.file_type() {
            Ok(ft) if ft.is_dir() && !ft.is_symlink() => dir_size(&e.path()),
            Ok(_) => e.metadata().map(|m| m.len()).unwrap_or(0),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_path_safe() {
        assert!(safe_id("m1-intro_x").is_ok());
        assert!(safe_id("../etc").is_err());
        assert!(safe_id("").is_err());
        assert!(safe_id("a/b").is_err());
    }

    #[test]
    fn mastery_from_quiz_and_sims() {
        let mut p = Progress::default();
        p.record_quiz("m1", &[("a".into(), 1.0)], 1.0);
        p.record_quiz("m1", &[("a".into(), 0.0)], 0.2);
        assert!((get(&p.mastery, "a").unwrap() - 0.4).abs() < 1e-6);
        assert_eq!(get(&p.quiz_scores, "m1"), Some(&1.0), "best score kept");
        p.record_sim(
            "s1",
            &["b".into()],
            SimResult {
                completed: true,
                objectives_done: 3,
                objectives_total: 3,
                hints_used: 2,
                seconds: 60,
            },
        );
        assert!((get(&p.mastery, "b").unwrap() - 0.8).abs() < 1e-6);
        p.record_sim(
            "s1",
            &[],
            SimResult {
                objectives_done: 1,
                objectives_total: 3,
                ..Default::default()
            },
        );
        assert_eq!(
            get(&p.sims, "s1").unwrap().objectives_done,
            3,
            "best attempt kept"
        );
    }

    #[test]
    fn overall_mastery_is_weighted() {
        let course = Course {
            skills: vec![
                Skill {
                    id: "a".into(),
                    weight: 9,
                    ..Default::default()
                },
                Skill {
                    id: "b".into(),
                    weight: 1,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut p = Progress::default();
        p.evidence("b", 1.0);
        assert!((overall_mastery(&course, &p) - 0.1).abs() < 1e-6);
    }
}
