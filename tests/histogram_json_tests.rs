use std::fs;

use autograder_rust::config::TestCfg;
use autograder_rust::testcases::{TestRunner, RepoResult, TcResult};

#[test]
fn histogram_and_write_json() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path();
    let project = "projx".to_string();

    // Prepare runner with any tests_path (not used for rubric in this test)
    let cfg = TestCfg { tests_path: base.to_string_lossy().to_string(), digital_path: String::from("~/Digital/Digital.jar") };
    let runner = TestRunner::new(&cfg, false, false, false, project.clone());

    // Two fake results with scores 3 and 7
    let rr1 = RepoResult { student: Some("alice".into()), score: 3, results: vec![TcResult{rubric:3, score:3, test:"01".into(), test_err: None}], comment: String::new(), build_err: None };
    let rr2 = RepoResult { student: Some("bob".into()), score: 7, results: vec![TcResult{rubric:7, score:7, test:"01".into(), test_err: None}], comment: String::new(), build_err: None };
    let class_results = vec![rr1, rr2];

    // Print histogram (smoke test: just ensure it doesn't panic)
    runner.print_histogram(&class_results);

    // Write JSON and verify contents
    let fname = format!("{}.json", project);
    runner.write_class_json(&class_results, None).unwrap();
    let data = fs::read_to_string(base.join(&fname)).unwrap_or_else(|_| fs::read_to_string(fname).unwrap());
    assert!(data.contains("alice"));
    assert!(data.contains("bob"));
}
