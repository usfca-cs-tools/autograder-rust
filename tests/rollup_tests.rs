use std::fs;
use std::sync::Mutex;

use autograder_rust::dates::DateItem;
use autograder_rust::rollup::rollup;

// pushd::Pushd mutates process-global cwd, so tests in this binary must
// serialize on this lock to avoid stomping on each other under cargo's
// default parallel test runner.
static CWD_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn rollup_math_applies_improvements_only() {
    let _lock = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path().to_path_buf();
    let _g = pushd::Pushd::new(&cwd);
    let project = "projx";

    // due: alice 5, bob 7
    fs::write(
        cwd.join(format!("{}-due.json", project)),
        r#"[
  {"student":"alice","score":5,"comment":""},
  {"student":"bob","score":7,"comment":""}
]"#,
    )
    .unwrap();
    // late: alice 8 (improvement 3 @ 50% => +1.5), bob 7 (no change)
    fs::write(
        cwd.join(format!("{}-late.json", project)),
        r#"[
  {"student":"alice","score":8,"comment":""},
  {"student":"bob","score":7,"comment":""}
]"#,
    )
    .unwrap();

    let dates = vec![
        DateItem {
            suffix: "due".into(),
            date: "2025-01-01".into(),
            percentage: 1.0,
        },
        DateItem {
            suffix: "late".into(),
            date: "2025-01-08".into(),
            percentage: 0.5,
        },
    ];

    rollup(project, &dates).unwrap();
    let out = fs::read_to_string(cwd.join(format!("{}-rollup.json", project))).unwrap();
    assert!(out.contains("\"student\": \"alice\""));
    assert!(out.contains("\"student\": \"bob\""));
    // alice: 5 then + (8-5)*0.5 = 6.5; bob: 7 stays 7
    assert!(out.contains("6.5"));
    assert!(out.contains("7.0"));
}

#[test]
fn rollup_ignores_regressions() {
    // A flaky middle run (or a worse late commit) must not drag the rolled
    // score below the best previous rolled total. Models the real-world case
    // where the same commit was graded three times and the middle run
    // produced a lower score than the other two.
    let _lock = CWD_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let tmp = tempfile::tempdir().unwrap();
    let cwd = tmp.path().to_path_buf();
    let _g = pushd::Pushd::new(&cwd);
    let project = "projx";

    fs::write(
        cwd.join(format!("{}-due.json", project)),
        r#"[{"student":"alice","score":100,"comment":""}]"#,
    )
    .unwrap();
    fs::write(
        cwd.join(format!("{}-1week-late.json", project)),
        r#"[{"student":"alice","score":74,"comment":""}]"#,
    )
    .unwrap();
    fs::write(
        cwd.join(format!("{}-super-late.json", project)),
        r#"[{"student":"alice","score":100,"comment":""}]"#,
    )
    .unwrap();

    let dates = vec![
        DateItem {
            suffix: "due".into(),
            date: "2025-01-01".into(),
            percentage: 1.0,
        },
        DateItem {
            suffix: "1week-late".into(),
            date: "2025-01-08".into(),
            percentage: 0.75,
        },
        DateItem {
            suffix: "super-late".into(),
            date: "2025-01-15".into(),
            percentage: 0.5,
        },
    ];

    rollup(project, &dates).unwrap();
    let out = fs::read_to_string(cwd.join(format!("{}-rollup.json", project))).unwrap();
    // Should hold at 100 — neither the flaky 74 nor the matching 100 lowers it.
    assert!(
        out.contains("\"score\": 100.0") || out.contains("\"score\": 100"),
        "expected rolled score to stay at 100, got: {}",
        out
    );
}
