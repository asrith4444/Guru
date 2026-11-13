//! Smaller commands: doctor, status, review, ask, tutor, check, incident,
//! clean, report, list/use, config.

use anyhow::Context;
use anyhow::Result;
use serde_json::Value;

use crate::codex::Codex;
use crate::codex::ExecRequest;
use crate::codex::Sandbox;
use crate::config::guru_home;
use crate::config::Config;
use crate::config::Store;
use crate::learn;
use crate::machine::MachineProfile;
use crate::model::HourStatus;
use crate::model::Issue;
use crate::prompts;
use crate::schemas;
use crate::session;
use crate::srs;
use crate::ui;

pub fn doctor(cfg: &Config) -> Result<()> {
    let codex = Codex::from_config(cfg);
    ui::header("Guru doctor");
    match codex.version() {
        Some(v) => ui::ok(&format!("Codex CLI: {v} ({})", codex.bin.display())),
        None => ui::warn(&format!(
            "Codex CLI not found at '{}'. Install: npm i -g @openai/codex (or brew install --cask codex), \
             or point Guru at a build from this repo: guru config codex_bin <path>",
            codex.bin.display()
        )),
    }
    if codex.available() {
        let (ok, msg) = codex.login_status();
        if ok {
            ui::ok(&format!("Auth: {msg}"));
        } else {
            ui::warn(&format!(
                "Auth: {msg} → run `guru login` (Sign in with ChatGPT)"
            ));
        }
    }
    let home = guru_home();
    let m = MachineProfile::detect(&home);
    let b = m.budget();
    ui::ok(&format!(
        "Machine: {} {}, {} threads, RAM {} MB total / {} MB free, disk free {}",
        m.os,
        m.arch,
        m.cpus,
        m.total_ram_mb,
        m.avail_ram_mb,
        m.free_disk_mb
            .map(|d| format!("{d} MB"))
            .unwrap_or_else(|| "unknown".into())
    ));
    ui::ok(&format!(
        "Lab budget: {:?} tier · ≤{} MB RAM · ≤{} MB disk · ≤{} CPU · containers {} · local cluster {}",
        b.tier,
        b.max_ram_mb,
        b.max_disk_mb,
        b.max_cpus,
        yes_no(b.containers),
        yes_no(b.local_cluster)
    ));
    ui::info(&format!(
        "Tools found: {}",
        if m.tools.is_empty() {
            "none".into()
        } else {
            m.tools.join(", ")
        }
    ));
    if !m.has("curl") {
        ui::warn(
            "curl not found: Stack Exchange mining is disabled (Codex web research still works).",
        );
    }
    ui::info(&format!("Data: {}", home.display()));
    Ok(())
}

fn yes_no(b: bool) -> &'static str {
    if b {
        "yes"
    } else {
        "no"
    }
}

pub fn status() -> Result<()> {
    let store = Store::active()?;
    let course = store.course()?;
    let deck = store.deck();
    let today = srs::today();
    let journal = store.journal();
    ui::header(&course.title);
    println!("{} {}", ui::bold("Target:"), course.target_performance);
    println!(
        "{} {}/{} hours · {} cards due · overall mastery {} {:.0}%",
        ui::bold("Progress:"),
        course.hours_done(),
        course.hours.len(),
        deck.cards.iter().filter(|c| c.is_due(today)).count(),
        ui::bar(course.weighted_mastery(), 16),
        course.weighted_mastery() * 100.0
    );
    println!();
    for h in &course.hours {
        let mark = match h.status {
            HourStatus::Done => ui::green("●"),
            HourStatus::InProgress => ui::yellow("◐"),
            HourStatus::Planned => ui::dim("○"),
        };
        let score = h
            .score
            .map(|s| format!(" {:.0}%", s * 100.0))
            .unwrap_or_default();
        println!("  {mark} Hour {} {}{}", h.hour, h.title, ui::dim(&score));
    }
    println!("\n{}", ui::bold("Skills (Pareto-weighted):"));
    let mut skills = course.skills.clone();
    skills.sort_by_key(|s| (s.hour, std::cmp::Reverse(s.pareto_weight)));
    for s in &skills {
        println!(
            "  {} {:>3.0}%  {} {}",
            ui::bar(s.mastery, 12),
            s.mastery * 100.0,
            s.name,
            ui::dim(&format!("w{} · h{}", s.pareto_weight, s.hour))
        );
    }
    if let Some(last) = journal.last() {
        let over = last.overconfidence;
        println!(
            "\n{} hour {} · {:.0}% · calibration {}",
            ui::bold("Last session:"),
            last.hour,
            last.hour_score * 100.0,
            if over > 0.15 {
                ui::yellow("overconfident")
            } else if over < -0.15 {
                ui::cyan("underconfident")
            } else {
                ui::green("good")
            }
        );
    }
    if !course.remediation.is_empty() {
        println!(
            "{} {}",
            ui::bold("Repairing next:"),
            course.remediation.join(", ")
        );
    }
    println!(
        "\n{}",
        ui::dim(&format!("Course files: {}", store.dir.display()))
    );
    Ok(())
}

