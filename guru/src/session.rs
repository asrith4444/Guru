//! The Guru Loop: one focused hour.
//!
//!   Recall   (~5 min)  spaced, interleaved retrieval of earlier material
//!   Read     (~15 min) predict first, then a tight reading-first lesson
//!   Practice (~25 min) hands-on lab: completion -> independent -> break-fix -> capstone
//!   Prove    (~10 min) lab check, exit quiz with confidence, explain-it-simply
//!   Reflect  (~5 min)  mastery update, calibration, cleanup, what's next
//!
//! While the learner does the Recall block (no AI needed), Codex prepares the
//! lesson and lab in the background, so the hour never starts with a wait.

use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

use crate::codex::Codex;
use crate::codex::ExecRequest;
use crate::codex::Sandbox;
use crate::config::Config;
use crate::config::Store;
use crate::learn;
use crate::machine::MachineProfile;
use crate::model::Course;
use crate::model::HourPack;
use crate::model::HourStatus;
use crate::model::JournalEntry;
use crate::model::NewCard;
use crate::model::Phase;
use crate::model::Progress;
use crate::model::QuizQuestion;
use crate::prompts;
use crate::schemas;
use crate::srs;
use crate::srs::Card;
use crate::srs::Grade;
use crate::ui;

/// Phase lengths for a 60-minute hour; scaled to the configured length.
fn minutes(course: &Course, base: u32) -> u32 {
    (base * course.minutes_per_hour / 60).max(1)
}

