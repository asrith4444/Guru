//! "Download course": the one AI-heavy step. Codex plans the course, then
//! writes every lesson, quiz, flashcard deck and simulation. Afterwards the
//! course plays offline.
//!
//! - Resumable: finished assets are files; re-running only builds what is missing.
//! - Streams: module 1 and its simulations are built first, so the learner can
//!   start while the rest downloads.
//! - Parallel: a small worker pool sized to the machine's free memory.
//! - Self-checking: every simulation is validated (sim.rs) and sent back to Codex
//!   with the exact problems if it would not play correctly.

use std::collections::VecDeque;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;

use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::codex::Codex;
use crate::codex::ExecRequest;
use crate::codex::Sandbox;
use crate::config::unique_slug;
use crate::config::Config;
use crate::course::Course;
use crate::course::CourseStatus;
use crate::course::Deck;
use crate::course::Module;
use crate::course::ModuleInfo;
use crate::course::Package;
use crate::course::Question;
use crate::course::SimPlan;
use crate::course::Skill;
use crate::course::FORMAT_VERSION;
use crate::machine::MachineProfile;
use crate::prompts;
use crate::research;
use crate::schemas;
use crate::scope;
use crate::scope::Scope;
use crate::sim;
use crate::sim::Simulation;
use crate::srs;
use crate::srs::Card;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub topic: String,
    pub goal: String,
    pub level: u8,
    pub hours: f32,
    pub scope: Option<Scope>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub slug: String,
    /// planning | researching | building | ready | paused | failed
    pub phase: String,
    pub done: usize,
    pub total: usize,
    /// What is being built right now ("Lesson: m2 …", "Simulation: Grafana alerting").
    pub current: String,
    /// What Codex is doing ("Researching: …").
    pub activity: String,
    pub message: String,
}

pub type Emit<'a> = &'a (dyn Fn(Event) + Sync);

/// Ask Codex to size a topic for the level bar (one quick call).
pub fn scope(codex: &Codex, topic: &str, goal: &str) -> Result<Scope> {
    let dir = crate::config::guru_home().join("scratch");
    std::fs::create_dir_all(&dir)?;
    let v = codex.exec(
        ExecRequest {
            label: "scope",
            cwd: &dir,
            prompt: prompts::scope(topic, goal),
            schema: Some(schemas::scope()),
            sandbox: Sandbox::ReadOnly,
            web_search: false,
            effort: Some("low"),
        },
        None,
    )?;
    let s: Scope = serde_json::from_value(v).context("unexpected scope shape")?;
    Ok(s.normalized())
}

/// Create the package skeleton so it shows up in the library immediately.
pub fn create(req: &Request) -> Result<Package> {
    let root = Package::root();
    std::fs::create_dir_all(&root)?;
    let slug = unique_slug(&root, &req.topic);
    let pkg = Package::create(&slug)?;
    let level = req.level.min(100);
    let course = Course {
        format: FORMAT_VERSION,
        slug: slug.clone(),
        title: req
            .scope
            .as_ref()
            .map(|s| s.title.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| req.topic.clone()),
        topic: req.topic.clone(),
        goal: req.goal.clone(),
        level,
        level_label: scope::level_label(level).into(),
        hours: ((req.hours.clamp(0.5, 40.0)) * 2.0).round() / 2.0,
        status: CourseStatus::Planning,
        created_unix: srs::now_unix(),
        ..Default::default()
    };
    pkg.save_course(&course)?;
    if let Some(s) = &req.scope {
        crate::config::write_json(&pkg.scratch().join("scope.json"), s)?;
    }
    Ok(pkg)
}

fn worker_count(cfg: &Config, machine: &MachineProfile) -> usize {
    if let Some(n) = cfg.parallel_jobs {
        return n.clamp(1, 6) as usize;
    }
    // Each Codex worker is a separate process; stay well inside free memory.
    match machine.avail_ram_mb {
        m if m >= 8000 => 3,
        m if m >= 3000 => 2,
        _ => 1,
    }
}

#[derive(Deserialize)]
struct PlanOut {
    title: String,
    summary: String,
    target_performance: String,
    out_of_scope: Vec<String>,
    skills: Vec<Skill>,
    modules: Vec<ModuleInfo>,
    capstone: SimPlan,
    research: crate::course::ResearchHints,
}

fn sanitize_id(s: &str, fallback: &str) -> String {
    let id: String = s
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(60)
        .collect();
    if id.is_empty() {
        fallback.to_string()
    } else {
        id
    }
}

