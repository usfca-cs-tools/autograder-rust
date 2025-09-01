use std::fs;

use autograder_rust::config::resolve_config_path;
use serial_test::serial;

fn set_env(k: &str, v: Option<&str>) {
    match v { Some(val) => std::env::set_var(k, val), None => std::env::remove_var(k) }
}

#[test]
#[serial]
fn config_path_env_overrides() {
    let tmp = tempfile::tempdir().unwrap();
    let cfgdir = tmp.path().join("cfg");
    fs::create_dir_all(&cfgdir).unwrap();
    set_env("GRADE_CONFIG_DIR", None);
    set_env("GRADE_CONFIG_DIR", Some(cfgdir.to_str().unwrap()));
    let p = resolve_config_path();
    assert_eq!(p, cfgdir.join("config.toml"));
}

#[test]
#[serial]
fn config_path_parent_traversal() {
    let tmp = tempfile::tempdir().unwrap();
    set_env("GRADE_CONFIG_DIR", None);
    let a = tmp.path().join("a");
    let b = a.join("b");
    fs::create_dir_all(&b).unwrap();
    fs::write(a.join("config.toml"), "# placeholder").unwrap();
    let _g = pushd::Pushd::new(&b);
    let p = resolve_config_path();
    let left = std::fs::canonicalize(p).unwrap();
    let right = std::fs::canonicalize(a.join("config.toml")).unwrap();
    assert_eq!(left, right);
}

#[test]
#[serial]
fn config_path_falls_back_home() {
    let tmp = tempfile::tempdir().unwrap();
    set_env("GRADE_CONFIG_DIR", None);
    set_env("GRADE_CONFIG_DIR", None);
    std::env::set_var("HOME", tmp.path());
    let wd = tmp.path().join("work/dir");
    fs::create_dir_all(&wd).unwrap();
    let _g = pushd::Pushd::new(&wd);
    let p = resolve_config_path();
    assert_eq!(p, tmp.path().join(".config/grade/config.toml"));
}