pub fn run(cfg: &Config) -> Result<()> {
    let store = Store::active()?;
    let mut course = store.course()?;
    let codex = Codex::from_config(cfg);
    learn::preflight(&codex)?;

    let Some(hour_plan) = course.next_hour().cloned() else {
        ui::ok("All hours are complete. Run `guru report` for your final report, `guru review` to keep memories fresh.");
        return Ok(());
    };
    let hour = hour_plan.hour;
    let machine = MachineProfile::detect(&store.dir);
    let mut progress = store
        .progress()
        .filter(|p| p.hour == hour)
        .unwrap_or_else(|| Progress {
            hour,
            phase: Phase::Recall,
            started_unix: srs::now_unix(),
            ..Default::default()
        });

    spacing_advice(&store);
    if let Some(h) = course.hour_mut(hour) {
        h.status = HourStatus::InProgress;
    }
    store.save_course(&course)?;
    store.save_progress(&progress)?;

    ui::header(&format!(
        "Hour {hour}/{} · {}",
        course.hours.len(),
        hour_plan.title
    ));
    for o in &hour_plan.objectives {
        println!("  • {o}");
    }

    // Kick off lesson + lab generation in the background if needed.
    let prep = if store.pack(hour).is_none() {
        let codex = codex.clone();
        let issues = store.issues();
        let course_c = course.clone();
        let hp = hour_plan.clone();
        let m = machine.clone();
        let dir = store.dir.clone();
        Some(std::thread::spawn(move || -> Result<Value> {
            let lab_rel = format!("lab/hour-{}", hp.hour);
            let lesson_rel = format!("lessons/hour-{}.md", hp.hour);
            codex.exec(ExecRequest {
                label: &format!("hour-{}", hp.hour),
                cwd: &dir,
                prompt: prompts::hour_pack(&course_c, &hp, &issues, &m, &lab_rel, &lesson_rel),
                schema: Some(schemas::hour_pack()),
                sandbox: Sandbox::WorkspaceWrite,
                web_search: true,
                quiet: true,
                images: vec![],
            })
        }))
    } else {
        None
    };

    if progress.phase <= Phase::Recall {
        ui::phase(
            "Recall",
            minutes(&course, 5),
            "Pull earlier material out of memory before adding new material. Effortful recall is what makes it stick.",
        );
        let reviewed = review_block(
            &store,
            20,
            Some(Duration::from_secs(u64::from(minutes(&course, 5)) * 60)),
        )?;
        if reviewed == 0 {
            if hour == 1 {
                ui::info("Nothing to recall yet: hour 1 builds the first memories. Your pretest answers already primed you.");
            } else {
                ui::info("No cards due right now. Quick self-check: without looking, list the 3 most important ideas from last hour.");
                let _ = ui::ask_multiline("Your 3 ideas:");
            }
        }
    }

    let pack = match prep {
        Some(handle) => {
            ui::info(&ui::dim(
                "\nFinishing your lesson and lab (built for your machine)…",
            ));
            let v = handle
                .join()
                .map_err(|_| anyhow::anyhow!("lesson preparation crashed"))??;
            let pack = install_pack(&store, &mut course, hour, v)?;
            ui::ok("Lesson and lab are ready.");
            pack
        }
        None => store.pack(hour).context("hour pack missing")?,
    };
    if progress.phase < Phase::Read {
        progress.phase = Phase::Read;
        store.save_progress(&progress)?;
    }

    if progress.phase == Phase::Read {
        ui::phase(
            "Read",
            minutes(&course, 15),
            "First commit to a prediction (it focuses your reading), then read actively: pause at every 'Pause and predict'.",
        );
        println!("{} {}", ui::bold("Predict:"), pack.predict_question);
        progress.prediction = ui::ask("Your best guess ›");
        let lesson = lesson_path(&store, &pack);
        ui::info(&ui::dim(&format!("Opening {} …", lesson.display())));
        ui::read_file(&lesson);
        if !progress.prediction.trim().is_empty() {
            println!(
                "\n{} you predicted “{}”. Was it right? What did the lesson change?",
                ui::bold("Check your prediction:"),
                progress.prediction
            );
        }
        progress.phase = Phase::Practice;
        store.save_progress(&progress)?;
    }

    if progress.phase == Phase::Practice {
        ui::phase(
            "Practice",
            minutes(&course, 25),
            "Work through the tasks in order. Struggle a little before taking a hint: productive struggle builds the skill.",
        );
        let finished = practice_console(&store, &course, &pack, &codex, &machine, &mut progress)?;
        if !finished {
            ui::info("Progress saved. Run `guru next` to continue exactly where you left off.");
            return Ok(());
        }
        progress.phase = Phase::Prove;
        store.save_progress(&progress)?;
    }

    if progress.phase == Phase::Prove {
        ui::phase(
            "Prove",
            minutes(&course, 10),
            "Answer from memory and rate your confidence honestly; Guru tracks whether your confidence matches reality.",
        );
        prove(&store, &mut course, &pack, &codex, &mut progress)?;
        progress.phase = Phase::Done;
        store.save_progress(&progress)?;
    }

    // Reflect / close.
    ui::phase(
        "Reflect",
        minutes(&course, 5),
        "Close the loop: what changed, what's next, and free the machine.",
    );
    teardown(&store, &pack, true)?;
    let course = store.course()?;
    if let Some(next) = course.next_hour() {
        println!(
            "\nNext: {} {}",
            ui::cyan(&format!("Hour {}", next.hour)),
            next.title
        );
        ui::info(
            "Best results: come back tomorrow (sleep consolidates today's learning). \
             `guru review` any time for 5 minutes of spaced recall.",
        );
    } else {
        ui::ok("That was the final hour. Run `guru report` for your end-of-course report.");
    }
    std::fs::write(
        store.dir.join("AGENTS.md"),
        prompts::agents_md(
            &course,
            &machine,
            course.next_hour().map(|h| h.hour).unwrap_or(hour),
        ),
    )?;
    store.clear_progress();
    Ok(())
}

fn spacing_advice(store: &Store) {
    let today = srs::today();
    let today_count = store.journal().iter().filter(|e| e.day == today).count();
    if today_count >= 2 {
        ui::warn(&format!(
            "You've already done {today_count} hours today. That works, but spacing sessions across days \
             roughly doubles long-term retention. Consider `guru review` now and the next hour tomorrow."
        ));
    }
}

#[derive(Deserialize)]
struct RawPack {
    lesson_file: String,
    predict_question: String,
    tasks: Vec<crate::model::Task>,
    setup: crate::model::SetupInfo,
    check_script: String,
    cards: Vec<NewCard>,
    exit_quiz: Vec<QuizQuestion>,
    feynman_prompt: String,
    used_issue_ids: Vec<String>,
}

fn install_pack(store: &Store, course: &mut Course, hour: u32, v: Value) -> Result<HourPack> {
    let raw: RawPack = serde_json::from_value(v).context("unexpected lesson pack shape")?;
    let pack = HourPack {
        hour,
        lesson_file: raw.lesson_file,
        predict_question: raw.predict_question,
        tasks: raw.tasks,
        setup: raw.setup,
        check_script: raw.check_script,
        cards: raw.cards,
        exit_quiz: raw.exit_quiz,
        feynman_prompt: raw.feynman_prompt,
    };
    store.save_pack(&pack)?;
    add_cards(store, &pack.cards, &format!("h{hour}"))?;
    if !raw.used_issue_ids.is_empty() {
        let mut issues = store.issues();
        for i in issues.iter_mut() {
            if raw.used_issue_ids.contains(&i.id) {
                i.used = true;
            }
        }
        store.save_issues(&issues)?;
    }
    // The plan is consumed: remediation (if any) has been built into this hour.
    course.remediation.clear();
    store.save_course(course)?;
    Ok(pack)
}

