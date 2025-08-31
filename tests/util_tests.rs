use std::fs;

use autograder_rust::util;

#[test]
fn project_from_cwd_parses_with_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path().join("project1-someuser");
    fs::create_dir(&d).unwrap();
    let _g = pushd::Pushd::new(&d);
    assert_eq!(util::project_from_cwd(), "project1");
}

#[test]
fn project_from_cwd_parses_without_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path().join("project1");
    fs::create_dir(&d).unwrap();
    let _g = pushd::Pushd::new(&d);
    assert_eq!(util::project_from_cwd(), "project1");
}

#[test]
fn format_and_failed() {
    let s = util::format_pass_fail("01", 5, 5);
    assert!(s.starts_with("01(5/5)"));
}
