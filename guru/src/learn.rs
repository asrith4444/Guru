//! `guru learn "<topic>"`: intake -> skill deconstruction -> diagnostic
//! pretest -> calibration + real-world issue bank -> ready for hour 1.

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

use crate::codex::Codex;
use crate::codex::ExecRequest;
use crate::codex::Sandbox;
use crate::config::unique_slug;
use crate::config::Config;
use crate::config::Store;
use crate::machine::MachineProfile;
use crate::model::Arena;
use crate::model::Capstone;
use crate::model::Course;
use crate::model::DiagnosticQuestion;
use crate::model::HourPlan;
use crate::model::Issue;
use crate::model::ResearchHints;
use crate::model::Skill;
use crate::prompts;
use crate::research;
use crate::schemas;
use crate::srs;
use crate::ui;

#[derive(Deserialize)]
struct PlanSkill {
    id: String,
    name: String,
    why_it_matters: String,
    pareto_weight: i64,
    prerequisites: Vec<String>,
    hour: i64,
}

#[derive(Deserialize)]
struct PlanHour {
    hour: i64,
    title: String,
    objectives: Vec<String>,
    skill_ids: Vec<String>,
    capstone_slice: String,
}

#[derive(Deserialize)]
struct Plan {
    title: String,
    domain: String,
    arena: Arena,
    target_performance: String,
    out_of_scope: Vec<String>,
    level_estimate: String,
    skills: Vec<PlanSkill>,
    hours: Vec<PlanHour>,
    capstone: Capstone,
    diagnostic: Vec<DiagnosticQuestion>,
    research: ResearchHints,
    lab_constraints: String,
}

#[derive(Deserialize)]
struct Calibration {
    diagnostic_results: Vec<DiagScore>,
    skill_mastery: Vec<SkillMastery>,
    level_estimate: String,
    plan_changes: String,
    hours: Vec<PlanHour>,
    issues: Vec<Issue>,
    misconceptions: Vec<String>,
}

#[derive(Deserialize)]
struct DiagScore {
    id: String,
    score: f32,
    feedback: String,
}

#[derive(Deserialize)]
pub struct SkillMastery {
    pub skill_id: String,
    pub mastery: f32,
}

/// Normalise the planner's hours so there is exactly one entry per hour 1..=n.
fn to_hours(raw: Vec<PlanHour>, n: u32) -> Vec<HourPlan> {
    let mut hours: Vec<HourPlan> = raw
        .into_iter()
        .map(|h| HourPlan {
            hour: h.hour.clamp(1, i64::from(n)) as u32,
            title: h.title,
            objectives: h.objectives,
            skill_ids: h.skill_ids,
            capstone_slice: h.capstone_slice,
            status: Default::default(),
            score: None,
        })
        .collect();
    hours.sort_by_key(|h| h.hour);
    hours.dedup_by_key(|h| h.hour);
    for i in 1..=n {
        if !hours.iter().any(|h| h.hour == i) {
            hours.push(HourPlan {
                hour: i,
                title: "Practice and consolidation".into(),
                objectives: vec!["Consolidate earlier skills through mixed practice".into()],
                skill_ids: vec![],
                capstone_slice: "Extend the capstone".into(),
                status: Default::default(),
                score: None,
            });
        }
    }
    hours.sort_by_key(|h| h.hour);
    hours.truncate(n as usize);
    hours
}

