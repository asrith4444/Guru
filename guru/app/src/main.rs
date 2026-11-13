//! Guru desktop app: a thin Tauri shell over guru-core.
//!
//! The window is the operating system's own web view (WebKit on macOS/Linux,
//! WebView2 on Windows), not a bundled browser, and the backend stays idle
//! unless a course is downloading. Everything a course needs to play lives on
//! disk; AI is only used when downloading a course or when the learner presses
//! one of the clearly marked "Ask AI" buttons.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::io::BufRead;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::sync::Mutex;

use guru_core::codex::Codex;
use guru_core::codex::ExecRequest;
use guru_core::codex::Sandbox;
use guru_core::config::Config;
use guru_core::course;
use guru_core::course::CourseStatus;
use guru_core::course::Package;
use guru_core::course::SimResult;
use guru_core::generator;
use guru_core::goal;
use guru_core::machine::MachineProfile;
use guru_core::prompts;
use guru_core::scope::Scope;
use guru_core::srs;
use serde::Serialize;
use serde_json::json;
use serde_json::Value;
use tauri::AppHandle;
use tauri::Emitter;
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    format!("{e:#}")
}

#[derive(Default)]
struct AppState {
    /// Running downloads: slug -> cancel flag.
    downloads: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

/// Run blocking work (Codex calls, file IO) off the UI thread.
async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(err)?
}

// ---------- status & settings ----------

#[tauri::command]
async fn app_status() -> CmdResult<Value> {
    blocking(|| {
        let cfg = Config::load();
        let codex = Codex::from_config(&cfg);
        let version = codex.version();
        let (logged_in, login_message) = if version.is_some() {
            codex.login_status()
        } else {
            (false, "Codex CLI not found".into())
        };
        let machine = MachineProfile::detect(&Package::root());
        Ok(json!({
            "codex_version": version,
            "codex_bin": codex.bin.display().to_string(),
            "logged_in": logged_in,
            "login_message": login_message,
            "machine": machine,
            "budget": machine.budget(),
            "config": cfg,
            "courses_dir": Package::root().display().to_string(),
        }))
    })
    .await
}

#[tauri::command]
fn save_settings(config: Config) -> CmdResult<()> {
    config.save().map_err(err)
}

#[tauri::command]
fn start_login(app: AppHandle) -> CmdResult<()> {
    let codex = Codex::from_config(&Config::load());
    let mut child = codex.spawn_login().map_err(err)?;
    let stderr = child.stderr.take();
    let stdout = child.stdout.take();
    for stream in [
        stderr.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
        stdout.map(|s| Box::new(s) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let app = app.clone();
        std::thread::spawn(move || {
            for line in std::io::BufReader::new(stream)
                .lines()
                .map_while(Result::ok)
            {
                if let Some(url) = line.split_whitespace().find(|w| w.starts_with("https://")) {
                    let _ = app.emit("login-url", url.to_string());
                }
            }
        });
    }
    std::thread::spawn(move || {
        let ok = child.wait().map(|s| s.success()).unwrap_or(false);
        let _ = app.emit("login-done", ok);
    });
    Ok(())
}

#[tauri::command]
fn open_url(url: String) -> CmdResult<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("only web links can be opened".into());
    }
    let (cmd, args): (&str, Vec<&str>) = if cfg!(target_os = "macos") {
        ("open", vec![])
    } else if cfg!(windows) {
        ("rundll32", vec!["url.dll,FileProtocolHandler"])
    } else {
        ("xdg-open", vec![])
    };
    std::process::Command::new(cmd)
        .args(args)
        .arg(&url)
        .spawn()
        .map_err(err)?;
    Ok(())
}

// ---------- new course ----------

#[tauri::command]
async fn scope_topic(topic: String, goal: String) -> CmdResult<Scope> {
    blocking(move || {
        let codex = Codex::from_config(&Config::load());
        match generator::scope(&codex, &topic, &goal) {
            Ok(s) => Ok(s),
            Err(e) => {
                let mut s = Scope::offline(&topic);
                s.note = format!(
                    "Estimated offline ({}).",
                    err(e).lines().next().unwrap_or("AI unavailable")
                );
                Ok(s)
            }
        }
    })
    .await
}

