//! "Download course": the one AI-heavy step. Codex plans the course, then
//! writes every lesson, quiz, flashcard deck and simulation. Afterwards the
//! course plays offline.
//!
//! Designed not to fail half-way:
//! - Preflight: Codex installed, signed in, enough disk — checked before anything starts.
//! - Every Codex request has a hard timeout (codex.rs) and every asset is retried with
//!   backoff. Usage limits pause the download with a clear message instead of
//!   burning retries; a lost sign-in stops it at once.
//! - Resumable: finished assets are files; re-running only builds what is missing.
//!   A download interrupted by a crash or shutdown is marked as such on next start.
//! - Streams: module 1 and its simulations are built first, so study can start early.
//! - Quality gates: lessons, quizzes and simulations that are thin or unplayable are
//!   sent back to Codex with the exact problems (sim.rs validates playability).
//! - Everything is logged to `.guru/download.log` inside the course.

use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::sync::Mutex;
use std::time::Duration;

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Request {
    pub topic: String,
    pub goal: String,
    pub level: u8,
    pub hours: f32,
    pub scope: Option<Scope>,
    /// Set when the course is a stage of a goal roadmap (see goal.rs).
    pub goal_context: String,
    pub goal_slug: String,
    pub stage_id: String,
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
    /// What Codex is doing ("Researching: …", "Retrying in 20 s…").
    pub activity: String,
    pub message: String,
}

pub type Emit<'a> = &'a (dyn Fn(Event) + Sync);

// ------------------------------------------------------------- errors ----

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Not signed in / token expired: retrying cannot help.
    Auth,
    /// ChatGPT plan usage limit or rate limit: wait, then resume.
    RateLimit,
    /// Timeouts, network drops, overloaded servers: retry soon.
    Transient,
    /// Anything else (bad output, validation): retry with feedback.
    Other,
}

pub fn classify(message: &str) -> ErrorKind {
    let m = message.to_lowercase();
    let any = |words: &[&str]| words.iter().any(|w| m.contains(w));
    if any(&[
        "not logged in",
        "unauthorized",
        "401",
        "login required",
        "please log in",
        "sign in",
        "authentication",
        "token expired",
        "refresh token",
    ]) {
        ErrorKind::Auth
    } else if any(&[
        "usage limit",
        "rate limit",
        "rate_limit",
        "429",
        "quota",
        "too many requests",
    ]) {
        ErrorKind::RateLimit
    } else if any(&[
        "timed out",
        "timeout",
        "network",
        "connection",
        "disconnected",
        "stream",
        "502",
        "503",
        "504",
        "500",
        "overloaded",
        "temporarily",
        "unavailable",
        "reset by peer",
        "eof",
    ]) {
        ErrorKind::Transient
    } else {
        ErrorKind::Other
    }
}

/// Base delay for retries; tests set GURU_RETRY_DELAY_MS=0.
fn base_delay() -> Duration {
    Duration::from_millis(
        std::env::var("GURU_RETRY_DELAY_MS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5_000),
    )
}

/// Sleep that wakes up early when the download is cancelled.
fn wait(d: Duration, cancel: &AtomicBool) {
    let end = std::time::Instant::now() + d;
    while std::time::Instant::now() < end && !cancel.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100).min(end - std::time::Instant::now()));
    }
}

/// A reason to stop the whole download (not just one asset).
#[derive(Debug, Clone)]
struct Halt {
    phase: &'static str,
    message: String,
}