pub fn run(topic: &str, cfg: &Config, hours: u32, minutes: u32) -> Result<()> {
    let codex = Codex::from_config(cfg);
    preflight(&codex)?;

    ui::header(&format!("Guru · {topic}"));
    ui::info(&ui::dim(&format!(
        "{hours} sessions × {minutes} min. Read, do, recall. Answer briefly; Enter to skip."
    )));
    let goal = ui::ask("What do you want to be able to DO after this course? (be concrete)");
    let prior = ui::ask("What do you already know that is related? (languages, tools, experience)");

    let slug = unique_slug(topic);
    let store = Store::create(&slug)?;
    let machine = MachineProfile::detect(&store.dir);
    let b = machine.budget();
    ui::ok(&format!(
        "Machine: {} CPU threads, {} MB RAM free → lab budget {} MB RAM, {} MB disk ({:?} tier{}).",
        machine.cpus,
        machine.avail_ram_mb,
        b.max_ram_mb,
        b.max_disk_mb,
        b.tier,
        if b.containers {
            ", containers ok"
        } else {
            ", no containers"
        }
    ));

    ui::info("\nDeconstructing the skill (this takes a minute or two)…");
    let plan_v = codex.exec(ExecRequest {
        label: "plan",
        cwd: &store.dir,
        prompt: prompts::plan(topic, &goal, &prior, hours, minutes, &machine),
        schema: Some(schemas::plan()),
        sandbox: Sandbox::ReadOnly,
        web_search: true,
        quiet: false,
        images: vec![],
    })?;
    let plan: Plan = serde_json::from_value(plan_v).context("unexpected plan shape")?;

    let mut course = Course {
        slug: slug.clone(),
        topic: topic.to_string(),
        title: plan.title,
        goal: if goal.trim().is_empty() {
            "(not stated)".into()
        } else {
            goal
        },
        prior_experience: if prior.trim().is_empty() {
            "(not stated)".into()
        } else {
            prior
        },
        domain: plan.domain,
        arena: plan.arena,
        target_performance: plan.target_performance,
        out_of_scope: plan.out_of_scope,
        level_estimate: plan.level_estimate,
        skills: plan
            .skills
            .into_iter()
            .map(|s| Skill {
                id: s.id,
                name: s.name,
                why_it_matters: s.why_it_matters,
                pareto_weight: s.pareto_weight.clamp(1, 10) as u8,
                prerequisites: s.prerequisites,
                hour: s.hour.clamp(1, i64::from(hours)) as u32,
                mastery: 0.0,
            })
            .collect(),
        hours: to_hours(plan.hours, hours),
        capstone: plan.capstone,
        research: plan.research,
        lab_constraints: plan.lab_constraints,
        minutes_per_hour: minutes,
        created_day: srs::today(),
        coach_notes: vec![],
        misconceptions: vec![],
        remediation: vec![],
    };
    store.save_course(&course)?;
    Store::set_active(&slug)?;
    show_plan(&course);

    // Mine Stack Exchange in the background while the learner takes the pretest.
    let se_handle = if cfg.stackexchange
        && machine.has("curl")
        && !course.research.stackexchange_site.is_empty()
    {
        let hints = course.research.clone();
        let key = cfg.stackexchange_key.clone();
        let save_to = store.dir.join("research").join("stackexchange.json");
        Some(std::thread::spawn(move || {
            research::fetch(&hints, key.as_deref(), &save_to)
        }))
    } else {
        None
    };

    ui::phase(
        "Pretest",
        5,
        "Guessing before you are taught improves what you learn next, even when you are wrong. \
         It also lets Guru skip what you already know.",
    );
    let mut answers = Vec::new();
    for (i, q) in plan.diagnostic.iter().enumerate() {
        println!("\n{} {}", ui::bold(&format!("Q{}.", i + 1)), q.question);
        let a = ui::ask("›");
        answers.push((q.id.clone(), q.question.clone(), q.answer_key.clone(), a));
    }

    let se_digest = match se_handle.map(|h| h.join()) {
        Some(Ok(Ok(items))) if !items.is_empty() => {
            ui::ok(&format!(
                "Collected {} highly-voted real-world questions from {}.",
                items.len(),
                course.research.stackexchange_site
            ));
            research::digest(&items)
        }
        Some(Ok(Err(e))) => {
            ui::warn(&format!(
                "Stack Exchange unavailable ({e}); Codex will research the web instead."
            ));
            String::new()
        }
        _ => String::new(),
    };

    ui::info("\nCalibrating your plan and building the real-world issue bank…");
    let cal_v = codex.exec(ExecRequest {
        label: "calibrate",
        cwd: &store.dir,
        prompt: prompts::calibrate(&course, &answers, &se_digest),
        schema: Some(schemas::calibrate()),
        sandbox: Sandbox::ReadOnly,
        web_search: true,
        quiet: false,
        images: vec![],
    })?;
    let cal: Calibration = serde_json::from_value(cal_v).context("unexpected calibration shape")?;

    println!();
    for r in &cal.diagnostic_results {
        let mark = if r.score >= 0.8 {
            ui::green("✓")
        } else if r.score >= 0.4 {
            ui::yellow("~")
        } else {
            ui::red("✗")
        };
        let n = plan_index(&plan.diagnostic, &r.id);
        println!("{mark} Q{n}: {}", r.feedback);
    }
    for m in cal.skill_mastery {
        course.update_mastery(&m.skill_id, m.mastery);
    }
    for m in &cal.misconceptions {
        course.add_misconception(m);
    }
    course.level_estimate = cal.level_estimate;
    if !cal.hours.is_empty() {
        course.hours = to_hours(cal.hours, hours);
    }
    course.add_coach_note(&cal.plan_changes);
    store.save_course(&course)?;
    store.save_issues(&cal.issues)?;
    std::fs::write(
        store.dir.join("AGENTS.md"),
        prompts::agents_md(&course, &machine, 1),
    )?;
    init_git(&store.dir);

    ui::header("Plan calibrated");
    if !cal.plan_changes.trim().is_empty() {
        ui::info(&cal.plan_changes);
    }
    ui::ok(&format!(
        "Problem bank: {} real-world {}.",
        cal.issues.len(),
        if cal.issues.len() == 1 {
            "issue"
        } else {
            "issues"
        }
    ));
    ui::info(&format!(
        "\nCourse saved in {}\nStart hour 1 now or any time with {}.",
        store.dir.display(),
        ui::bold("guru next")
    ));
    if ui::confirm("Start hour 1 now?", true) {
        crate::session::run(cfg)?;
    }
    Ok(())
}