pub fn add_cards(store: &Store, cards: &[NewCard], prefix: &str) -> Result<usize> {
    let mut deck = store.deck();
    let today = srs::today();
    let mut added = 0;
    for (i, c) in cards.iter().enumerate() {
        if c.front.trim().is_empty() || deck.cards.iter().any(|d| d.front == c.front) {
            continue;
        }
        deck.cards.push(Card::new(
            format!("{prefix}-{}-{i}", deck.cards.len()),
            c.skill_id.clone(),
            c.front.clone(),
            c.back.clone(),
            // New cards are first retrieved in the *next* session, not right after reading.
            today + 1,
        ));
        added += 1;
    }
    store.save_deck(&deck)?;
    Ok(added)
}

/// Resolve a path Codex returned, refusing anything outside the course directory.
fn course_path(store: &Store, rel: &str) -> Option<PathBuf> {
    let rel = rel.trim();
    if rel.is_empty() {
        return None;
    }
    let p = Path::new(rel);
    let p = if p.is_absolute() {
        p.to_path_buf()
    } else {
        store.dir.join(p)
    };
    if p.components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
        || !p.starts_with(&store.dir)
    {
        return None;
    }
    p.exists().then_some(p)
}

fn lesson_path(store: &Store, pack: &HourPack) -> PathBuf {
    course_path(store, &pack.lesson_file).unwrap_or_else(|| {
        store
            .dir
            .join("lessons")
            .join(format!("hour-{}.md", pack.hour))
    })
}

/// Spaced retrieval block. Returns the number of cards reviewed.
pub fn review_block(store: &Store, limit: usize, time_box: Option<Duration>) -> Result<usize> {
    let mut deck = store.deck();
    let today = srs::today();
    let mut rng = srs::Rng::seeded();
    let mut queue = srs::interleave_due(&deck.cards, today, limit, &mut rng);
    if queue.is_empty() {
        return Ok(0);
    }
    println!(
        "{}",
        ui::dim(&format!(
            "{} cards. Recall the answer in your head (or type it), then grade yourself honestly.",
            queue.len()
        ))
    );
    let start = Instant::now();
    let mut retried = std::collections::HashSet::new();
    let mut reviewed = 0;
    let mut i = 0;
    while i < queue.len() {
        if let Some(tb) = time_box {
            if start.elapsed() > tb && reviewed > 0 {
                ui::info(&ui::dim("Time box reached; remaining cards stay due."));
                break;
            }
        }
        let idx = queue[i];
        let card = &deck.cards[idx];
        println!("\n{} {}", ui::cyan("◆"), card.front);
        if ui::read_line(&ui::dim("  (your answer, or Enter to reveal) › ")).is_none() {
            break;
        }
        println!("  {} {}", ui::bold("Answer:"), card.back);
        let grade = loop {
            let g = ui::ask("  [1] forgot  [2] hard  [3] good  [4] easy ›");
            match g.trim() {
                "1" => break Grade::Again,
                "2" => break Grade::Hard,
                "3" | "" => break Grade::Good,
                "4" => break Grade::Easy,
                _ => ui::warn("1-4 please"),
            }
        };
        let again = deck.cards[idx].review(grade, today);
        reviewed += 1;
        if again && retried.insert(idx) {
            queue.push(idx);
        }
        i += 1;
        store.save_deck(&deck)?;
    }
    Ok(reviewed)
}

fn print_tasks(pack: &HourPack, lab: &Path) {
    println!("\n{} {}", ui::bold("Lab:"), lab.display());
    for t in &pack.tasks {
        println!(
            "\n  {} {} {}",
            ui::cyan(&format!("[{}]", t.id)),
            ui::bold(&t.title),
            ui::dim(&format!("({}, ~{} min)", t.kind, t.minutes))
        );
        for line in t.instructions.lines() {
            println!("      {line}");
        }
        for c in &t.success_criteria {
            println!("      {} {c}", ui::dim("✓ done when:"));
        }
    }
}