/// Run `f` with retries. Returns Err(Ok(halt)) when the whole download must
/// stop, Err(Err(e)) when this item failed for good.
fn with_retries(
    what: &str,
    cancel: &AtomicBool,
    act: &(dyn Fn(&str) + Sync),
    mut f: impl FnMut(usize) -> Result<()>,
) -> std::result::Result<(), std::result::Result<Halt, anyhow::Error>> {
    const ATTEMPTS: usize = 4;
    let base = base_delay();
    let mut rate_limited = 0;
    let mut attempt = 0;
    loop {
        match f(attempt) {
            Ok(()) => return Ok(()),
            Err(e) => {
                let msg = format!("{e:#}");
                match classify(&msg) {
                    ErrorKind::Auth => {
                        return Err(Ok(Halt {
                            phase: "failed",
                            message: "Codex is not signed in (or the sign-in expired). Open Settings → Sign in with ChatGPT, then press Resume. Everything built so far is kept.".into(),
                        }))
                    }
                    ErrorKind::RateLimit => {
                        rate_limited += 1;
                        if rate_limited > 2 {
                            return Err(Ok(Halt {
                                phase: "paused",
                                message: "Paused: your ChatGPT usage limit was reached. Press Resume later; everything built so far is kept.".into(),
                            }));
                        }
                        let d = base * 12 * rate_limited as u32;
                        act(&format!("Usage limit hit while building {what}; waiting {} s…", d.as_secs()));
                        wait(d, cancel);
                    }
                    kind => {
                        attempt += 1;
                        if attempt >= ATTEMPTS {
                            return Err(Err(e));
                        }
                        let d = base * [1u32, 4, 12][attempt.min(3) - 1];
                        let why = if kind == ErrorKind::Transient { "connection problem" } else { "problem" };
                        act(&format!("Retrying {what} after a {why} (attempt {}/{ATTEMPTS}, in {} s)", attempt + 1, d.as_secs()));
                        wait(d, cancel);
                    }
                }
                if cancel.load(Ordering::SeqCst) {
                    return Err(Ok(Halt {
                        phase: "paused",
                        message: "Paused. Resume to finish the download.".into(),
                    }));
                }
            }
        }
    }
}

// ---------------------------------------------------------- preflight ----

/// Fail fast, before anything is created or any AI is used.
pub fn preflight(codex: &Codex) -> Result<()> {
    if codex.version().is_none() {
        bail!(
            "Codex CLI was not found at '{}'. Install it (npm i -g @openai/codex) or set its path in Settings.",
            codex.bin.display()
        );
    }
    let (ok, msg) = codex.login_status();
    if !ok {
        bail!("Codex is not signed in ({msg}). Open Settings → Sign in with ChatGPT.");
    }
    let root = Package::root();
    std::fs::create_dir_all(&root)?;
    if let Some(free) = MachineProfile::detect(&root).free_disk_mb {
        if free < 300 {
            bail!("Only {free} MB of disk space is free; Guru needs about 300 MB to build a course safely.");
        }
    }
    Ok(())
}

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
        goal_context: req.goal_context.clone(),
        goal_slug: req.goal_slug.clone(),
        stage_id: req.stage_id.clone(),
        ..Default::default()
    };
    pkg.save_course(&course)?;
    if let Some(s) = &req.scope {
        crate::config::write_json(&pkg.scratch().join("scope.json"), s)?;
    }
    Ok(pkg)
}