fn plan(codex: &Codex, pkg: &Package, course: &mut Course, emit: Emit<'_>) -> Result<()> {
    let modules = scope::modules_for(course.hours);
    let can_do = crate::config::read_json::<Scope>(&pkg.scratch().join("scope.json"))
        .map(|s| s.can_do(course.level).to_string())
        .unwrap_or_default();
    let ev = |activity: &str| {
        emit(Event {
            slug: course.slug.clone(),
            phase: "planning".into(),
            current: "Designing the course".into(),
            activity: activity.into(),
            ..Default::default()
        })
    };
    ev("Breaking the skill down…");
    let act = |a: &str| ev(a);
    let v = codex.exec(
        ExecRequest {
            label: "plan",
            cwd: &pkg.dir,
            prompt: prompts::plan(
                &course.topic,
                &course.goal,
                course.level,
                &course.level_label,
                &can_do,
                course.hours,
                modules,
            ),
            schema: Some(schemas::plan()),
            sandbox: Sandbox::ReadOnly,
            web_search: true,
            effort: None,
        },
        Some(&act),
    )?;
    let p: PlanOut = serde_json::from_value(v).context("unexpected plan shape")?;
    if p.modules.is_empty() {
        bail!("the plan has no modules");
    }
    course.title = p.title;
    course.summary = p.summary;
    course.target_performance = p.target_performance;
    course.out_of_scope = p.out_of_scope;
    course.skills = p
        .skills
        .into_iter()
        .map(|mut s| {
            s.id = sanitize_id(&s.id, "skill");
            s.weight = s.weight.clamp(1, 10);
            s
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    course.modules = p
        .modules
        .into_iter()
        .enumerate()
        .map(|(i, mut m)| {
            m.id = sanitize_id(&m.id, &format!("m{}", i + 1));
            if !seen.insert(m.id.clone()) {
                m.id = format!("m{}", i + 1);
                seen.insert(m.id.clone());
            }
            m.minutes = m.minutes.clamp(10, 180);
            for (j, s) in m.sims.iter_mut().enumerate() {
                s.id = sanitize_id(&s.id, &format!("{}s{}", m.id, j + 1));
                if !seen.insert(s.id.clone()) {
                    s.id = format!("{}s{}", m.id, j + 1);
                    seen.insert(s.id.clone());
                }
            }
            m
        })
        .collect();
    course.capstone = p.capstone;
    course.capstone.id = "capstone".into();
    course.research = p.research;
    course.status = CourseStatus::Downloading;
    pkg.save_course(course)?;
    Ok(())
}

fn research_digest(
    pkg: &Package,
    course: &Course,
    cfg: &Config,
    machine: &MachineProfile,
    emit: Emit<'_>,
) -> String {
    let path = pkg.dir.join("research").join("digest.md");
    if let Ok(d) = std::fs::read_to_string(&path) {
        return d;
    }
    let mut digest = String::new();
    if cfg.stackexchange && machine.has("curl") && !course.research.stackexchange_site.is_empty() {
        emit(Event {
            slug: course.slug.clone(),
            phase: "researching".into(),
            current: format!(
                "Collecting real-world problems from {}",
                course.research.stackexchange_site
            ),
            ..Default::default()
        });
        let save = pkg.dir.join("research").join("stackexchange.json");
        if let Ok(items) =
            research::fetch(&course.research, cfg.stackexchange_key.as_deref(), &save)
        {
            digest = research::digest(&items);
        }
    }
    let _ = std::fs::write(&path, &digest);
    digest
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

#[derive(Clone)]
enum Job {
    Module(usize),
    Sim(Option<usize>, SimPlan),
}

impl Job {
    fn label(&self, course: &Course) -> String {
        match self {
            Job::Module(i) => format!("Lesson & quiz: {}", course.modules[*i].title),
            Job::Sim(None, p) => format!("Capstone simulation: {}", p.title),
            Job::Sim(Some(_), p) => format!("Simulation: {}", p.title),
        }
    }
}

/// Keep only questions the player can grade.
fn clean_quiz(quiz: Vec<Question>) -> Vec<Question> {
    quiz.into_iter()
        .filter(|q| match q.kind.as_str() {
            "single" | "multi" => {
                !q.options.is_empty()
                    && !q.correct.is_empty()
                    && q.correct.iter().all(|&c| c < q.options.len())
            }
            "text" => {
                !q.accept.is_empty()
                    && q.accept.iter().all(|p| {
                        regex::RegexBuilder::new(p)
                            .case_insensitive(true)
                            .build()
                            .is_ok()
                    })
            }
            _ => false,
        })
        .collect()
}

fn build_module(
    codex: &Codex,
    pkg: &Package,
    course: &Course,
    idx: usize,
    issues: &str,
    deck: &Mutex<Deck>,
    act: crate::codex::Activity<'_>,
) -> Result<()> {
    let info = &course.modules[idx];
    let v = codex.exec(
        ExecRequest {
            label: &format!("module-{}", info.id),
            cwd: &pkg.dir,
            prompt: prompts::module(course, info, issues),
            schema: Some(schemas::module()),
            sandbox: Sandbox::ReadOnly,
            web_search: true,
            effort: None,
        },
        Some(act),
    )?;
    let mut m: Module = serde_json::from_value(v).context("unexpected module shape")?;
    if m.lesson.trim().is_empty() {
        bail!("module {} came back without a lesson", info.id);
    }
    m.id = info.id.clone();
    m.title = info.title.clone();
    m.quiz = clean_quiz(m.quiz);
    for (i, q) in m.quiz.iter_mut().enumerate() {
        q.id = format!("{}q{}", info.id, i + 1);
    }
    pkg.save_module(&m)?;
    let mut deck = deck.lock().unwrap_or_else(|e| e.into_inner());
    let today = srs::today();
    for (i, c) in m.cards.iter().enumerate() {
        if c.front.trim().is_empty() || deck.cards.iter().any(|d| d.front == c.front) {
            continue;
        }
        deck.cards.push(Card::new(
            format!("{}c{}", m.id, i + 1),
            c.skill_id.clone(),
            c.front.clone(),
            c.back.clone(),
            today,
        ));
    }
    pkg.save_deck(&deck)?;
    Ok(())
}

fn build_sim(
    codex: &Codex,
    pkg: &Package,
    course: &Course,
    module: Option<usize>,
    plan: &SimPlan,
    issues: &str,
    act: crate::codex::Activity<'_>,
) -> Result<()> {
    let info = module.map(|i| &course.modules[i]);
    let minutes = match info {
        Some(m) => (m.minutes / 2).clamp(10, 60),
        None => 30,
    };
    let base = prompts::simulation(course, info, plan, issues, minutes);
    let mut prompt = base.clone();
    let mut use_schema = true;
    let mut last_errors = Vec::new();
    for attempt in 0..3 {
        let label = format!("sim-{}-{attempt}", plan.id);
        let req = ExecRequest {
            label: &label,
            cwd: &pkg.dir,
            prompt: if use_schema {
                prompt.clone()
            } else {
                format!(
                    "{prompt}\n\nReturn ONLY one JSON object (no prose, no code fences) matching this JSON Schema:\n{}",
                    schemas::simulation()
                )
            },
            schema: use_schema.then(schemas::simulation),
            sandbox: Sandbox::ReadOnly,
            web_search: attempt == 0,
            effort: None,
        };
        let v: Value = match codex.exec(req, Some(act)) {
            Ok(v) => v,
            Err(e) if use_schema => {
                // Some providers/models reject large strict schemas: fall back to
                // instruction-only JSON and rely on our own validation.
                act(&format!("Retrying without strict schema ({e})"));
                use_schema = false;
                continue;
            }
            Err(e) => return Err(e),
        };
        let mut s: Simulation = match serde_json::from_value(v) {
            Ok(s) => s,
            Err(e) => {
                last_errors = vec![format!("JSON does not match the simulation shape: {e}")];
                prompt = format!("{base}{}", prompts::fix_simulation(&last_errors));
                continue;
            }
        };
        s.id = plan.id.clone();
        if s.title.trim().is_empty() {
            s.title = plan.title.clone();
        }
        let errors = sim::validate(&s);
        if errors.is_empty() {
            pkg.save_sim(&s)?;
            return Ok(());
        }
        act(&format!(
            "Fixing {} problem(s) in the simulation",
            errors.len()
        ));
        last_errors = errors;
        prompt = format!("{base}{}", prompts::fix_simulation(&last_errors));
    }
    bail!(
        "simulation '{}' did not pass validation: {}",
        plan.title,
        last_errors.join("; ")
    )
}

/// Run (or resume) the download. Returns when every asset exists, the run is
/// cancelled, or a step fails (the course is then resumable).
pub fn run(pkg: &Package, cfg: &Config, cancel: &AtomicBool, emit: Emit<'_>) -> Result<()> {
    let codex = Codex::from_config(cfg);
    let machine = MachineProfile::detect(&pkg.dir);
    let mut course = pkg.course()?;
    course.generator = codex.version().unwrap_or_default();

    let fail = |course: &mut Course, msg: String| -> Result<()> {
        course.status = CourseStatus::Failed;
        course.status_detail = msg.clone();
        pkg.save_course(course)?;
        emit(Event {
            slug: course.slug.clone(),
            phase: "failed".into(),
            message: msg.clone(),
            ..Default::default()
        });
        bail!(msg)
    };

    if course.modules.is_empty() {
        if let Err(e) = plan(&codex, pkg, &mut course, emit) {
            return fail(&mut course, format!("Planning failed: {e:#}"));
        }
    }
    course.status = CourseStatus::Downloading;
    course.status_detail.clear();
    pkg.save_course(&course)?;

    let digest = research_digest(pkg, &course, cfg, &machine, emit);
    let issues = if digest.trim().is_empty() {
        "(none collected; use your own knowledge of common real-world failures, verified with web search)".to_string()
    } else {
        truncate(&digest, 12_000).to_string()
    };

    // Build order = study order: lesson, then its simulations, module by module.
    let mut all = Vec::new();
    for (i, m) in course.modules.iter().enumerate() {
        all.push(Job::Module(i));
        for s in &m.sims {
            all.push(Job::Sim(Some(i), s.clone()));
        }
    }
    if !course.capstone.id.is_empty() {
        all.push(Job::Sim(None, course.capstone.clone()));
    }
    let exists = |j: &Job| match j {
        Job::Module(i) => pkg
            .module_path(&course.modules[*i].id)
            .map(|p| p.exists())
            .unwrap_or(false),
        Job::Sim(_, p) => pkg.sim_path(&p.id).map(|p| p.exists()).unwrap_or(false),
    };
    let total = all.len();
    let pending: VecDeque<Job> = all.iter().filter(|j| !exists(j)).cloned().collect();
    let done = AtomicUsize::new(total - pending.len());
    let queue = Mutex::new(pending);
    let deck = Mutex::new(pkg.deck());
    let errors = Mutex::new(Vec::<String>::new());
    let workers = worker_count(cfg, &machine);
    let course_ref = &course;

    let progress = |current: String, activity: String| {
        emit(Event {
            slug: course_ref.slug.clone(),
            phase: "building".into(),
            done: done.load(Ordering::SeqCst),
            total,
            current,
            activity,
            message: String::new(),
        })
    };
    progress("Starting".into(), format!("{workers} parallel builder(s)"));

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                if cancel.load(Ordering::SeqCst) {
                    return;
                }
                let job = { queue.lock().unwrap_or_else(|e| e.into_inner()).pop_front() };
                let Some(job) = job else { return };
                let label = job.label(course_ref);
                progress(label.clone(), String::new());
                let act = |a: &str| progress(label.clone(), a.to_string());
                let result = match &job {
                    Job::Module(i) => {
                        build_module(&codex, pkg, course_ref, *i, &issues, &deck, &act)
                    }
                    Job::Sim(m, p) => build_sim(&codex, pkg, course_ref, *m, p, &issues, &act),
                };
                match result {
                    Ok(()) => {
                        done.fetch_add(1, Ordering::SeqCst);
                        progress(format!("Ready: {label}"), String::new());
                    }
                    Err(e) => errors
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(format!("{label}: {e:#}")),
                }
            });
        }
    });

    let errors = errors.into_inner().unwrap_or_default();
    let done = done.load(Ordering::SeqCst);
    if cancel.load(Ordering::SeqCst) && done < total {
        course.status = CourseStatus::Failed;
        course.status_detail = "Paused. Resume to finish the download.".into();
        pkg.save_course(&course)?;
        emit(Event {
            slug: course.slug.clone(),
            phase: "paused".into(),
            done,
            total,
            message: course.status_detail.clone(),
            ..Default::default()
        });
        return Ok(());
    }
    if !errors.is_empty() {
        return fail(
            &mut course,
            format!(
                "{} of {total} items could not be built. Resume to retry them. {}",
                errors.len(),
                errors.join(" | ")
            ),
        );
    }
    course.status = CourseStatus::Ready;
    course.status_detail.clear();
    pkg.save_course(&course)?;
    emit(Event {
        slug: course.slug.clone(),
        phase: "ready".into(),
        done: total,
        total,
        message: "Your course is ready. No AI needed from here.".into(),
        ..Default::default()
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiz_cleaning_drops_ungradable_questions() {
        let q = |kind: &str, options: usize, correct: Vec<usize>, accept: Vec<&str>| Question {
            kind: kind.into(),
            options: (0..options).map(|i| i.to_string()).collect(),
            correct,
            accept: accept.into_iter().map(String::from).collect(),
            ..Default::default()
        };
        let cleaned = clean_quiz(vec![
            q("single", 4, vec![1], vec![]),
            q("single", 4, vec![7], vec![]),
            q("multi", 0, vec![0], vec![]),
            q("text", 0, vec![], vec!["rate\\(.*\\)"]),
            q("text", 0, vec![], vec!["(?<=x)"]),
            q("essay", 0, vec![], vec![]),
        ]);
        assert_eq!(cleaned.len(), 2);
    }

    #[test]
    fn ids_are_sanitized() {
        assert_eq!(sanitize_id("m1 / intro", "x"), "m1intro");
        assert_eq!(sanitize_id("../", "x"), "x");
        assert_eq!(truncate("héllo", 2), "h");
    }
}