pub fn review() -> Result<()> {
    let store = Store::active()?;
    ui::header("Spaced recall");
    let n = session::review_block(&store, 30, None)?;
    if n == 0 {
        let next = store.deck().cards.iter().map(|c| c.due_day).min();
        match next {
            Some(d) => ui::ok(&format!(
                "Nothing due. Next card due in {} day(s).",
                (d - srs::today()).max(1)
            )),
            None => ui::ok("No cards yet: they are created as you complete hours."),
        }
    } else {
        ui::ok(&format!("Reviewed {n} cards."));
    }
    Ok(())
}

pub fn ask(cfg: &Config, question: &str) -> Result<()> {
    let store = Store::active()?;
    let course = store.course()?;
    let codex = Codex::from_config(cfg);
    learn::preflight(&codex)?;
    let v = codex.exec(ExecRequest {
        label: "ask",
        cwd: &store.dir,
        prompt: prompts::ask(&course, question),
        schema: None,
        sandbox: Sandbox::ReadOnly,
        web_search: true,
        quiet: false,
        images: vec![],
    })?;
    println!("\n{}", v.as_str().unwrap_or_default());
    Ok(())
}

pub fn tutor(cfg: &Config) -> Result<()> {
    let store = Store::active()?;
    let course = store.course()?;
    let codex = Codex::from_config(cfg);
    learn::preflight(&codex)?;
    let hour = course
        .next_hour()
        .map(|h| h.hour)
        .unwrap_or(course.hours.len() as u32);
    codex.interactive(
        &store.dir,
        &format!("I'm on hour {hour} of my Guru course. Start by asking me what I'm working on."),
        true,
    )
}

fn current_pack(store: &Store) -> Result<crate::model::HourPack> {
    let course = store.course()?;
    let hour = store
        .progress()
        .map(|p| p.hour)
        .or_else(|| course.next_hour().map(|h| h.hour))
        .unwrap_or(course.hours.len() as u32);
    store
        .pack(hour)
        .or_else(|| (1..hour).rev().find_map(|h| store.pack(h)))
        .context("no lab prepared yet; run `guru next`")
}

pub fn check(cfg: &Config) -> Result<()> {
    let store = Store::active()?;
    let course = store.course()?;
    let pack = current_pack(&store)?;
    let codex = Codex::from_config(cfg);
    learn::preflight(&codex)?;
    if let Some(r) = session::check(&store, &course, &pack, &codex, false)? {
        if let Some(mut p) = store.progress().filter(|p| p.hour == pack.hour) {
            p.lab_result = Some(r);
            store.save_progress(&p)?;
        }
    }
    Ok(())
}

