use std::fs;

use autograder_rust::dates::Dates;

#[test]
fn dates_from_tests_path() {
    let tmp = tempfile::tempdir().unwrap();
    let tests = tmp.path().join("tests_repo");
    fs::create_dir_all(&tests).unwrap();
    fs::write(tests.join("dates.toml"), r#"
[projx]

[[projx.dates]]
suffix = "due"
date = "2025-03-11"
percentage = 1.0

[[projx.dates]]
suffix = "late"
date = "2025-03-18"
percentage = 0.5
"#).unwrap();

    let d = Dates::from_tests_path(tests.to_str().unwrap(), "projx").unwrap();
    assert_eq!(d.items.len(), 2);
    assert_eq!(d.items[0].suffix, "due");
    assert_eq!(d.items[1].suffix, "late");
}

