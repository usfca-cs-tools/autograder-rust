use std::fs;
use std::os::unix::fs::PermissionsExt;

use autograder_rust::cmd::{exec_capture, ExecOptions, ExecError};

#[test]
fn exec_timeout() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("sleep.sh");
    fs::write(&script, "#!/bin/sh\nsleep 1\n").unwrap();
    let mut perm = fs::metadata(&script).unwrap().permissions();
    perm.set_mode(0o755); fs::set_permissions(&script, perm).unwrap();

    let args = vec![script.to_string_lossy().to_string()];
    let opts = ExecOptions { cwd: None, timeout: std::time::Duration::from_millis(100), capture_stderr: true, output_limit: 220_000 };
    let res = exec_capture(&args, &opts);
    assert!(matches!(res, Err(ExecError::Timeout(_))));
}

#[test]
fn exec_output_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("spam.sh");
    // Print 1000 lines of 100 chars => 100k; we'll set limit lower to trigger
    fs::write(&script, "#!/bin/sh\ni=0\nwhile [ $i -lt 2000 ]; do\n  echo 012345678901234567890123456789012345678901234567890123456789012345678901234567890123456789\n  i=$((i+1))\ndone\n").unwrap();
    let mut perm = fs::metadata(&script).unwrap().permissions();
    perm.set_mode(0o755); fs::set_permissions(&script, perm).unwrap();

    let args = vec![script.to_string_lossy().to_string()];
    let opts = ExecOptions { cwd: None, timeout: std::time::Duration::from_secs(5), capture_stderr: true, output_limit: 10_000 };
    let res = exec_capture(&args, &opts);
    assert!(matches!(res, Err(ExecError::OutputLimit(_))));
}

