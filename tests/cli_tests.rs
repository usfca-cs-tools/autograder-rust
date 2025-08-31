use std::fs;
use std::os::unix::fs::PermissionsExt;

use assert_cmd::prelude::*;
use predicates::str::contains as p_contains;
use std::process::Command;

fn write_mini_repo(base: &std::path::Path, program_name: &str) -> std::path::PathBuf {
    let repo = base.join("repo");
    fs::create_dir_all(&repo).unwrap();
    let prog = repo.join(program_name);
    fs::write(&prog, "#!/bin/sh\necho ok\n").unwrap();
    let mut perm = fs::metadata(&prog).unwrap().permissions();
    perm.set_mode(0o755); fs::set_permissions(&prog, perm).unwrap();
    fs::write(repo.join("Makefile"), "all:\n\t@echo built\n").unwrap();
    repo
}

fn write_tests_repo(base: &std::path::Path, project: &str) -> std::path::PathBuf {
    let tests = base.join("tests_repo").join(project);
    fs::create_dir_all(&tests).unwrap();
    fs::write(tests.join(format!("{}.toml", project)), r#"
[project]
build = 'make'

[[tests]]
name = "01"
input = ["./$project"]
expected = "ok"
rubric = 10
"#).unwrap();
    tests.parent().unwrap().to_path_buf()
}

#[test]
fn cli_test_subcommand_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let base = tmp.path();
    let project = "projx";
    let repo = write_mini_repo(base, project);
    let tests_repo = write_tests_repo(base, project);

    // Prepare config dir
    let cfgdir = base.join("cfg");
    fs::create_dir_all(&cfgdir).unwrap();
    fs::write(cfgdir.join("config.toml"), format!("[Test]\ntests_path = \"{}\"\n", tests_repo.to_string_lossy())).unwrap();

    // Set HOME to temp and GRADE_CONFIG_DIR to cfgdir
    let mut cmd = Command::cargo_bin("grade-rs").unwrap();
    cmd.arg("test").args(["-p", project])
        .env("HOME", base)
        .env("GRADE_CONFIG_DIR", &cfgdir)
        .current_dir(&repo);
    cmd.assert().success().stdout(p_contains("10/10"));
}