/// Interactive practice loop. Returns true when the learner is done practising.
fn practice_console(
    store: &Store,
    course: &Course,
    pack: &HourPack,
    codex: &Codex,
    machine: &MachineProfile,
    progress: &mut Progress,
) -> Result<bool> {
    let lab = store.lab_dir(pack.hour);
    print_tasks(pack, &lab);
    if let Some(setup) = course_path(store, &pack.setup.script) {
        run_setup(store, &setup, pack, machine)?;
    }
    let budget = Duration::from_secs(u64::from(minutes(course, 25)) * 60);
    let start = Instant::now();
    let help = "[h] hint  [a] ask AI about my work  [c] check  [t] live tutor  [r] re-read  [l] tasks  [d] done  [q] pause";
    println!(
        "\nWork in your own editor and terminal. Commands: {}",
        ui::dim(help)
    );
    loop {
        let elapsed = start.elapsed();
        let clock = if elapsed > budget {
            ui::yellow(&format!(
                "{} (over the {} block — consider [d])",
                ui::fmt_minutes(elapsed.as_secs()),
                ui::fmt_minutes(budget.as_secs())
            ))
        } else {
            ui::dim(&format!(
                "{} / {}",
                ui::fmt_minutes(elapsed.as_secs()),
                ui::fmt_minutes(budget.as_secs())
            ))
        };
        let Some(input) = ui::read_line(&format!("\n{clock} {} ", ui::bold("practice ›"))) else {
            return Ok(false);
        };
        let mut parts = input.trim().splitn(2, ' ');
        let cmd = parts.next().unwrap_or("");
        let arg = parts.next().unwrap_or("").trim();
        match cmd {
            "h" | "hint" => local_hint(pack, progress, arg),
            "a" | "ask" => {
                let Some(task) = pick_task(pack, arg) else {
                    continue;
                };
                let q = ui::ask("What are you stuck on?");
                let level = hint_level(progress, &task.id);
                let v = codex.exec(ExecRequest {
                    label: "hint",
                    cwd: &store.dir,
                    prompt: prompts::ai_hint(
                        course,
                        task,
                        &format!("lab/hour-{}", pack.hour),
                        level as usize,
                        &q,
                    ),
                    schema: Some(schemas::hint()),
                    sandbox: Sandbox::ReadOnly,
                    web_search: false,
                    quiet: false,
                    images: vec![],
                })?;
                println!(
                    "\n{} {}",
                    ui::bold("Hint:"),
                    v["hint"].as_str().unwrap_or("")
                );
                println!(
                    "{} {}",
                    ui::bold("Think:"),
                    v["question_back"].as_str().unwrap_or("")
                );
            }
            "c" | "check" => {
                let r = check(store, course, pack, codex, false)?;
                if let Some(r) = r {
                    progress.lab_result = Some(r);
                }
            }
            "t" | "tutor" => {
                let task = pick_task(pack, arg)
                    .map(|t| format!(" I'm working on task {} ({}).", t.id, t.title))
                    .unwrap_or_default();
                codex.interactive(
                    &store.dir,
                    &format!(
                        "I'm in hour {} of my Guru course (lab/hour-{}).{task} Coach me Socratically; don't give me the solution unless I ask.",
                        pack.hour, pack.hour
                    ),
                    true,
                )?;
            }
            "r" | "read" => ui::read_file(&lesson_path(store, pack)),
            "l" | "tasks" => print_tasks(pack, &lab),
            "d" | "done" => {
                store.save_progress(progress)?;
                return Ok(true);
            }
            "q" | "quit" | "pause" => {
                store.save_progress(progress)?;
                return Ok(false);
            }
            "" => {}
            _ => println!("{}", ui::dim(help)),
        }
        store.save_progress(progress)?;
    }
}

fn pick_task<'a>(pack: &'a HourPack, arg: &str) -> Option<&'a crate::model::Task> {
    if pack.tasks.is_empty() {
        ui::warn("This hour has no tasks.");
        return None;
    }
    let wanted = if arg.is_empty() {
        let ids: Vec<&str> = pack.tasks.iter().map(|t| t.id.as_str()).collect();
        ui::ask(&format!("Which task? ({})", ids.join(", ")))
    } else {
        arg.to_string()
    };
    let wanted = wanted.trim();
    pack.tasks
        .iter()
        .find(|t| t.id == wanted || t.id.trim_start_matches('t') == wanted.trim_start_matches('t'))
        .or_else(|| {
            ui::warn("Unknown task.");
            None
        })
}

