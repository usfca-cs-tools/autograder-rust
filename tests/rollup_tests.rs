use std::fs;
use std::path::PathBuf;

use autograder_rust::rollup::rollup;
use autograder_rust::dates::DateItem;

#[test]
fn rollup_math_applies_improvements_only() {
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
    ).unwrap();
    // late: alice 8 (improvement 3 @ 50% => +1.5), bob 7 (no change)
    fs::write(
        cwd.join(format!("{}-late.json", project)),
        r#"[
  {"student":"alice","score":8,"comment":""},
  {"student":"bob","score":7,"comment":""}
]"#,
    ).unwrap();

    let dates = vec![
        DateItem { suffix: "due".into(), date: "2025-01-01".into(), percentage: 1.0 },
        DateItem { suffix: "late".into(), date: "2025-01-08".into(), percentage: 0.5 },
    ];

    rollup(project, &dates).unwrap();
    let out = fs::read_to_string(cwd.join(format!("{}-rollup.json", project))).unwrap();
    assert!(out.contains("\"student\": \"alice\""));
    assert!(out.contains("\"student\": \"bob\""));
    // alice: 5 then + (8-5)*0.5 = 6.5; bob: 7 stays 7
    assert!(out.contains("6.5"));
    assert!(out.contains("7.0"));
}

