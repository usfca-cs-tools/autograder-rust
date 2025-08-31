use std::fs;
use std::os::unix::fs::PermissionsExt;

use autograder_rust::config::TestCfg;
use autograder_rust::testcases::{TestRunner, Repo};

#[test]
fn digital_substitution_and_java_invocation() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path();
    let project = "projx";

    // Create tests repo with Digital invocation
    let tests = base.join("tests_repo").join(project);
    fs::create_dir_all(&tests).unwrap();

    // Fake java in PATH that prints a fixed expected line
    let bindir = base.join("bin");
    fs::create_dir_all(&bindir).unwrap();
    let fake_java = bindir.join("java");
    // We'll have this script print exactly the expected line we set below
    let digital_path = base.join("Digital").join("Digital.jar");
    fs::create_dir_all(digital_path.parent().unwrap()).unwrap();
    let expected_line = format!("-cp {} CLI test {}/1bfa_test.dig", digital_path.to_string_lossy(), tests.to_string_lossy());
    fs::write(&fake_java, format!("#!/bin/sh\nprintf \"%s\\n\" \"{}\"\n", expected_line)).unwrap();
    let mut perm = fs::metadata(&fake_java).unwrap().permissions();
    perm.set_mode(0o755); fs::set_permissions(&fake_java, perm).unwrap();
    // Prepend bindir to PATH
    let old_path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("{}:{}", bindir.display(), old_path));
    fs::write(&tests.join(format!("{}.toml", project)), format!(r#"
[project]
build = 'none'

[[tests]]
name = "dig"
input = ["java", "-cp", "$digital", "CLI", "test", "$project_tests/1bfa_test.dig"]
expected = "{}\n"
rubric = 1
"#, expected_line)).unwrap();

    // Create local repo dir
    let repo_dir = base.join("repo");
    fs::create_dir_all(&repo_dir).unwrap();

    let cfg = TestCfg { tests_path: tests.parent().unwrap().to_string_lossy().to_string(), digital_path: digital_path.to_string_lossy().to_string() };
    let mut runner = TestRunner::new(&cfg, false, false, false, project.to_string());
    let repo = Repo::local(repo_dir.to_string_lossy().to_string(), runner.project_subdir());
    let res = runner.test_repo(&repo, None).unwrap();
    assert_eq!(res.score, 1);
    assert_eq!(res.results.len(), 1);
}