fn spawn_download(app: AppHandle, state: &AppState, pkg: Package) -> CmdResult<()> {
    let slug = pkg.course().map_err(err)?.slug;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut d = state.downloads.lock().map_err(err)?;
        if d.contains_key(&slug) {
            return Ok(());
        }
        d.insert(slug.clone(), cancel.clone());
    }
    std::thread::spawn(move || {
        let cfg = Config::load();
        let emit = |e: generator::Event| {
            let _ = app.emit("download", e);
        };
        if let Err(e) = generator::run(&pkg, &cfg, &cancel, &emit) {
            emit(generator::Event {
                slug: slug.clone(),
                phase: "failed".into(),
                message: err(e),
                ..Default::default()
            });
        }
        let state: State<AppState> = tauri::Manager::state(&app);
        let removed = state.downloads.lock().map(|mut d| d.remove(&slug));
        drop(removed);
    });
    Ok(())
}

#[tauri::command]
fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    request: generator::Request,
) -> CmdResult<String> {
    if request.topic.trim().is_empty() {
        return Err("Tell Guru what you want to learn.".into());
    }
    // Refuse up front rather than fail half-way.
    generator::preflight(&Codex::from_config(&Config::load())).map_err(err)?;
    let pkg = generator::create(&request).map_err(err)?;
    let slug = pkg.course().map_err(err)?.slug;
    spawn_download(app, &state, pkg)?;
    Ok(slug)
}

#[tauri::command]
fn resume_download(app: AppHandle, state: State<'_, AppState>, slug: String) -> CmdResult<()> {
    generator::preflight(&Codex::from_config(&Config::load())).map_err(err)?;
    let pkg = Package::open(&slug).map_err(err)?;
    spawn_download(app, &state, pkg)
}

// ---------- goals: "What's your goal?" → questions → roadmap ----------

#[tauri::command]
async fn clarify_goal(goal: String) -> CmdResult<goal::Clarification> {
    if goal.trim().len() < 3 {
        return Err("Tell Guru your goal in a few words.".into());
    }
    blocking(move || {
        let codex = Codex::from_config(&Config::load());
        generator::preflight(&codex).map_err(err)?;
        goal::clarify(&codex, &goal).map_err(err)
    })
    .await
}

#[tauri::command]
async fn plan_goal(goal: String, answers: Vec<goal::Answer>) -> CmdResult<goal::GoalPlan> {
    blocking(move || {
        let codex = Codex::from_config(&Config::load());
        goal::plan(&codex, &goal, answers).map_err(err)
    })
    .await
}

#[tauri::command]
fn list_goals() -> Vec<goal::GoalPlan> {
    goal::list()
}

#[tauri::command]
fn get_goal(slug: String) -> CmdResult<Value> {
    let plan = goal::load(&slug).map_err(err)?;
    // Progress per stage from its downloaded course, if any.
    let stages: Vec<Value> = plan
        .stages
        .iter()
        .map(|st| {
            let card = (!st.course_slug.is_empty())
                .then(|| Package::open(&st.course_slug).ok())
                .flatten()
                .and_then(|p| card_for(&p, false).ok());
            json!({ "stage": st, "card": card })
        })
        .collect();
    Ok(json!({ "plan": plan, "stages": stages }))
}

#[tauri::command]
fn delete_goal(slug: String) -> CmdResult<()> {
    goal::delete(&slug).map_err(err)
}

#[tauri::command]
fn download_stage(
    app: AppHandle,
    state: State<'_, AppState>,
    goal_slug: String,
    stage_id: String,
) -> CmdResult<String> {
    let plan = goal::load(&goal_slug).map_err(err)?;
    let stage = plan.stage(&stage_id).ok_or("unknown stage")?;
    if !stage.course_slug.is_empty() && Package::open(&stage.course_slug).is_ok() {
        return Ok(stage.course_slug.clone());
    }
    generator::preflight(&Codex::from_config(&Config::load())).map_err(err)?;
    let req = plan.request_for(&stage_id).map_err(err)?;
    let pkg = generator::create(&req).map_err(err)?;
    let slug = pkg.course().map_err(err)?.slug;
    goal::link_course(&goal_slug, &stage_id, &slug).map_err(err)?;
    spawn_download(app, &state, pkg)?;
    Ok(slug)
}