fn hint_level(progress: &Progress, task_id: &str) -> u32 {
    progress
        .hints_used
        .iter()
        .find(|(t, _)| t == task_id)
        .map(|(_, l)| *l)
        .unwrap_or(0)
}

fn local_hint(pack: &HourPack, progress: &mut Progress, arg: &str) {
    let Some(task) = pick_task(pack, arg) else {
        return;
    };
    let level = hint_level(progress, &task.id) as usize;
    match task.hints.get(level) {
        Some(h) => {
            let names = ["Nudge", "Pointer", "Near-solution"];
            println!(
                "\n{} {h}",
                ui::bold(&format!(
                    "{} ({}/{}):",
                    names.get(level).unwrap_or(&"Hint"),
                    level + 1,
                    task.hints.len()
                ))
            );
            match progress.hints_used.iter_mut().find(|(t, _)| *t == task.id) {
                Some(entry) => entry.1 += 1,
                None => progress.hints_used.push((task.id.clone(), 1)),
            }
        }
        None => ui::info(
            "No more prepared hints. Try [a] (AI looks at your actual files) or [t] (live tutor).",
        ),
    }
}

fn run_setup(
    store: &Store,
    script: &Path,
    pack: &HourPack,
    machine: &MachineProfile,
) -> Result<()> {
    let marker = script.with_extension("done");
    if marker.exists() {
        return Ok(());
    }
    let budget = machine.budget();
    println!(
        "\n{} needs setup: ~{} MB RAM, ~{} MB disk (budget {} MB / {} MB). {}",
        ui::bold("This lab"),
        pack.setup.est_ram_mb,
        pack.setup.est_disk_mb,
        budget.max_ram_mb,
        budget.max_disk_mb,
        pack.setup.notes
    );
    if u64::from(pack.setup.est_ram_mb) > budget.max_ram_mb {
        ui::warn("The estimate exceeds your RAM budget; close heavy apps first or ask the tutor for a lighter variant.");
    }
    if approved(store, script, "the lab setup script") {
        let status = script_command(script)?.status()?;
        if status.success() {
            let _ = std::fs::write(&marker, "");
            ui::ok("Lab environment ready.");
        } else {
            ui::warn(
                "Setup failed. Use [t] to work through it with the tutor (that's practice too).",
            );
        }
    }
    Ok(())
}

/// Codex writes lab scripts inside its sandbox, but they run with the learner's
/// own permissions. Show each script once and ask before running it. Approval is
/// tied to the exact contents, so any later change asks again.
fn approved(store: &Store, script: &Path, purpose: &str) -> bool {
    use std::hash::Hash;
    use std::hash::Hasher;
    let text = std::fs::read_to_string(script).unwrap_or_default();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    let marker = store
        .dir
        .join(".guru")
        .join(format!("approved-{:016x}", h.finish()));
    if marker.exists() {
        return true;
    }
    println!(
        "\nGuru wants to run {purpose} {}",
        ui::dim(&format!("({})", script.display()))
    );
    for line in text.lines().take(40) {
        println!("  {}", ui::dim(line));
    }
    if text.lines().count() > 40 {
        println!("  {}", ui::dim("… (open the file to see the rest)"));
    }
    if ui::confirm("Run it?", true) {
        let _ = std::fs::write(&marker, script.display().to_string());
        true
    } else {
        false
    }
}

fn script_command(script: &Path) -> Result<Command> {
    let dir = script.parent().context("script has no directory")?;
    let mut cmd = if script.extension().and_then(|e| e.to_str()) == Some("ps1") {
        let mut c = Command::new("powershell");
        c.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File"])
            .arg(script);
        c
    } else {
        let shell = if crate::machine::which("bash").is_some() {
            "bash"
        } else {
            "sh"
        };
        let mut c = Command::new(shell);
        c.arg(script);
        c
    };
    cmd.current_dir(dir);
    Ok(cmd)
}

/// Run a script with captured output and a timeout.
fn run_captured(script: &Path, timeout: Duration) -> Result<(bool, String)> {
    let mut child = script_command(script)?
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut out = child.stdout.take();
    let mut err = child.stderr.take();
    let t_out = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(o) = out.as_mut() {
            let _ = o.read_to_string(&mut s);
        }
        s
    });
    let t_err = std::thread::spawn(move || {
        let mut s = String::new();
        if let Some(e) = err.as_mut() {
            let _ = e.read_to_string(&mut s);
        }
        s
    });
    let start = Instant::now();
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break Some(s);
        }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(150));
    };
    let mut text = t_out.join().unwrap_or_default();
    text.push_str(&t_err.join().unwrap_or_default());
    if status.is_none() {
        text.push_str("\n[guru] check timed out\n");
    }
    // Keep only the tail: that's where failures are, and it bounds tokens.
    let chars: Vec<char> = text.chars().collect();
    let text: String = chars[chars.len().saturating_sub(6000)..].iter().collect();
    Ok((status.map(|s| s.success()).unwrap_or(false), text))
}