/// Downloads that were running when the app quit or crashed are still marked
/// as downloading on disk. Mark them resumable so the UI offers Resume.
pub fn recover_interrupted() -> usize {
    let mut n = 0;
    for pkg in Package::list() {
        if let Ok(mut c) = pkg.course() {
            if matches!(c.status, CourseStatus::Planning | CourseStatus::Downloading) {
                c.status = CourseStatus::Failed;
                c.status_detail = "Interrupted when Guru closed. Press Resume to finish; everything built so far is kept.".into();
                if pkg.save_course(&c).is_ok() {
                    n += 1;
                }
            }
        }
    }
    n
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

// -------------------------------------------------------------- plan ----

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

fn plan(
    codex: &Codex,
    pkg: &Package,
    course: &mut Course,
    act: &(dyn Fn(&str) + Sync),
) -> Result<()> {
    let modules = scope::modules_for(course.hours);
    let can_do = crate::config::read_json::<Scope>(&pkg.scratch().join("scope.json"))
        .map(|s| s.can_do(course.level).to_string())
        .unwrap_or_default();
    act("Breaking the skill down…");
    let v = codex.exec(
        ExecRequest {
            label: "plan",
            cwd: &pkg.dir,
            prompt: prompts::plan(course, &can_do, modules),
            schema: Some(schemas::plan()),
            sandbox: Sandbox::ReadOnly,
            web_search: true,
            effort: None,
        },
        Some(act),
    )?;
    let p: PlanOut = serde_json::from_value(v).context("unexpected plan shape")?;
    if p.modules.is_empty() || p.skills.is_empty() {
        bail!("the plan came back without modules or skills");
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
    let skill_ids: std::collections::HashSet<String> =
        course.skills.iter().map(|s| s.id.clone()).collect();
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
            m.skill_ids.retain(|s| skill_ids.contains(s));
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
        // Research is a bonus: failures here never stop the download.
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

// ------------------------------------------------------------ modules ----

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

fn words(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Is this module worth the learner's time? Problems are fed back to Codex.
pub fn module_issues(m: &Module, minutes: u32) -> Vec<String> {
    let mut v = Vec::new();
    let want = (minutes as usize * 10).clamp(450, 1200);
    let w = words(&m.lesson);
    if w < want {
        v.push(format!("the lesson is too thin ({w} words); write about {want}-{} words of dense, concrete material", want + 600));
    }
    let sections = m.lesson.lines().filter(|l| l.starts_with("## ")).count();
    if sections < 5 {
        v.push(format!("the lesson has {sections} '## ' sections; follow the required structure (why, mental model, worked example, the 20%, how it breaks, contrast, cheat sheet)"));
    }
    if m.quiz.len() < 5 {
        v.push(format!("only {} gradable quiz questions; write 5-7 (single needs options+correct indices, text needs accept regexes)", m.quiz.len()));
    }
    if m.quiz.iter().any(|q| q.explanation.trim().len() < 20) {
        v.push("every quiz question needs an explanation of why the answer is right".into());
    }
    if m.cards.len() < 6 {
        v.push(format!("only {} flashcards; write 6-10", m.cards.len()));
    }
    if m.key_points.len() < 4 {
        v.push("give 4-6 key points".into());
    }
    v
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
    let base = prompts::module(course, info, issues);
    let mut prompt = base.clone();
    let mut m = Module::default();
    for round in 0..2 {
        let v = codex.exec(
            ExecRequest {
                label: &format!("module-{}-{round}", info.id),
                cwd: &pkg.dir,
                prompt: prompt.clone(),
                schema: Some(schemas::module()),
                sandbox: Sandbox::ReadOnly,
                web_search: round == 0,
                effort: None,
            },
            Some(act),
        )?;
        m = serde_json::from_value(v).context("unexpected module shape")?;
        m.quiz = clean_quiz(m.quiz);
        let problems = module_issues(&m, info.minutes);
        if problems.is_empty() {
            break;
        }
        if round == 0 {
            act(&format!(
                "Improving the lesson ({} quality issue(s))",
                problems.len()
            ));
            prompt = format!("{base}{}", prompts::quality_feedback(&problems));
        } else if words(&m.lesson) < 250 || m.quiz.len() < 3 {
            bail!(
                "module {} is still too thin after revision: {}",
                info.id,
                problems.join("; ")
            );
        }
    }
    m.id = info.id.clone();
    m.title = info.title.clone();
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

// --------------------------------------------------------- simulations ----

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
    const ROUNDS: usize = 3;
    for attempt in 0..ROUNDS {
        let label = format!("sim-{}-{attempt}", plan.id);
        let req = ExecRequest {
            label: &label,
            cwd: &pkg.dir,
            prompt: if use_schema {
                prompt.clone()
            } else {
                format!("{prompt}\n\nReturn ONLY one JSON object (no prose, no code fences) matching this JSON Schema:\n{}", schemas::simulation())
            },
            schema: use_schema.then(schemas::simulation),
            sandbox: Sandbox::ReadOnly,
            web_search: attempt == 0,
            effort: None,
        };
        let v: Value = match codex.exec(req, Some(act)) {
            Ok(v) => v,
            // Some providers/models reject large strict schemas: fall back to
            // instruction-only JSON and rely on our own validation.
            Err(e) if use_schema && classify(&format!("{e:#}")) == ErrorKind::Other => {
                act("Retrying without the strict schema");
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
        let quality = sim::quality(&s);
        let last = attempt + 1 == ROUNDS;
        // Unplayable is never accepted; thin is accepted only on the last round.
        if errors.is_empty() && (quality.is_empty() || last) {
            pkg.save_sim(&s)?;
            return Ok(());
        }
        last_errors = errors.into_iter().chain(quality).collect();
        act(&format!(
            "Fixing {} problem(s) in the simulation",
            last_errors.len()
        ));
        prompt = format!("{base}{}", prompts::fix_simulation(&last_errors));
    }
    bail!(
        "simulation '{}' did not pass validation: {}",
        plan.title,
        last_errors.join("; ")
    )
}

// ----------------------------------------------------------------- run ----

/// Run (or resume) the download. Returns when every asset exists, the run is
/// cancelled, or it must stop (the course is then resumable).
pub fn run(pkg: &Package, cfg: &Config, cancel: &AtomicBool, emit: Emit<'_>) -> Result<()> {
    let codex = Codex::from_config(cfg);
    let machine = MachineProfile::detect(&pkg.dir);
    let mut course = pkg.course()?;
    if let Some(v) = codex.version() {
        course.generator = v;
    }

    // Persist every event: if anything goes wrong, the log says exactly what.
    let log = Mutex::new(
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(pkg.scratch().join("download.log"))
            .ok(),
    );
    let emit_logged = |e: Event| {
        if let Ok(mut guard) = log.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = writeln!(
                    f,
                    "{} [{}] {}/{} {} {} {}",
                    srs::now_unix(),
                    e.phase,
                    e.done,
                    e.total,
                    e.current,
                    e.activity,
                    e.message
                );
            }
        }
        emit(e)
    };
    let emit: Emit<'_> = &emit_logged;

    let stop = |course: &mut Course, phase: &str, msg: String| -> Result<()> {
        course.status = CourseStatus::Failed;
        course.status_detail = msg.clone();
        pkg.save_course(course)?;
        emit(Event {
            slug: course.slug.clone(),
            phase: phase.into(),
            message: msg,
            ..Default::default()
        });
        Ok(())
    };

    if course.modules.is_empty() {
        let slug = course.slug.clone();
        let act = |a: &str| {
            emit(Event {
                slug: slug.clone(),
                phase: "planning".into(),
                current: "Designing the course".into(),
                activity: a.into(),
                ..Default::default()
            })
        };
        let mut planned = course.clone();
        match with_retries("the course plan", cancel, &act, |_| {
            plan(&codex, pkg, &mut planned, &act)
        }) {
            Ok(()) => course = planned,
            Err(Ok(halt)) => return stop(&mut course, halt.phase, halt.message),
            Err(Err(e)) => {
                return stop(
                    &mut course,
                    "failed",
                    format!("Planning failed: {e:#}. Press Resume to try again."),
                )
            }
        }
    }
    course.status = CourseStatus::Downloading;
    course.status_detail.clear();
    pkg.save_course(&course)?;

    let digest = research_digest(pkg, &course, cfg, &machine, emit);
    let issues = if digest.trim().is_empty() {
        "(none collected; use your own knowledge of common real-world failures in this field, verified with web search)".to_string()
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
    let halt = Mutex::new(None::<Halt>);
    let halted = AtomicBool::new(false);
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
                if cancel.load(Ordering::SeqCst) || halted.load(Ordering::SeqCst) {
                    return;
                }
                let job = { queue.lock().unwrap_or_else(|e| e.into_inner()).pop_front() };
                let Some(job) = job else { return };
                let label = job.label(course_ref);
                progress(label.clone(), String::new());
                let act = |a: &str| progress(label.clone(), a.to_string());
                let result = with_retries(&label, cancel, &act, |_| match &job {
                    Job::Module(i) => {
                        build_module(&codex, pkg, course_ref, *i, &issues, &deck, &act)
                    }
                    Job::Sim(m, p) => build_sim(&codex, pkg, course_ref, *m, p, &issues, &act),
                });
                match result {
                    Ok(()) => {
                        done.fetch_add(1, Ordering::SeqCst);
                        progress(format!("Ready: {label}"), String::new());
                    }
                    Err(Ok(h)) => {
                        halted.store(true, Ordering::SeqCst);
                        let mut slot = halt.lock().unwrap_or_else(|e| e.into_inner());
                        if slot.is_none() {
                            *slot = Some(h);
                        }
                    }
                    Err(Err(e)) => errors
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(format!("{label}: {e:#}")),
                }
            });
        }
    });

    let errors = errors.into_inner().unwrap_or_default();
    let done = done.load(Ordering::SeqCst);
    if let Some(h) = halt.into_inner().unwrap_or_default() {
        return stop(&mut course, h.phase, h.message);
    }
    if cancel.load(Ordering::SeqCst) && done < total {
        return stop(
            &mut course,
            "paused",
            "Paused. Resume to finish the download.".into(),
        );
    }
    if !errors.is_empty() {
        return stop(
            &mut course,
            "failed",
            format!(
                "{} of {total} items could not be built after several attempts; everything else is ready to study. Press Resume to retry them. Details: {}",
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

    #[test]
    fn errors_are_classified() {
        assert_eq!(
            classify("Codex step 'plan' failed: Not logged in. Run codex login"),
            ErrorKind::Auth
        );
        assert_eq!(
            classify("You've hit your usage limit. Try again later."),
            ErrorKind::RateLimit
        );
        assert_eq!(classify("429 Too Many Requests"), ErrorKind::RateLimit);
        assert_eq!(
            classify("Codex step 'm1' timed out after 20 min"),
            ErrorKind::Transient
        );
        assert_eq!(
            classify("stream disconnected before completion"),
            ErrorKind::Transient
        );
        assert_eq!(classify("unexpected module shape"), ErrorKind::Other);
    }

    #[test]
    fn retries_then_succeeds_and_halts_on_auth() {
        std::env::set_var("GURU_RETRY_DELAY_MS", "0");
        let cancel = AtomicBool::new(false);
        let noop = |_: &str| {};
        let mut calls = 0;
        let r = with_retries("x", &cancel, &noop, |_| {
            calls += 1;
            if calls < 3 {
                bail!("connection reset by peer")
            }
            Ok(())
        });
        assert!(r.is_ok());
        assert_eq!(calls, 3);

        let mut calls = 0;
        let r = with_retries("x", &cancel, &noop, |_| {
            calls += 1;
            bail!("bad output")
        });
        assert!(matches!(r, Err(Err(_))));
        assert_eq!(calls, 4, "gives up after 4 attempts");

        let r = with_retries("x", &cancel, &noop, |_| bail!("401 Unauthorized"));
        assert!(matches!(
            r,
            Err(Ok(Halt {
                phase: "failed",
                ..
            }))
        ));

        let r = with_retries("x", &cancel, &noop, |_| bail!("usage limit reached"));
        assert!(matches!(
            r,
            Err(Ok(Halt {
                phase: "paused",
                ..
            }))
        ));
    }

    #[test]
    fn module_quality_gate() {
        let thin = Module {
            lesson: "## A\nshort".into(),
            ..Default::default()
        };
        let issues = module_issues(&thin, 45);
        assert!(issues.iter().any(|i| i.contains("too thin")));
        assert!(issues.iter().any(|i| i.contains("quiz")));
        let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../ui/demo");
        for course in std::fs::read_dir(root).unwrap() {
            let dir = course.unwrap().path();
            let c: crate::course::Course =
                crate::config::read_json(&dir.join("course.json")).unwrap();
            for info in &c.modules {
                let m: Module = crate::config::read_json(
                    &dir.join("modules").join(format!("{}.json", info.id)),
                )
                .unwrap();
                assert_eq!(
                    module_issues(&m, info.minutes),
                    Vec::<String>::new(),
                    "demo module {} meets the bar",
                    info.id
                );
            }
        }
    }
}