#[tauri::command]
fn pause_download(state: State<'_, AppState>, slug: String) -> CmdResult<()> {
    if let Some(flag) = state.downloads.lock().map_err(err)?.get(&slug) {
        flag.store(true, Ordering::SeqCst);
    }
    Ok(())
}

// ---------- library ----------

#[derive(Serialize)]
struct CourseCard {
    course: course::Course,
    mastery: f32,
    modules_ready: usize,
    sims_ready: usize,
    sims_total: usize,
    lessons_read: usize,
    due_cards: usize,
    downloading: bool,
    size_mb: f32,
}

fn card_for(pkg: &Package, downloading: bool) -> CmdResult<CourseCard> {
    let c = pkg.course().map_err(err)?;
    let p = pkg.progress();
    let today = srs::today();
    let sims = c.all_sims();
    Ok(CourseCard {
        mastery: course::overall_mastery(&c, &p),
        modules_ready: c
            .modules
            .iter()
            .filter(|m| pkg.module_path(&m.id).map(|p| p.exists()).unwrap_or(false))
            .count(),
        sims_ready: sims
            .iter()
            .filter(|s| pkg.sim_path(&s.id).map(|p| p.exists()).unwrap_or(false))
            .count(),
        sims_total: sims.len(),
        lessons_read: p.lessons_read.len(),
        due_cards: pkg.deck().cards.iter().filter(|c| c.is_due(today)).count(),
        downloading,
        size_mb: pkg.size_bytes() as f32 / 1_048_576.0,
        course: c,
    })
}

#[tauri::command]
async fn list_courses(state: State<'_, AppState>) -> CmdResult<Vec<CourseCard>> {
    let running: Vec<String> = state
        .downloads
        .lock()
        .map_err(err)?
        .keys()
        .cloned()
        .collect();
    blocking(move || {
        Ok(Package::list()
            .iter()
            .filter_map(|p| {
                let slug = p.dir.file_name()?.to_string_lossy().into_owned();
                card_for(p, running.contains(&slug)).ok()
            })
            .collect())
    })
    .await
}

#[tauri::command]
fn get_course(state: State<'_, AppState>, slug: String) -> CmdResult<Value> {
    let pkg = Package::open(&slug).map_err(err)?;
    let downloading = state.downloads.lock().map_err(err)?.contains_key(&slug);
    let card = card_for(&pkg, downloading)?;
    let ready_modules: Vec<String> = card
        .course
        .modules
        .iter()
        .filter(|m| pkg.module_path(&m.id).map(|p| p.exists()).unwrap_or(false))
        .map(|m| m.id.clone())
        .collect();
    let ready_sims: Vec<String> = card
        .course
        .all_sims()
        .iter()
        .filter(|s| pkg.sim_path(&s.id).map(|p| p.exists()).unwrap_or(false))
        .map(|s| s.id.clone())
        .collect();
    Ok(json!({
        "card": card,
        "progress": pkg.progress(),
        "ready_modules": ready_modules,
        "ready_sims": ready_sims,
    }))
}

#[tauri::command]
fn get_module(slug: String, id: String) -> CmdResult<course::Module> {
    Package::open(&slug)
        .and_then(|p| p.module(&id))
        .map_err(err)
}

#[tauri::command]
fn get_sim(slug: String, id: String) -> CmdResult<Value> {
    let pkg = Package::open(&slug).map_err(err)?;
    let path = pkg.sim_path(&id).map_err(err)?;
    guru_core::config::read_json(&path).map_err(err)
}

#[tauri::command]
fn delete_course(state: State<'_, AppState>, slug: String) -> CmdResult<()> {
    if state.downloads.lock().map_err(err)?.contains_key(&slug) {
        return Err("Pause the download before deleting this course.".into());
    }
    Package::open(&slug).and_then(Package::delete).map_err(err)
}

