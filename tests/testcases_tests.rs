use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use autograder_rust::config::TestCfg;
use autograder_rust::testcases::{TestRunner, Repo};

fn write_mini_repo(base: &PathBuf, program_name: &str) -> PathBuf {
    let repo = base.join("repo");
    fs::create_dir_all(&repo).unwrap();
    let prog = repo.join(program_name);
    let script = r#"#!/bin/sh
set -eu
if [ "$#" -eq 0 ]; then
  echo ok
  exit 0
fi
if [ "$#" -eq 1 ]; then
  echo "$1"
  exit 0
fi
if [ "$#" -eq 2 ] && [ "$1" = "-o" ]; then
  printf "%s" "04out" > "$2"
  exit 0
fi
exit 0
"#;
    fs::write(&prog, script).unwrap();
    let mut perm = fs::metadata(&prog).unwrap().permissions();
    perm.set_mode(0o755);
    fs::set_permissions(&prog, perm).unwrap();
    fs::write(repo.join("Makefile"), "all:\n\t@echo built\n").unwrap();
    repo
}

fn write_tests_repo(base: &PathBuf, project: &str) -> PathBuf {
    let tests = base.join("tests_repo").join(project);
    fs::create_dir_all(&tests).unwrap();
    let toml = format!(r#"
[project]
build = 'make'
strip_output = ''

[[tests]]
name = "01"
input = ["./$project"]
expected = "ok"
rubric = 2

[[tests]]
name = "02"
input = ["./$project", "hello"]
expected = "hello"
rubric = 3

[[tests]]
name = "04"
output = "04.txt"
input = ["./$project", "-o", "04.txt"]
expected = "04out"
rubric = 5
"#);
    fs::write(tests.join(format!("{}.toml", project)), toml).unwrap();
    tests.parent().unwrap().to_path_buf()
}

#[test]
fn test_runner_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path().to_path_buf();
    let project = "projx";
    let repo = write_mini_repo(&base, project);
    let tests_repo = write_tests_repo(&base, project);

    let cfg = TestCfg { tests_path: tests_repo.to_string_lossy().to_string(), digital_path: String::from("~/Digital/Digital.jar") };
    let mut runner = TestRunner::new(&cfg, false, false, false, project.to_string());
    // Create local repo object
    let repo_obj = Repo::local(repo.to_string_lossy().to_string(), runner.project_subdir());
    let res = runner.test_repo(&repo_obj, None).unwrap();
    assert_eq!(res.score, 10);
    assert_eq!(res.results.len(), 3);
}