/// Run the automatic check (free) and optionally an AI review (one Codex turn).
/// Returns (passed, total) when known.
pub fn check(
    store: &Store,
    course: &Course,
    pack: &HourPack,
    codex: &Codex,
    force_ai: bool,
) -> Result<Option<(u32, u32)>> {
    let mut output = String::new();
    let mut script_result = None;
    if let Some(script) = course_path(store, &pack.check_script)
        .filter(|s| approved(store, s, "the lab check script"))
    {
        ui::info(&ui::dim("Running the lab checks…"));
        let (ok, text) = run_captured(&script, Duration::from_secs(180))?;
        let pass = text
            .lines()
            .filter(|l| l.trim_start().starts_with("PASS"))
            .count() as u32;
        let fail = text
            .lines()
            .filter(|l| l.trim_start().starts_with("FAIL"))
            .count() as u32;
        for l in text.lines().filter(|l| {
            let t = l.trim_start();
            t.starts_with("PASS") || t.starts_with("FAIL")
        }) {
            if l.trim_start().starts_with("PASS") {
                println!("  {}", ui::green(l.trim()));
            } else {
                println!("  {}", ui::red(l.trim()));
            }
        }
        if pass + fail == 0 {
            println!(
                "{}",
                ui::dim(
                    &text
                        .lines()
                        .rev()
                        .take(15)
                        .collect::<Vec<_>>()
                        .into_iter()
                        .rev()
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            );
        }
        if ok && fail == 0 {
            ui::ok("All automatic checks pass.");
        }
        script_result = (pass + fail > 0).then_some((pass, pass + fail));
        output = text;
    }
    let want_ai = force_ai
        || script_result.is_none()
        || ui::confirm(
            "Get detailed feedback on your work from Guru (uses one AI turn)?",
            false,
        );
    if !want_ai {
        return Ok(script_result);
    }
    ui::info(&ui::dim("Reviewing your work…"));
    let v = codex.exec(ExecRequest {
        label: "check",
        cwd: &store.dir,
        prompt: prompts::lab_check(
            course,
            pack.hour,
            &pack.tasks,
            &format!("lab/hour-{}", pack.hour),
            &output,
        ),
        schema: Some(schemas::lab_check()),
        sandbox: Sandbox::ReadOnly,
        web_search: false,
        quiet: false,
        images: vec![],
    })?;
    let mut passed = 0;
    let results = v["task_results"].as_array().cloned().unwrap_or_default();
    for r in &results {
        let ok = r["passed"].as_bool().unwrap_or(false);
        passed += u32::from(ok);
        println!(
            "\n  {} {} {}",
            if ok { ui::green("✓") } else { ui::red("✗") },
            ui::bold(r["task_id"].as_str().unwrap_or("?")),
            r["feedback"].as_str().unwrap_or("")
        );
    }
    if let Some(n) = v["next_step"].as_str() {
        println!("\n{} {n}", ui::bold("Next:"));
    }
    let mut course_m = store.course()?;
    for m in v["misconceptions"].as_array().into_iter().flatten() {
        if let Some(m) = m.as_str() {
            course_m.add_misconception(m);
        }
    }
    store.save_course(&course_m)?;
    Ok(if results.is_empty() {
        script_result
    } else {
        Some((passed, results.len() as u32))
    })
}

fn confidence_to_p(c: u32) -> f32 {
    [0.1, 0.3, 0.5, 0.7, 0.9][(c.clamp(1, 5) - 1) as usize]
}

/// (brier score, overconfidence) for (confidence 1-5, achieved score 0-1) pairs.
pub fn calibration(pairs: &[(u32, f32)]) -> (f32, f32) {
    if pairs.is_empty() {
        return (0.0, 0.0);
    }
    let n = pairs.len() as f32;
    let brier = pairs
        .iter()
        .map(|(c, s)| (confidence_to_p(*c) - s).powi(2))
        .sum::<f32>()
        / n;
    let over = pairs
        .iter()
        .map(|(c, s)| confidence_to_p(*c) - s)
        .sum::<f32>()
        / n;
    (brier, over)
}

pub fn hour_score(quiz: f32, feynman: f32, lab: Option<f32>) -> f32 {
    match lab {
        Some(l) => 0.5 * quiz + 0.15 * feynman + 0.35 * l,
        None => 0.75 * quiz + 0.25 * feynman,
    }
}

fn prove(
    store: &Store,
    course: &mut Course,
    pack: &HourPack,
    codex: &Codex,
    progress: &mut Progress,
) -> Result<()> {
    if progress.lab_result.is_none() && !pack.tasks.is_empty() {
        if let Some(r) = check(store, course, pack, codex, true)? {
            progress.lab_result = Some(r);
            store.save_progress(progress)?;
        }
    }
    let lab_score = progress
        .lab_result
        .filter(|(_, t)| *t > 0)
        .map(|(p, t)| p as f32 / t as f32);

    println!("\n{}", ui::bold("Exit quiz — from memory, no peeking."));
    let mut quiz = Vec::new();
    for (i, q) in pack.exit_quiz.iter().enumerate() {
        println!("\n{} {}", ui::bold(&format!("Q{}.", i + 1)), q.question);
        let a = ui::ask("›");
        let c = ui::ask_number("  Confidence you're right (1 guess … 5 certain)", 1, 5, 3);
        quiz.push((q.clone(), a, c));
    }
    println!();
    let feynman = ui::ask_multiline(&format!("Explain it simply: {}", pack.feynman_prompt));

    let lab_summary = match progress.lab_result {
        Some((p, t)) => {
            let hints: u32 = progress.hints_used.iter().map(|(_, l)| l).sum();
            format!("{p}/{t} tasks passed; {hints} prepared hints used")
        }
        None => "no lab result".into(),
    };
    ui::info(&ui::dim("\nGrading…"));
    let v = codex.exec(ExecRequest {
        label: "assess",
        cwd: &store.dir,
        prompt: prompts::assess(
            course,
            pack.hour,
            &quiz,
            &pack.feynman_prompt,
            &feynman,
            &lab_summary,
        ),
        schema: Some(schemas::assess()),
        sandbox: Sandbox::ReadOnly,
        web_search: false,
        quiet: true,
        images: vec![],
    })?;

    // Re-load: lab checks may have added misconceptions.
    *course = store.course()?;
    let mut pairs = Vec::new();
    let mut total = 0.0;
    let results = v["results"].as_array().cloned().unwrap_or_default();
    for (i, (q, _, c)) in quiz.iter().enumerate() {
        let r = results.iter().find(|r| r["id"].as_str() == Some(&q.id));
        let score = r
            .and_then(|r| r["score"].as_f64())
            .unwrap_or(0.0)
            .clamp(0.0, 1.0) as f32;
        let fb = r.and_then(|r| r["feedback"].as_str()).unwrap_or("");
        total += score;
        pairs.push((*c, score));
        let mark = if score >= 0.8 {
            ui::green("✓")
        } else if score >= 0.4 {
            ui::yellow("~")
        } else {
            ui::red("✗")
        };
        println!("{mark} Q{}: {fb}", i + 1);
    }
    let quiz_score = if quiz.is_empty() {
        0.0
    } else {
        total / quiz.len() as f32
    };
    let f_score = v["feynman"]["score"]
        .as_f64()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0) as f32;
    println!(
        "\n{} {}",
        ui::bold("Your explanation:"),
        v["feynman"]["feedback"].as_str().unwrap_or("")
    );
    for m in v["feynman"]["misconceptions"]
        .as_array()
        .into_iter()
        .flatten()
    {
        if let Some(m) = m.as_str() {
            course.add_misconception(m);
        }
    }
    for m in learn::parse_mastery(&v["skill_mastery"]) {
        course.update_mastery(&m.skill_id, m.mastery);
    }
    let new_cards: Vec<NewCard> = v["new_cards"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| serde_json::from_value(c.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    let added = add_cards(store, &new_cards, &format!("h{}fix", pack.hour))?;
    let coach_note = v["coach_note"].as_str().unwrap_or("").to_string();
    course.add_coach_note(&coach_note);

    let (brier, over) = calibration(&pairs);
    let score = hour_score(quiz_score, f_score, lab_score);

    // Mastery gate: anything taught so far that is still weak gets remediated next hour.
    let hp = course.hours.iter().find(|h| h.hour == pack.hour).cloned();
    let mut weak: Vec<String> = hp
        .map(|h| h.skill_ids)
        .unwrap_or_default()
        .into_iter()
        .filter(|id| course.skill(id).map(|s| s.mastery < 0.7).unwrap_or(false))
        .collect();
    weak.dedup();
    course.remediation = weak.clone();
    if let Some(h) = course.hour_mut(pack.hour) {
        h.status = HourStatus::Done;
        h.score = Some(score);
    }
    store.save_course(course)?;

    ui::header(&format!("Hour {} result", pack.hour));
    println!(
        "  Quiz      {} {:.0}%",
        ui::bar(quiz_score, 20),
        quiz_score * 100.0
    );
    if let Some(l) = lab_score {
        println!("  Lab       {} {:.0}%", ui::bar(l, 20), l * 100.0);
    }
    println!(
        "  Explain   {} {:.0}%",
        ui::bar(f_score, 20),
        f_score * 100.0
    );
    println!("  Hour      {} {:.0}%", ui::bar(score, 20), score * 100.0);
    if over > 0.15 {
        ui::warn(&format!(
            "Overconfident by {:.0} points on average: the feeling of knowing came before the knowing. Expect more retrieval on these.",
            over * 100.0
        ));
    } else if over < -0.15 {
        ui::info(&format!("You know more than you think (underconfident by {:.0} points). Trust your recall a bit more.", -over * 100.0));
    } else if !pairs.is_empty() {
        ui::ok("Well calibrated: your confidence matches your performance.");
    }
    if score >= 0.8 {
        ui::ok("Mastery gate passed.");
    } else {
        ui::warn(&format!(
            "Below the 80% mastery gate. Next hour opens by repairing: {}.",
            if weak.is_empty() {
                "this hour's weak points".into()
            } else {
                weak.join(", ")
            }
        ));
    }
    if added > 0 {
        ui::info(&format!(
            "{added} new {} target exactly what you missed.",
            if added == 1 { "card" } else { "cards" }
        ));
    }
    if !coach_note.is_empty() {
        println!("{} {coach_note}", ui::bold("Coach:"));
    }

    let reflection = ui::ask(
        "One line: what's the most useful thing you can do now that you couldn't an hour ago?",
    );
    store.append_journal(&JournalEntry {
        hour: pack.hour,
        day: srs::today(),
        minutes_spent: ((srs::now_unix().saturating_sub(progress.started_unix)) / 60) as u32,
        quiz_score,
        lab_score,
        hour_score: score,
        overconfidence: over,
        brier,
        reflection,
        coach_note,
    })?;
    Ok(())
}

/// Stop everything the lab started. Guru leaves no background load behind.
pub fn teardown(store: &Store, pack: &HourPack, ask: bool) -> Result<()> {
    let Some(script) = course_path(store, &pack.setup.teardown_script) else {
        return Ok(());
    };
    if ask && !ui::confirm("Stop the lab's services and free RAM/CPU now?", true) {
        ui::info("Left running. `guru clean` stops it later.");
        return Ok(());
    }
    if !approved(store, &script, "the lab teardown script") {
        return Ok(());
    }
    let (ok, text) = run_captured(&script, Duration::from_secs(120))?;
    if ok {
        ui::ok("Lab stopped; resources released.");
        // Next time the lab is used, setup must run again.
        if let Some(setup) = course_path(store, &pack.setup.script) {
            let _ = std::fs::remove_file(setup.with_extension("done"));
        }
    } else {
        ui::warn(&format!(
            "Teardown reported a problem:\n{}",
            text.lines().rev().take(5).collect::<Vec<_>>().join("\n")
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_math() {
        let (brier, over) = calibration(&[(5, 0.0), (5, 0.0)]);
        assert!((brier - 0.81).abs() < 1e-5);
        assert!((over - 0.9).abs() < 1e-5);
        let (_, over) = calibration(&[(1, 1.0)]);
        assert!(over < -0.8);
        assert_eq!(calibration(&[]), (0.0, 0.0));
    }

    #[test]
    fn hour_score_weights() {
        assert!((hour_score(1.0, 1.0, Some(1.0)) - 1.0).abs() < 1e-6);
        assert!((hour_score(1.0, 0.0, None) - 0.75).abs() < 1e-6);
        assert!((hour_score(0.0, 0.0, Some(1.0)) - 0.35).abs() < 1e-6);
    }
}