/// The built-in demo courses ship inside the binary: (slug, [(path, contents)]).
const DEMOS: &[(&str, &[(&str, &str)])] = &[
    (
        "demo-incident-response",
        &[
            (
                "course.json",
                include_str!("../../ui/demo/incident-response/course.json"),
            ),
            (
                "modules/m1.json",
                include_str!("../../ui/demo/incident-response/modules/m1.json"),
            ),
            (
                "modules/m2.json",
                include_str!("../../ui/demo/incident-response/modules/m2.json"),
            ),
            (
                "sims/infected-web-01.json",
                include_str!("../../ui/demo/incident-response/sims/infected-web-01.json"),
            ),
            (
                "sims/grafana-alerting.json",
                include_str!("../../ui/demo/incident-response/sims/grafana-alerting.json"),
            ),
        ],
    ),
    (
        "demo-electrical-fault-finding",
        &[
            (
                "course.json",
                include_str!("../../ui/demo/electrical/course.json"),
            ),
            (
                "modules/e1.json",
                include_str!("../../ui/demo/electrical/modules/e1.json"),
            ),
            (
                "sims/rcd-kitchen.json",
                include_str!("../../ui/demo/electrical/sims/rcd-kitchen.json"),
            ),
        ],
    ),
];

#[tauri::command]
fn install_demo(name: Option<String>) -> CmdResult<String> {
    let (slug, files) = DEMOS
        .iter()
        .find(|(slug, _)| name.as_deref().map(|n| slug.contains(n)).unwrap_or(true))
        .copied()
        .ok_or("unknown demo")?;
    let pkg = Package::create(slug).map_err(err)?;
    for (path, text) in files {
        let dest = pkg.dir.join(path);
        if !dest.exists() {
            std::fs::write(&dest, text).map_err(err)?;
        }
    }
    let mut c = pkg.course().map_err(err)?;
    if c.created_unix == 0 {
        c.created_unix = srs::now_unix();
        c.status = CourseStatus::Ready;
        pkg.save_course(&c).map_err(err)?;
        let today = srs::today();
        let mut deck = pkg.deck();
        for m in &c.modules {
            for (i, card) in pkg.module(&m.id).map_err(err)?.cards.iter().enumerate() {
                deck.cards.push(srs::Card::new(
                    format!("{}c{}", m.id, i + 1),
                    card.skill_id.clone(),
                    card.front.clone(),
                    card.back.clone(),
                    today,
                ));
            }
        }
        pkg.save_deck(&deck).map_err(err)?;
    }
    Ok(slug.into())
}

// ---------- studying (all local, no AI) ----------

#[tauri::command]
fn mark_read(slug: String, module: String, seconds: u64) -> CmdResult<()> {
    let pkg = Package::open(&slug).map_err(err)?;
    let mut p = pkg.progress();
    p.mark_read(&module);
    p.seconds_studied += seconds.min(4 * 3600);
    pkg.save_progress(&p).map_err(err)
}

#[tauri::command]
fn record_quiz(
    slug: String,
    module: String,
    per_skill: Vec<(String, f32)>,
    score: f32,
) -> CmdResult<Value> {
    let pkg = Package::open(&slug).map_err(err)?;
    let mut p = pkg.progress();
    p.record_quiz(&module, &per_skill, score);
    pkg.save_progress(&p).map_err(err)?;
    Ok(json!(p.mastery))
}

#[tauri::command]
fn record_sim(
    slug: String,
    sim_id: String,
    skills: Vec<String>,
    result: SimResult,
) -> CmdResult<()> {
    let pkg = Package::open(&slug).map_err(err)?;
    let mut p = pkg.progress();
    p.seconds_studied += u64::from(result.seconds.min(4 * 3600));
    p.record_sim(&sim_id, &skills, result);
    pkg.save_progress(&p).map_err(err)
}

