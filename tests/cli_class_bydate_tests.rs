use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

use assert_cmd::prelude::*;
use predicates::str::contains as p_contains;

fn write_student_repo(base: &std::path::Path, label: &str, project: &str) {
    let repo = base.join(label);
    fs::create_dir_all(&repo).unwrap();
    let prog = repo.join(project);
    fs::write(&prog, "#!/bin/sh\necho ok\n").unwrap();
    let mut perm = fs::metadata(&prog).unwrap().permissions();
    perm.set_mode(0o755); fs::set_permissions(&prog, perm).unwrap();
    fs::write(repo.join("Makefile"), "all:\n\t@echo built\n").unwrap();
}

fn write_tests_repo_with_case(base: &std::path::Path, project: &str) -> std::path::PathBuf {
    let tests = base.join("tests_repo").join(project);
    fs::create_dir_all(&tests).unwrap();
    fs::write(tests.join(format!("{}.toml", project)), r#"
[project]
build = 'make'

[[tests]]
name = "01"
input = ["./$project"]
expected = "OK"
rubric = 10
case_sensitive = true
"#).unwrap();
    // dates.toml for selection
    fs::write(base.join("tests_repo").join("dates.toml"), r#"
[projx]

[[projx.dates]]
suffix = "due"
date = "2025-03-11"
percentage = 1.0
"#).unwrap();
    tests.parent().unwrap().to_path_buf()
}

#[test]
fn cli_class_by_date_unified_diff_and_json() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path();
    let project = "projx";
    let suffix = "due";

    // Prepare tests repo with a case-sensitive mismatch
    let tests_repo = write_tests_repo_with_case(base, project);

    // Prepare config dir with one student 'alice'
    let cfgdir = base.join("cfg");
    fs::create_dir_all(&cfgdir).unwrap();
    fs::write(cfgdir.join("config.toml"), format!("[Test]\ntests_path = \"{}\"\n[Config]\nstudents = [\"alice\"]\n", tests_repo.to_string_lossy())).unwrap();

    // Create student repo with date suffix: ./projx-alice-due
    write_student_repo(base, &format!("{}-{}-{}", project, "alice", suffix), project);

    let mut cmd = Command::cargo_bin("grade-rs").unwrap();
    cmd.arg("class").args(["-p", project, "-d", "-v", "--unified-diff"]) // single date auto-selects
        .env("HOME", base)
        .env("GRADE_CONFIG_DIR", &cfgdir)
        .current_dir(&base);

    // Score will be 0/10 due to mismatch; JSON file should have -due suffix
    cmd.assert().success().stdout(p_contains("Score frequency")).stdout(p_contains("0/10"));
    let json_path = base.join(format!("{}-{}.json", project, suffix));
    assert!(json_path.exists());
    let data = fs::read_to_string(&json_path).unwrap();
    assert!(data.contains("\"student\": \"alice\""));
}
