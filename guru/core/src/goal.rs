//! Goal-first learning: "What's your goal?" → a few sharp clarifying questions
//! → a professional, stage-by-stage roadmap. Each stage is downloaded as its own
//! course (generator.rs) that knows where it sits in the roadmap.
//!
//! Stored as `$GURU_HOME/goals/<slug>.json`.

use std::path::PathBuf;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;

use crate::codex::Codex;
use crate::codex::ExecRequest;
use crate::codex::Sandbox;
use crate::config::guru_home;
use crate::config::read_json;
use crate::config::unique_slug;
use crate::config::write_json;
use crate::generator::Request;
use crate::prompts;
use crate::schemas;
use crate::srs;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Question {
    pub id: String,
    pub question: String,
    /// Why Guru asks (shown small under the question).
    pub why: String,
    /// Suggested answers; the learner may also type their own.
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Clarification {
    pub restated_goal: String,
    pub field: String,
    pub questions: Vec<Question>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Answer {
    pub question: String,
    pub answer: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Stage {
    pub id: String,
    pub title: String,
    /// Why this stage comes here, in one or two sentences.
    pub why: String,
    pub level: u8,
    pub hours: f32,
    /// Measurable outcomes: what the learner can do after this stage.
    pub outcomes: Vec<String>,
    pub competencies: Vec<String>,
    /// The simulations to expect (e.g. "Fault-find a tripping RCD on a consumer unit").
    pub practice: Vec<String>,
    /// How mastery is demonstrated at the end of the stage.
    pub assessment: String,
    /// The downloaded course for this stage, once the learner starts it.
    pub course_slug: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct GoalPlan {
    pub slug: String,
    pub goal: String,
    pub answers: Vec<Answer>,
    pub title: String,
    pub field: String,
    /// What the learner can do at the end, described as a professional would.
    pub destination: String,
    /// Frameworks, codes, standards and certifications the roadmap follows.
    pub standards: Vec<String>,
    /// Real tools and instruments of the trade (all simulated inside Guru).
    pub tools: Vec<String>,
    /// Safety-critical rules of the field (empty when not applicable).
    pub safety: Vec<String>,
    /// What still needs the real world: licences, supervised hours, physical practice.
    pub beyond_guru: Vec<String>,
    pub stages: Vec<Stage>,
    pub created_unix: u64,
}

impl GoalPlan {
    pub fn total_hours(&self) -> f32 {
        self.stages.iter().map(|s| s.hours).sum()
    }

    /// Compact roadmap context passed into every asset of a stage's course.
    pub fn context_for(&self, stage_id: &str) -> String {
        let mut s = format!(
            "Roadmap: {} ({})\nLearner's goal: {}\nDestination: {}\n",
            self.title, self.field, self.goal, self.destination
        );
        for a in &self.answers {
            s.push_str(&format!("- {}: {}\n", a.question, a.answer));
        }
        if !self.standards.is_empty() {
            s.push_str(&format!(
                "Standards to follow: {}\n",
                self.standards.join("; ")
            ));
        }
        if !self.safety.is_empty() {
            s.push_str(&format!(
                "Safety rules of the field: {}\n",
                self.safety.join("; ")
            ));
        }
        s.push_str("Stages:\n");
        for st in &self.stages {
            let marker = if st.id == stage_id {
                "→ THIS STAGE"
            } else {
                ""
            };
            s.push_str(&format!(
                "- {} {} ({} h, level {}): {} {marker}\n",
                st.id,
                st.title,
                st.hours,
                st.level,
                st.outcomes.join("; ")
            ));
        }
        s
    }

    pub fn stage(&self, id: &str) -> Option<&Stage> {
        self.stages.iter().find(|s| s.id == id)
    }

    /// The download request for one stage.
    pub fn request_for(&self, stage_id: &str) -> Result<Request> {
        let st = self.stage(stage_id).context("unknown stage")?;
        Ok(Request {
            topic: st.title.clone(),
            goal: format!(
                "{} Outcomes: {}. Assessment: {}",
                st.why,
                st.outcomes.join("; "),
                st.assessment
            ),
            level: st.level,
            hours: st.hours,
            scope: None,
            goal_context: self.context_for(stage_id),
            goal_slug: self.slug.clone(),
            stage_id: stage_id.to_string(),
        })
    }
}

pub fn goals_dir() -> PathBuf {
    guru_home().join("goals")
}

fn path(slug: &str) -> Result<PathBuf> {
    if slug.is_empty() || !slug.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        bail!("invalid goal id '{slug}'");
    }
    Ok(goals_dir().join(format!("{slug}.json")))
}

pub fn load(slug: &str) -> Result<GoalPlan> {
    read_json(&path(slug)?)
}

pub fn save(plan: &GoalPlan) -> Result<()> {
    write_json(&path(&plan.slug)?, plan)
}

pub fn delete(slug: &str) -> Result<()> {
    std::fs::remove_file(path(slug)?)?;
    Ok(())
}

pub fn list() -> Vec<GoalPlan> {
    let mut v: Vec<GoalPlan> = std::fs::read_dir(goals_dir())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .filter_map(|e| read_json(&e.path()).ok())
                .collect()
        })
        .unwrap_or_default();
    v.sort_by_key(|g: &GoalPlan| std::cmp::Reverse(g.created_unix));
    v
}

/// Record which course a stage was downloaded as.
pub fn link_course(slug: &str, stage_id: &str, course_slug: &str) -> Result<()> {
    let mut plan = load(slug)?;
    if let Some(st) = plan.stages.iter_mut().find(|s| s.id == stage_id) {
        st.course_slug = course_slug.to_string();
    }
    save(&plan)
}

fn scratch() -> Result<PathBuf> {
    let d = guru_home().join("scratch");
    std::fs::create_dir_all(&d)?;
    Ok(d)
}

/// Step 1: the questions a senior practitioner would ask before planning.
pub fn clarify(codex: &Codex, goal: &str) -> Result<Clarification> {
    let v = codex.exec(
        ExecRequest {
            label: "clarify",
            cwd: &scratch()?,
            prompt: prompts::clarify(goal),
            schema: Some(schemas::clarify()),
            sandbox: Sandbox::ReadOnly,
            web_search: false,
            effort: Some("low"),
        },
        None,
    )?;
    let mut c: Clarification =
        serde_json::from_value(v).context("unexpected clarification shape")?;
    c.questions.truncate(6);
    for (i, q) in c.questions.iter_mut().enumerate() {
        q.id = format!("q{}", i + 1);
    }
    Ok(c)
}

#[derive(Deserialize)]
struct PlanOut {
    title: String,
    field: String,
    destination: String,
    standards: Vec<String>,
    tools: Vec<String>,
    safety: Vec<String>,
    beyond_guru: Vec<String>,
    stages: Vec<Stage>,
}

/// Step 2: the roadmap.
pub fn plan(codex: &Codex, goal: &str, answers: Vec<Answer>) -> Result<GoalPlan> {
    let mut last = None;
    // The roadmap is the learner's first impression: retry once on any problem.
    for attempt in 0..2 {
        let r = codex.exec(
            ExecRequest {
                label: &format!("goal-plan-{attempt}"),
                cwd: &scratch()?,
                prompt: prompts::goal_plan(goal, &answers),
                schema: Some(schemas::goal_plan()),
                sandbox: Sandbox::ReadOnly,
                web_search: true,
                effort: None,
            },
            None,
        );
        match r
            .and_then(|v| serde_json::from_value::<PlanOut>(v).context("unexpected roadmap shape"))
        {
            Ok(p) if !p.stages.is_empty() => {
                let slug = unique_slug_file(&p.title);
                let mut plan = GoalPlan {
                    slug,
                    goal: goal.to_string(),
                    answers: answers.clone(),
                    title: p.title,
                    field: p.field,
                    destination: p.destination,
                    standards: p.standards,
                    tools: p.tools,
                    safety: p.safety,
                    beyond_guru: p.beyond_guru,
                    stages: p.stages,
                    created_unix: srs::now_unix(),
                };
                normalize(&mut plan);
                save(&plan)?;
                return Ok(plan);
            }
            Ok(_) => last = Some(anyhow::anyhow!("the roadmap came back without stages")),
            Err(e) => {
                if crate::generator::classify(&format!("{e:#}"))
                    == crate::generator::ErrorKind::Auth
                {
                    return Err(e);
                }
                last = Some(e)
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("roadmap failed")))
}

fn unique_slug_file(title: &str) -> String {
    let dir = goals_dir();
    let _ = std::fs::create_dir_all(&dir);
    // unique_slug checks directory entries; goals are files, so check both forms.
    let base = unique_slug(&dir, title);
    if !dir.join(format!("{base}.json")).exists() {
        return base;
    }
    (2..)
        .map(|n| format!("{base}-{n}"))
        .find(|s| !dir.join(format!("{s}.json")).exists())
        .unwrap_or(base)
}

/// Clean up whatever the model returned: ids, levels that only go up, sane hours.
pub fn normalize(plan: &mut GoalPlan) {
    let mut last_level = 0u8;
    for (i, st) in plan.stages.iter_mut().enumerate() {
        st.id = format!("s{}", i + 1);
        st.level = st.level.clamp(last_level, 100);
        last_level = st.level;
        st.hours = ((st.hours.clamp(0.5, 20.0)) * 2.0).round() / 2.0;
        st.course_slug.clear();
    }
    plan.stages.truncate(10);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan() -> GoalPlan {
        GoalPlan {
            slug: "x".into(),
            goal: "become an electrician's mate".into(),
            title: "Electrical fundamentals".into(),
            field: "Electrical installation".into(),
            destination: "Fault-find final circuits safely".into(),
            standards: vec!["BS 7671".into()],
            safety: vec!["Safe isolation".into()],
            stages: vec![
                Stage {
                    id: "a".into(),
                    title: "Safe isolation".into(),
                    level: 40,
                    hours: 2.26,
                    outcomes: vec!["prove dead".into()],
                    ..Default::default()
                },
                Stage {
                    id: "b".into(),
                    title: "Fault finding".into(),
                    level: 20,
                    hours: 99.0,
                    ..Default::default()
                },
            ],
            ..Default::default()
        }
    }

    #[test]
    fn normalization_orders_levels_and_ids() {
        let mut p = plan();
        normalize(&mut p);
        assert_eq!(p.stages[0].id, "s1");
        assert_eq!(p.stages[1].id, "s2");
        assert_eq!(p.stages[0].hours, 2.5);
        assert_eq!(p.stages[1].hours, 20.0);
        assert_eq!(
            p.stages[1].level, 40,
            "levels never go down along the roadmap"
        );
        assert_eq!(p.total_hours(), 22.5);
    }

    #[test]
    fn stage_requests_carry_the_roadmap() {
        let mut p = plan();
        normalize(&mut p);
        let r = p.request_for("s1").unwrap();
        assert_eq!(r.topic, "Safe isolation");
        assert_eq!(r.stage_id, "s1");
        assert!(r.goal_context.contains("BS 7671"));
        assert!(r.goal_context.contains("→ THIS STAGE"));
        assert!(r.goal.contains("prove dead"));
        assert!(p.request_for("nope").is_err());
    }

    #[test]
    fn ids_are_checked() {
        assert!(path("../x").is_err());
        assert!(path("ok-1").is_ok());
    }
}