#[tauri::command]
fn due_cards(slug: String, limit: usize) -> CmdResult<Vec<srs::Card>> {
    let pkg = Package::open(&slug).map_err(err)?;
    let deck = pkg.deck();
    let mut rng = srs::Rng::seeded();
    let order = srs::interleave_due(&deck.cards, srs::today(), limit.clamp(1, 100), &mut rng);
    Ok(order.into_iter().map(|i| deck.cards[i].clone()).collect())
}

#[tauri::command]
fn grade_card(slug: String, id: String, grade: u8) -> CmdResult<bool> {
    let pkg = Package::open(&slug).map_err(err)?;
    let mut deck = pkg.deck();
    let g = match grade {
        1 => srs::Grade::Again,
        2 => srs::Grade::Hard,
        4 => srs::Grade::Easy,
        _ => srs::Grade::Good,
    };
    let again = match deck.cards.iter_mut().find(|c| c.id == id) {
        Some(c) => c.review(g, srs::today()),
        None => return Err("card not found".into()),
    };
    pkg.save_deck(&deck).map_err(err)?;
    Ok(again)
}

// ---------- optional AI help (explicit, opt-in) ----------

#[tauri::command]
async fn ask_guru(slug: String, context: String, question: String) -> CmdResult<String> {
    blocking(move || {
        let pkg = Package::open(&slug).map_err(err)?;
        let c = pkg.course().map_err(err)?;
        let codex = Codex::from_config(&Config::load());
        let v = codex
            .exec(
                ExecRequest {
                    label: "ask",
                    cwd: &pkg.scratch(),
                    prompt: prompts::ask(&c, &context, &question),
                    schema: None,
                    sandbox: Sandbox::ReadOnly,
                    web_search: true,
                    effort: Some("low"),
                },
                None,
            )
            .map_err(err)?;
        Ok(v.as_str().unwrap_or_default().to_string())
    })
    .await
}

#[tauri::command]
async fn ai_command(
    slug: String,
    sim_id: String,
    hostname: String,
    history: String,
    command: String,
) -> CmdResult<String> {
    blocking(move || {
        let pkg = Package::open(&slug).map_err(err)?;
        let mut sim: Value =
            guru_core::config::read_json(&pkg.sim_path(&sim_id).map_err(err)?).map_err(err)?;
        // Never leak the answers into the improvised terminal.
        if let Some(o) = sim.as_object_mut() {
            o.remove("objectives");
            o.remove("debrief");
            o.remove("hints");
        }
        let text = serde_json::to_string(&sim).map_err(err)?;
        let text: String = text.chars().take(60_000).collect();
        let codex = Codex::from_config(&Config::load());
        let v = codex
            .exec(
                ExecRequest {
                    label: "terminal",
                    cwd: &pkg.scratch(),
                    prompt: prompts::simulate_command(&text, &hostname, &history, &command),
                    schema: None,
                    sandbox: Sandbox::ReadOnly,
                    web_search: false,
                    effort: Some("low"),
                },
                None,
            )
            .map_err(err)?;
        Ok(v.as_str().unwrap_or_default().trim_end().to_string())
    })
    .await
}

fn main() {
    // WebKitGTK's GPU compositing roughly triples the web process's memory
    // (measured: ~250 MB vs ~90 MB) and Guru's UI needs no heavy animation.
    // Respect an explicit choice by the user.
    if cfg!(target_os = "linux") && std::env::var_os("WEBKIT_DISABLE_COMPOSITING_MODE").is_none() {
        std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    }
    // Downloads cut off by a crash or shutdown become resumable, never stuck.
    generator::recover_interrupted();
    tauri::Builder::default()
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            app_status,
            save_settings,
            start_login,
            open_url,
            scope_topic,
            start_download,
            resume_download,
            pause_download,
            list_courses,
            get_course,
            get_module,
            get_sim,
            delete_course,
            install_demo,
            clarify_goal,
            plan_goal,
            list_goals,
            get_goal,
            delete_goal,
            download_stage,
            mark_read,
            record_quiz,
            record_sim,
            due_cards,
            grade_card,
            ask_guru,
            ai_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Guru");
}
