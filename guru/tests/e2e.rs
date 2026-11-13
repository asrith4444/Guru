//! End-to-end: drive the real `guru` binary through course creation, a full
//! hour and a spaced review, with a fake Codex that returns canned answers.

#![cfg(unix)]

use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

fn temp_home(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("guru-e2e-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn fake_codex() -> PathBuf {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fake_codex.sh");
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(&p).unwrap().permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&p, perm).unwrap();
    p
}

fn guru(home: &Path, today: i64, args: &[&str], stdin: &str) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_guru"))
        .args(args)
        .env("GURU_HOME", home)
        .env("GURU_CODEX_BIN", fake_codex())
        .env("GURU_TODAY", today.to_string())
        .env("NO_COLOR", "1")
        .env_remove("PAGER")
        .env_remove("GURU_READER")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), text)
}

fn json(path: &Path) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn learn_session_review_roundtrip() {
    let home = temp_home("roundtrip");
    let day = 20_000;

    // Intake: goal, prior, 2 diagnostic answers, then decline starting now.
    let (ok, out) = guru(
        &home,
        day,
        &["learn", "Test", "Topic"],
        "build widgets\nsome python\na thing\nno idea\nn\n",
    );
    assert!(ok, "{out}");
    assert!(out.contains("Test Topic in 7 Hours"), "{out}");
    assert!(out.contains("Problem bank: 1 real-world issue."), "{out}");
    let course_dir = home.join("courses/test-topic");
    let course = json(&course_dir.join("course.json"));
    assert_eq!(course["arena"], "code_lab");
    assert_eq!(
        course["hours"].as_array().unwrap().len(),
        7,
        "padded to 7 hours"
    );
    assert_eq!(course["hours"][0]["title"], "First widget (compressed)");
    assert_eq!(course["skills"][0]["mastery"], 0.5);
    assert_eq!(course["misconceptions"][0], "widgets never fail");
    assert!(std::fs::read_to_string(course_dir.join("AGENTS.md"))
        .unwrap()
        .contains("HARD LAB BUDGET"));

    // Hour 1: predict, practice (hint + check, decline AI feedback, done),
    // quiz (2 answers with confidence), explain, reflect.
    let input = [
        "a widget is a thing", // predict
        "h t1",                // hint level 1
        "c",                   // run checks
        "y",                   // approve the check script (asked once per script version)
        "n",                   // no AI feedback
        "d",                   // done practising
        "a thing",
        "5", // q1 + confidence
        "make it explode",
        "5", // q2 + confidence (overconfident)
        "Widgets are things that do stuff.",
        "",                     // explanation
        "I can build a widget", // reflection
    ]
    .join("\n")
        + "\n";
    let (ok, out) = guru(&home, day, &["next"], &input);
    assert!(ok, "{out}");
    assert!(out.contains("Widgets are things."), "lesson shown: {out}");
    assert!(out.contains("Nudge (1/3): look at TODO"), "{out}");
    assert!(
        out.contains("Guru wants to run the lab check script"),
        "{out}"
    );
    assert!(out.contains("PASS t1"), "{out}");
    assert!(out.contains("FAIL t2"), "{out}");
    assert!(out.contains("Overconfident"), "{out}");
    assert!(out.contains("Below the 80% mastery gate"), "{out}");

    let course = json(&course_dir.join("course.json"));
    assert_eq!(course["hours"][0]["status"], "done");
    let score = course["hours"][0]["score"].as_f64().unwrap();
    // quiz 0.625*0.5 + feynman 0.8*0.15 + lab 0.5*0.35
    assert!((score - 0.6075).abs() < 1e-3, "score {score}");
    // basics: 0.4 * 0.5 (pretest) + 0.6 * 0.9 (this hour) = 0.74 >= 0.7, so nothing to repair.
    assert_eq!(course["remediation"], serde_json::json!([]));
    // The AI lab review was declined, so only the pretest misconception is known.
    assert_eq!(
        course["misconceptions"],
        serde_json::json!(["widgets never fail"])
    );
    let issues = json(&course_dir.join("issues.json"));
    assert_eq!(issues[0]["used"], true);
    assert!(!course_dir.join("progress.json").exists());
    let journal = std::fs::read_to_string(course_dir.join("journal.jsonl")).unwrap();
    assert!(journal.contains("I can build a widget"));

    // Cards: 2 from the lesson + 1 targeting the mistake, first due tomorrow.
    let deck = json(&course_dir.join("cards.json"));
    assert_eq!(deck["cards"].as_array().unwrap().len(), 3);
    let (ok, out) = guru(&home, day, &["review"], "");
    assert!(ok && out.contains("Nothing due"), "{out}");
    let (ok, out) = guru(&home, day + 1, &["review"], "\n3\n\n1\n\n3\n\n3\n");
    assert!(ok, "{out}");
    assert!(
        out.contains("Reviewed 4 cards"),
        "forgotten card retried once: {out}"
    );

    let (ok, out) = guru(&home, day + 1, &["status"], "");
    assert!(ok, "{out}");
    assert!(out.contains("1/7 hours"), "{out}");

    let (ok, out) = guru(&home, day + 1, &["ask", "what", "is", "a", "widget"], "");
    assert!(ok && out.contains("A widget is a thing."), "{out}");

    // Every prompt carried the method and the learner model.
    let log = std::fs::read_to_string(course_dir.join(".guru/fake-prompts.log")).unwrap();
    assert!(log.contains("Retrieval over re-reading"));
    assert!(log.contains("HARD LAB BUDGET"));
    assert!(log.contains("Known misconceptions to repair"));

    let _ = std::fs::remove_dir_all(&home);
}