/// Extra break-fix practice built from the real-world issue bank.
pub fn incident(cfg: &Config) -> Result<()> {
    let store = Store::active()?;
    let course = store.course()?;
    let codex = Codex::from_config(cfg);
    learn::preflight(&codex)?;
    let machine = MachineProfile::detect(&store.dir);
    let mut issues: Vec<Issue> = store.issues();
    // Prefer issues for skills already taught, unused first, weakest skill first.
    let taught = course.next_hour().map(|h| h.hour).unwrap_or(u32::MAX);
    let pick = issues
        .iter()
        .enumerate()
        .filter(|(_, i)| {
            course
                .skill(&i.skill_id)
                .map(|s| s.hour <= taught)
                .unwrap_or(true)
        })
        .min_by(|(_, a), (_, b)| {
            let ma = course.skill(&a.skill_id).map(|s| s.mastery).unwrap_or(0.5);
            let mb = course.skill(&b.skill_id).map(|s| s.mastery).unwrap_or(0.5);
            (a.used, ma)
                .partial_cmp(&(b.used, mb))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(i, _)| i);
    let n = std::fs::read_dir(store.dir.join("lab"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.file_name().to_string_lossy().starts_with("incident-"))
                .count()
        })
        .unwrap_or(0)
        + 1;
    let dir_rel = format!("lab/incident-{n}");
    ui::info("Building an incident from real-world failures…");
    let v: Value = codex.exec(ExecRequest {
        label: "incident",
        cwd: &store.dir,
        prompt: prompts::incident(&course, pick.map(|i| &issues[i]), &machine, &dir_rel),
        schema: Some(schemas::incident()),
        sandbox: Sandbox::WorkspaceWrite,
        web_search: true,
        quiet: false,
        images: vec![],
    })?;
    if let Some(i) = pick {
        issues[i].used = true;
        store.save_issues(&issues)?;
    }
    ui::header(&format!("Incident: {}", v["title"].as_str().unwrap_or("")));
    println!("{}", v["briefing"].as_str().unwrap_or(""));
    println!(
        "\n{} {}",
        ui::bold("Directory:"),
        store
            .dir
            .join(v["directory"].as_str().unwrap_or(&dir_rel))
            .display()
    );
    let hints: Vec<String> = v["hints"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|h| h.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    std::fs::write(
        store.dir.join(&dir_rel).join(".hints.json"),
        serde_json::to_vec_pretty(&hints)?,
    )
    .ok();
    ui::info("Diagnose from symptoms first: what do you observe, what could cause it, how will you test each hypothesis?");
    for (i, h) in hints.iter().enumerate() {
        if !ui::confirm(&format!("Reveal hint {}/{}?", i + 1, hints.len()), false) {
            break;
        }
        println!("  {h}");
    }
    Ok(())
}

/// Stop all labs and report disk use. `deep` also removes build caches.
pub fn clean(deep: bool) -> Result<()> {
    let store = Store::active()?;
    let course = store.course()?;
    for h in &course.hours {
        if let Some(pack) = store.pack(h.hour) {
            session::teardown(&store, &pack, false)?;
        }
    }
    if deep {
        let mut removed = 0u64;
        for name in [
            "target",
            "node_modules",
            "build",
            ".gradle",
            "__pycache__",
            ".venv",
            "dist",
        ] {
            removed += remove_named_dirs(&store.dir.join("lab"), name)?;
            removed += remove_named_dirs(&store.dir.join("capstone"), name)?;
        }
        ui::ok(&format!(
            "Removed {} MB of build output and caches.",
            removed / 1_048_576
        ));
    }
    ui::ok(&format!(
        "Course uses {} MB on disk.",
        dir_size(&store.dir) / 1_048_576
    ));
    Ok(())
}

fn remove_named_dirs(root: &std::path::Path, name: &str) -> Result<u64> {
    let mut freed = 0;
    let Ok(rd) = std::fs::read_dir(root) else {
        return Ok(0);
    };
    for e in rd.filter_map(|e| e.ok()) {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if !ft.is_dir() || ft.is_symlink() {
            continue;
        }
        if e.file_name() == name {
            freed += dir_size(&p);
            std::fs::remove_dir_all(&p)?;
        } else {
            freed += remove_named_dirs(&p, name)?;
        }
    }
    Ok(freed)
}

fn dir_size(p: &std::path::Path) -> u64 {
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

pub fn report(cfg: &Config) -> Result<()> {
    let store = Store::active()?;
    let course = store.course()?;
    let codex = Codex::from_config(cfg);
    learn::preflight(&codex)?;
    let journal = std::fs::read_to_string(store.dir.join("journal.jsonl")).unwrap_or_default();
    ui::info("Writing your report…");
    let v = codex.exec(ExecRequest {
        label: "report",
        cwd: &store.dir,
        prompt: prompts::report(&course, &journal),
        schema: None,
        sandbox: Sandbox::WorkspaceWrite,
        web_search: false,
        quiet: false,
        images: vec![],
    })?;
    println!("\n{}", v.as_str().unwrap_or_default());
    let report = store.dir.join("REPORT.md");
    if report.exists() {
        ui::ok(&format!("Full report: {}", report.display()));
    }
    Ok(())
}

pub fn list() -> Result<()> {
    let active = std::fs::read_to_string(guru_home().join("active")).unwrap_or_default();
    let courses = Store::list();
    if courses.is_empty() {
        ui::info("No courses yet. Start one: guru learn \"Spring Boot\"");
    }
    for slug in courses {
        let store = Store::open(&slug)?;
        let Ok(c) = store.course() else { continue };
        let mark = if active.trim() == slug {
            ui::green("▶")
        } else {
            " ".into()
        };
        println!(
            "{mark} {:<28} {} {}/{} hours · {:.0}% mastery",
            slug,
            ui::dim(&c.title),
            c.hours_done(),
            c.hours.len(),
            c.weighted_mastery() * 100.0
        );
    }
    Ok(())
}

pub fn use_course(slug: &str) -> Result<()> {
    Store::open(slug)?;
    Store::set_active(slug)?;
    ui::ok(&format!("Active course: {slug}"));
    Ok(())
}

pub fn plan() -> Result<()> {
    let store = Store::active()?;
    let course = store.course()?;
    learn::show_plan(&course);
    for h in &course.hours {
        println!("\n{}", ui::bold(&format!("Hour {} · {}", h.hour, h.title)));
        for o in &h.objectives {
            println!("  • {o}");
        }
        println!("  {} {}", ui::dim("capstone:"), h.capstone_slice);
    }
    Ok(())
}

pub fn config(key: Option<String>, value: Option<String>) -> Result<()> {
    let mut cfg = Config::load();
    match (key, value) {
        (Some(k), Some(v)) => {
            cfg.set(&k, &v)?;
            cfg.save()?;
            ui::ok(&format!("{k} = {v}"));
        }
        _ => println!("{}", serde_json::to_string_pretty(&cfg)?),
    }
    Ok(())
}
