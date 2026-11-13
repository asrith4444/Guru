//! Persistent data model. Everything Guru knows about a learner lives in plain
//! JSON files inside the course directory, so it is inspectable, diffable and
//! cheap to load (no database, no daemon).

use serde::Deserialize;
use serde::Serialize;

use crate::srs::Card;

/// The kind of practice environment ("arena") a topic needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Arena {
    /// Real code in a local project with checks (languages, frameworks, libraries).
    CodeLab,
    /// Broken or evolving systems to operate and repair (DevOps, infra, databases, networking).
    OpsLab,
    /// Live conversation with an AI partner (spoken languages, negotiation, interviews, sales).
    Roleplay,
    /// Graded problems with worked solutions (math, physics, finance, statistics).
    ProblemSet,
    /// Timed physical/creative drills with self-assessment rubrics (instrument, drawing, typing, sport).
    DrillProtocol,
    /// Realistic scenarios to analyse and decide (management, law, medicine, strategy, history).
    CaseStudy,
}

impl Arena {
    pub fn as_str(self) -> &'static str {
        match self {
            Arena::CodeLab => "code_lab",
            Arena::OpsLab => "ops_lab",
            Arena::Roleplay => "roleplay",
            Arena::ProblemSet => "problem_set",
            Arena::DrillProtocol => "drill_protocol",
            Arena::CaseStudy => "case_study",
        }
    }

    pub const ALL: [&'static str; 6] = [
        "code_lab",
        "ops_lab",
        "roleplay",
        "problem_set",
        "drill_protocol",
        "case_study",
    ];
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub why_it_matters: String,
    /// 1..=10. How much real-world performance this sub-skill buys (Pareto weight).
    pub pareto_weight: u8,
    pub prerequisites: Vec<String>,
    /// Hour (1-based) in which this skill is first taught.
    pub hour: u32,
    /// Current mastery estimate in [0, 1].
    #[serde(default)]
    pub mastery: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourPlan {
    pub hour: u32,
    pub title: String,
    pub objectives: Vec<String>,
    pub skill_ids: Vec<String>,
    pub capstone_slice: String,
    #[serde(default)]
    pub status: HourStatus,
    #[serde(default)]
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HourStatus {
    #[default]
    Planned,
    InProgress,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capstone {
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagnosticQuestion {
    pub id: String,
    pub question: String,
    pub skill_id: String,
    pub answer_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResearchHints {
    /// Stack Exchange site slug (e.g. "stackoverflow", "serverfault", "spanish"), or "" for none.
    pub stackexchange_site: String,
    pub tags: Vec<String>,
    pub queries: Vec<String>,
}

/// A real-world problem people repeatedly hit with this topic. Mined from
/// Stack Exchange / docs / issue trackers and turned into break-fix practice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub symptom: String,
    pub root_cause: String,
    pub fix: String,
    pub skill_id: String,
    pub source_url: String,
    #[serde(default)]
    pub used: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Course {
    pub slug: String,
    pub topic: String,
    pub title: String,
    pub goal: String,
    pub prior_experience: String,
    pub domain: String,
    pub arena: Arena,
    pub target_performance: String,
    pub out_of_scope: Vec<String>,
    pub level_estimate: String,
    pub skills: Vec<Skill>,
    pub hours: Vec<HourPlan>,
    pub capstone: Capstone,
    pub research: ResearchHints,
    pub lab_constraints: String,
    pub minutes_per_hour: u32,
    pub created_day: i64,
    /// Running notes the planner and assessor leave for the next session.
    #[serde(default)]
    pub coach_notes: Vec<String>,
    /// Misconceptions detected so far (they get explicitly repaired).
    #[serde(default)]
    pub misconceptions: Vec<String>,
    /// Skill ids that need remediation at the start of the next hour.
    #[serde(default)]
    pub remediation: Vec<String>,
}

impl Course {
    pub fn skill(&self, id: &str) -> Option<&Skill> {
        self.skills.iter().find(|s| s.id == id)
    }

    pub fn skill_mut(&mut self, id: &str) -> Option<&mut Skill> {
        self.skills.iter_mut().find(|s| s.id == id)
    }

    /// Next hour that is not done yet.
    pub fn next_hour(&self) -> Option<&HourPlan> {
        self.hours.iter().find(|h| h.status != HourStatus::Done)
    }

    pub fn hour_mut(&mut self, hour: u32) -> Option<&mut HourPlan> {
        self.hours.iter_mut().find(|h| h.hour == hour)
    }

    pub fn hours_done(&self) -> usize {
        self.hours
            .iter()
            .filter(|h| h.status == HourStatus::Done)
            .count()
    }

    /// Pareto-weighted mastery across all skills, in [0, 1].
    pub fn weighted_mastery(&self) -> f32 {
        let total: f32 = self.skills.iter().map(|s| f32::from(s.pareto_weight)).sum();
        if total <= 0.0 {
            return 0.0;
        }
        self.skills
            .iter()
            .map(|s| f32::from(s.pareto_weight) * s.mastery.clamp(0.0, 1.0))
            .sum::<f32>()
            / total
    }

    /// Blend a fresh evidence-based estimate into a skill's mastery. New evidence
    /// dominates, but one lucky or unlucky quiz cannot erase the history.
    pub fn update_mastery(&mut self, skill_id: &str, estimate: f32) {
        if let Some(skill) = self.skill_mut(skill_id) {
            let estimate = estimate.clamp(0.0, 1.0);
            skill.mastery = if skill.mastery <= 0.0 {
                estimate
            } else {
                0.4 * skill.mastery + 0.6 * estimate
            };
        }
    }

    pub fn add_misconception(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() || self.misconceptions.iter().any(|m| m == text) {
            return;
        }
        self.misconceptions.push(text.to_string());
        // Keep the learner model compact: it is sent with every request.
        if self.misconceptions.len() > 20 {
            self.misconceptions.remove(0);
        }
    }

    pub fn add_coach_note(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.coach_notes.push(text.to_string());
        if self.coach_notes.len() > 10 {
            self.coach_notes.remove(0);
        }
    }

    /// A compact, token-cheap description of the learner, sent to Codex.
    pub fn learner_model(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("Topic: {}\n", self.topic));
        out.push_str(&format!("Goal (learner's words): {}\n", self.goal));
        out.push_str(&format!("Prior experience: {}\n", self.prior_experience));
        out.push_str(&format!("Level estimate: {}\n", self.level_estimate));
        out.push_str(&format!(
            "Target performance after the course: {}\n",
            self.target_performance
        ));
        out.push_str(&format!("Practice arena: {}\n", self.arena.as_str()));
        out.push_str("Skill map (id | name | pareto weight | taught in hour | mastery 0-1):\n");
        for s in &self.skills {
            out.push_str(&format!(
                "- {} | {} | w{} | h{} | {:.2}\n",
                s.id, s.name, s.pareto_weight, s.hour, s.mastery
            ));
        }
        out.push_str(&format!(
            "Capstone: {} — {}\n",
            self.capstone.title, self.capstone.description
        ));
        if !self.misconceptions.is_empty() {
            out.push_str("Known misconceptions to repair:\n");
            for m in &self.misconceptions {
                out.push_str(&format!("- {m}\n"));
            }
        }
        if !self.remediation.is_empty() {
            out.push_str(&format!(
                "Skills needing remediation next: {}\n",
                self.remediation.join(", ")
            ));
        }
        if !self.coach_notes.is_empty() {
            out.push_str("Coach notes from previous sessions:\n");
            for n in &self.coach_notes {
                out.push_str(&format!("- {n}\n"));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    /// worked_completion | independent | break_fix | capstone | roleplay | drill | problem
    pub kind: String,
    pub instructions: String,
    pub success_criteria: Vec<String>,
    /// Graduated hints: nudge -> pointer -> near-solution. Served locally, no tokens.
    pub hints: Vec<String>,
    pub minutes: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuizQuestion {
    pub id: String,
    pub question: String,
    pub skill_id: String,
    /// recall | apply | interleaved | diagnose
    pub kind: String,
    pub answer_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewCard {
    pub skill_id: String,
    pub front: String,
    pub back: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupInfo {
    pub script: String,
    pub teardown_script: String,
    pub est_ram_mb: u32,
    pub est_disk_mb: u32,
    pub notes: String,
}

/// Everything prepared for one hour. Written by the Codex "prepare" step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourPack {
    pub hour: u32,
    pub lesson_file: String,
    pub predict_question: String,
    pub tasks: Vec<Task>,
    pub setup: SetupInfo,
    pub check_script: String,
    pub cards: Vec<NewCard>,
    pub exit_quiz: Vec<QuizQuestion>,
    pub feynman_prompt: String,
}

/// Where the learner is inside the current hour, so sessions survive crashes,
/// closed laptops and interruptions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Recall,
    Read,
    Practice,
    Prove,
    Reflect,
    Done,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    pub hour: u32,
    pub phase: Phase,
    pub started_unix: u64,
    /// Highest hint level revealed per task id (for scoring honesty + coaching).
    #[serde(default)]
    pub hints_used: Vec<(String, u32)>,
    /// Result of the most recent lab check: (passed, total).
    #[serde(default)]
    pub lab_result: Option<(u32, u32)>,
    #[serde(default)]
    pub prediction: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub hour: u32,
    pub day: i64,
    pub minutes_spent: u32,
    pub quiz_score: f32,
    pub lab_score: Option<f32>,
    pub hour_score: f32,
    pub overconfidence: f32,
    pub brier: f32,
    pub reflection: String,
    pub coach_note: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CardDeck {
    pub cards: Vec<Card>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn course() -> Course {
        Course {
            slug: "x".into(),
            topic: "x".into(),
            title: "x".into(),
            goal: "g".into(),
            prior_experience: "p".into(),
            domain: "software".into(),
            arena: Arena::CodeLab,
            target_performance: "t".into(),
            out_of_scope: vec![],
            level_estimate: "beginner".into(),
            skills: vec![
                Skill {
                    id: "a".into(),
                    name: "A".into(),
                    why_it_matters: "".into(),
                    pareto_weight: 9,
                    prerequisites: vec![],
                    hour: 1,
                    mastery: 0.0,
                },
                Skill {
                    id: "b".into(),
                    name: "B".into(),
                    why_it_matters: "".into(),
                    pareto_weight: 1,
                    prerequisites: vec!["a".into()],
                    hour: 2,
                    mastery: 1.0,
                },
            ],
            hours: vec![],
            capstone: Capstone {
                title: "c".into(),
                description: "d".into(),
                acceptance_criteria: vec![],
            },
            research: ResearchHints::default(),
            lab_constraints: "".into(),
            minutes_per_hour: 60,
            created_day: 0,
            coach_notes: vec![],
            misconceptions: vec![],
            remediation: vec![],
        }
    }

    #[test]
    fn weighted_mastery_respects_pareto_weights() {
        let c = course();
        assert!((c.weighted_mastery() - 0.1).abs() < 1e-6);
    }

    #[test]
    fn mastery_blending() {
        let mut c = course();
        c.update_mastery("a", 0.5);
        assert!((c.skill("a").unwrap().mastery - 0.5).abs() < 1e-6);
        c.update_mastery("a", 1.0);
        assert!((c.skill("a").unwrap().mastery - 0.8).abs() < 1e-6);
        c.update_mastery("b", -3.0);
        assert!((c.skill("b").unwrap().mastery - 0.4).abs() < 1e-6);
    }

    #[test]
    fn misconceptions_are_deduplicated_and_bounded() {
        let mut c = course();
        c.add_misconception("same");
        c.add_misconception("same");
        assert_eq!(c.misconceptions.len(), 1);
        for i in 0..30 {
            c.add_misconception(&format!("m{i}"));
        }
        assert_eq!(c.misconceptions.len(), 20);
    }
}
