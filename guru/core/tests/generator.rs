//! End-to-end against a fake Codex (tests/fake_codex.sh):
//! goal → clarifying questions → roadmap → download a stage, including a
//! dropped connection, a hung request, a thin lesson and an unplayable
//! simulation, all of which must be recovered without failing the download.

#![cfg(unix)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use guru_core::codex::Codex;
use guru_core::config::Config;
use guru_core::course::CourseStatus;
use guru_core::course::Package;
use guru_core::generator;
use guru_core::goal;
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
    std::env::set_var(
        "FAKE_DEMO_DIR",
        manifest.join("../ui/demo/incident-response"),
    );
    std::env::set_var("GURU_RETRY_DELAY_MS", "0");
    std::env::set_var("GURU_CODEX_TIMEOUT_SECS", "3");
    home
}

#[test]
fn goal_to_stage_download_survives_failures() {
    let home = setup();
    let cfg = Config {
        parallel_jobs: Some(2),
        stackexchange: false,
        ..Config::default()
    };
    let codex = Codex::from_config(&cfg);
    generator::preflight(&codex).expect("fake codex is installed and signed in");

    // Goal → questions → roadmap.
    let c = goal::clarify(&codex, "I want to become an electrician").unwrap();
    assert_eq!(c.questions.len(), 2);
    assert_eq!(c.questions[0].id, "q1", "ids are normalised");
    let answers = vec![goal::Answer {
        question: c.questions[1].question.clone(),
        answer: "UK (BS 7671)".into(),
    }];
    let plan = goal::plan(&codex, "I want to become an electrician", answers).unwrap();
    assert_eq!(plan.stages.len(), 3);
    assert_eq!(plan.stages[2].id, "s3");
    assert_eq!(plan.total_hours(), 9.0);
    assert_eq!(goal::list().len(), 1);

    // Download stage 1 as a course.
    let req = plan.request_for("s1").unwrap();
    let pkg = generator::create(&req).unwrap();
    let slug = pkg.course().unwrap().slug;
    goal::link_course(&plan.slug, "s1", &slug).unwrap();
    assert_eq!(goal::load(&plan.slug).unwrap().stages[0].course_slug, slug);

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
    assert!(
        course.goal_context.contains("BS 7671"),
        "roadmap context is stored"
    );
    assert_eq!(course.capstone.id, "capstone");
    for m in &course.modules {
        let module = pkg.module(&m.id).unwrap();
        assert!(
            generator::module_issues(&module, m.minutes).is_empty(),
            "module {} is worth the time",
            m.id
        );
    }
    for s in course.all_sims() {
        let sim = pkg.sim(&s.id).unwrap();
        assert!(sim::validate(&sim).is_empty(), "{} invalid", s.id);
    }

    let log = std::fs::read_to_string(pkg.dir.join(".guru/fake-prompts.log")).unwrap();
    assert!(
        log.contains("one stage of the learner's roadmap"),
        "the plan knows its stage"
    );
    assert!(
        log.contains("Professional standard, any field"),
        "professional rules are in every prompt"
    );
    assert!(
        log.contains("NOT GOOD ENOUGH"),
        "the thin lesson was sent back"
    );
    assert!(
        log.contains("FAILED VALIDATION") && log.contains("unknown node 'ghost'"),
        "validator errors are fed back"
    );

    let events = events.into_inner().unwrap();
    let acts: Vec<&str> = events.iter().map(|e| e.activity.as_str()).collect();
    assert!(
        acts.iter().any(|a| a.contains("connection problem")),
        "dropped stream was retried: {acts:?}"
    );
    assert!(
        acts.iter()
            .any(|a| a.starts_with("Retrying Simulation: First incident")),
        "hung request was retried: {acts:?}"
    );
    assert!(acts.iter().any(|a| a.starts_with("Improving the lesson")));
    let last = events.last().unwrap();
    assert_eq!(
        (last.phase.as_str(), last.done, last.total),
        ("ready", 5, 5)
    );
    let dl = std::fs::read_to_string(pkg.dir.join(".guru/download.log")).unwrap();
    assert!(dl.contains("[ready]"), "download log is written");

    // Resume after losing a file rebuilds only that file.
    std::fs::remove_file(pkg.sim_path("m2s1").unwrap()).unwrap();
    let before = log.matches("----").count();
    generator::run(&pkg, &cfg, &AtomicBool::new(false), &|_| {}).unwrap();
    let after = std::fs::read_to_string(pkg.dir.join(".guru/fake-prompts.log"))
        .unwrap()
        .matches("----")
        .count();
    assert_eq!(
        after - before,
        2,
        "only the missing simulation was rebuilt (one invalid attempt + one fix)"
    );

    // A crash mid-download is recovered as resumable.
    let mut c = pkg.course().unwrap();
    c.status = CourseStatus::Downloading;
    pkg.save_course(&c).unwrap();
    assert_eq!(generator::recover_interrupted(), 1);
    let c = pkg.course().unwrap();
    assert_eq!(c.status, CourseStatus::Failed);
    assert!(c.status_detail.contains("Resume"));

    // A quick course without a roadmap still works (level-bar path).
    let scope = generator::scope(&codex, "Widgets", "ship widgets").unwrap();
    assert_eq!(scope.intermediate.hours, 2.5);

    Package::open(&slug).unwrap().delete().unwrap();
    goal::delete(&plan.slug).unwrap();
    assert!(Package::list().is_empty() && goal::list().is_empty());
    let _ = std::fs::remove_dir_all(home);
}