fn plan_index(diag: &[DiagnosticQuestion], id: &str) -> usize {
    diag.iter()
        .position(|q| q.id == id)
        .map(|i| i + 1)
        .unwrap_or(0)
}

pub fn show_plan(course: &Course) {
    ui::header(&course.title);
    println!("{} {}", ui::bold("Target:"), course.target_performance);
    println!("{} {}", ui::bold("Practice arena:"), course.arena.as_str());
    println!("{} {}", ui::bold("Capstone:"), course.capstone.title);
    if !course.out_of_scope.is_empty() {
        println!(
            "{} {}",
            ui::bold("Deliberately later:"),
            course.out_of_scope.join("; ")
        );
    }
    println!();
    for h in &course.hours {
        println!("  {} {}", ui::cyan(&format!("Hour {}", h.hour)), h.title);
    }
}

pub fn preflight(codex: &Codex) -> Result<()> {
    if !codex.available() {
        anyhow::bail!(
            "Codex CLI not found ({}). Install it (npm i -g @openai/codex) or set GURU_CODEX_BIN / `guru config codex_bin <path>`.",
            codex.bin.display()
        );
    }
    let (logged_in, msg) = codex.login_status();
    if !logged_in {
        ui::warn(&format!("Codex is not signed in ({msg})."));
        if ui::confirm("Sign in with ChatGPT now?", true) {
            codex.login()?;
        } else {
            anyhow::bail!("Guru needs Codex to be signed in. Run `guru login`.");
        }
    }
    Ok(())
}

/// A git repo makes the course a proper Codex project root and lets the
/// learner (and the tutor) diff their own progress. Best effort.
fn init_git(dir: &std::path::Path) {
    if dir.join(".git").exists() || crate::machine::which("git").is_none() {
        return;
    }
    let _ = std::fs::write(
        dir.join(".gitignore"),
        ".guru/\ntarget/\nnode_modules/\n.venv/\n__pycache__/\n",
    );
    let _ = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(dir)
        .status();
}

/// Used by the session to parse mastery updates.
pub fn parse_mastery(v: &Value) -> Vec<SkillMastery> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| serde_json::from_value(x.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ph(hour: i64) -> PlanHour {
        PlanHour {
            hour,
            title: format!("t{hour}"),
            objectives: vec![],
            skill_ids: vec![],
            capstone_slice: String::new(),
        }
    }

    #[test]
    fn hours_are_normalised() {
        let hours = to_hours(vec![ph(3), ph(1), ph(1), ph(99)], 4);
        let ids: Vec<u32> = hours.iter().map(|h| h.hour).collect();
        assert_eq!(ids, vec![1, 2, 3, 4]);
        assert_eq!(hours[0].title, "t1");
        assert_eq!(
            hours[3].title, "t99",
            "out-of-range hour is clamped to the last"
        );
        assert!(hours[1].title.contains("consolidation"));
    }
}
