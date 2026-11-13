//! End-to-end: size a topic, create a course, run the full download pipeline
//! against a fake Codex, then check the offline package is complete and valid.

#![cfg(unix)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use guru_core::codex::Codex;
use guru_core::config::Config;
use guru_core::course::CourseStatus;
use guru_core::course::Package;
use guru_core::generator;
use guru_core::sim;

fn setup() -> PathBuf {
    let home = std::env::temp_dir().join(format!("guru-gen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).unwrap();
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fake = manifest.join("tests/fake_codex.sh");
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(&fake).unwrap().permissions();
    perm.set_mode(0o755);
    std::fs::set_permissions(&fake, perm).unwrap();
    // Single test per binary, so process-wide env is safe here.
    std::env::set_var("GURU_HOME", &home);
    std::env::set_var("GURU_CODEX_BIN", &fake);
    std::env::set_var("FAKE_DEMO_DIR", manifest.join("../ui/demo"));
    home
}

#[test]
fn scope_create_download_resume() {
    let home = setup();
    let cfg = Config {
        parallel_jobs: Some(2),
        stackexchange: false,
        ..Config::default()
    };
    let codex = Codex::from_config(&cfg);

    let scope = generator::scope(&codex, "Widgets", "ship widgets").unwrap();
    assert_eq!(scope.intermediate.hours, 2.5);
    let hours = scope.hours_for(66);

    let pkg = generator::create(&generator::Request {
        topic: "Widgets".into(),
        goal: "ship widgets".into(),
        level: 66,
        hours,
        scope: Some(scope),
    })
    .unwrap();
    assert_eq!(pkg.course().unwrap().status, CourseStatus::Planning);

    let events = Mutex::new(Vec::new());
    let emit = |e: generator::Event| events.lock().unwrap().push(e);
    generator::run(&pkg, &cfg, &AtomicBool::new(false), &emit).unwrap();

    let course = pkg.course().unwrap();
    assert_eq!(
        course.status,
        CourseStatus::Ready,
        "{}",
        course.status_detail
    );
    assert_eq!(course.title, "Widgets Fast");
    assert_eq!(course.level_label, "Intermediate");
    assert_eq!(course.capstone.id, "capstone", "capstone id is normalised");
    assert_eq!(course.modules.len(), 2);
    for m in &course.modules {
        let module = pkg.module(&m.id).unwrap();
        assert!(!module.lesson.is_empty());
        assert_eq!(module.id, m.id);
        assert!(module.quiz.iter().all(|q| q.id.starts_with(&m.id)));
    }
    for s in course.all_sims() {
        let sim = pkg.sim(&s.id).unwrap();
        assert_eq!(sim.id, s.id);
        assert!(sim::validate(&sim).is_empty(), "{} invalid", s.id);
    }
    // The flawed first attempt was repaired with the validator's feedback.
    let log = std::fs::read_to_string(pkg.dir.join(".guru/fake-prompts.log")).unwrap();
    assert!(log.contains("FAILED VALIDATION"));
    assert!(
        log.contains("unknown node 'ghost'"),
        "validator errors are fed back"
    );
    assert!(
        log.contains("Pace aggressively"),
        "method is in every prompt"
    );
    // Flashcards were collected once (both modules return the same cards).
    assert_eq!(pkg.deck().cards.len(), 6);

    let events = events.into_inner().unwrap();
    assert!(events.iter().any(|e| e.phase == "planning"));
    assert!(events.iter().any(|e| e.activity.starts_with("Researching")));
    let last = events.last().unwrap();
    assert_eq!(
        (last.phase.as_str(), last.done, last.total),
        ("ready", 5, 5)
    );

    // Resume after losing a file rebuilds only that file.
    std::fs::remove_file(pkg.sim_path("m1s1").unwrap()).unwrap();
    let before = log.matches("----").count();
    let events2 = Mutex::new(Vec::new());
    let emit2 = |e: generator::Event| events2.lock().unwrap().push(e);
    generator::run(&pkg, &cfg, &AtomicBool::new(false), &emit2).unwrap();
    let after = std::fs::read_to_string(pkg.dir.join(".guru/fake-prompts.log"))
        .unwrap()
        .matches("----")
        .count();
    assert_eq!(
        after - before,
        1,
        "only the missing simulation was regenerated"
    );
    assert!(pkg.sim_path("m1s1").unwrap().exists());

    // Library listing and deletion.
    assert_eq!(Package::list().len(), 1);
    Package::open(&course.slug).unwrap().delete().unwrap();
    assert!(Package::list().is_empty());
    let _ = std::fs::remove_dir_all(home);
}
