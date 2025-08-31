use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::thread;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, thiserror::Error)]
pub enum ExecError {
    #[error("process timed out after {0:?}")] 
    Timeout(Duration),
    #[error("output exceeded limit: {0} bytes")] 
    OutputLimit(usize),
    #[error("io error: {0}")] 
    Io(#[from] std::io::Error),
}

pub struct ExecOptions {
    pub cwd: Option<String>,
    pub timeout: Duration,
    pub capture_stderr: bool,
    pub output_limit: usize,
}

impl Default for ExecOptions {
    fn default() -> Self {
        ExecOptions { cwd: None, timeout: Duration::from_secs(60), capture_stderr: true, output_limit: 220_000 }
    }
}

pub fn exec_capture(cmdline: &[String], opts: &ExecOptions) -> Result<String, ExecError> {
    if cmdline.is_empty() { return Ok(String::new()); }
    let mut c = Command::new(&cmdline[0]);
    if cmdline.len() > 1 { c.args(&cmdline[1..]); }
    if let Some(cwd) = &opts.cwd { c.current_dir(cwd); }
    c.stdout(Stdio::piped());
    if opts.capture_stderr { c.stderr(Stdio::piped()); } else { c.stderr(Stdio::null()); }

    let mut child = c.spawn()?;
    let start = Instant::now();
    let timeout = opts.timeout;

    let total = Arc::new(AtomicUsize::new(0));
    let out_buf = Arc::new(Mutex::new(Vec::<u8>::new()));

    // Reader thread for stdout
    let total_clone = total.clone();
    let out_clone = out_buf.clone();
    let mut stdout = child.stdout.take();
    let reader = thread::spawn(move || {
        if let Some(mut s) = stdout.take() {
            let mut buf = [0u8; 8192];
            loop {
                match s.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        total_clone.fetch_add(n, Ordering::Relaxed);
                        if let Ok(mut o) = out_clone.lock() { o.extend_from_slice(&buf[..n]); }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                }
            }
        }
    });

    // Poll for exit, timeout, or output limit
    loop {
        if let Some(_status) = child.try_wait().unwrap_or(None) { break; }
        if start.elapsed() > timeout {
            let _ = child.kill();
            let _ = reader.join();
            return Err(ExecError::Timeout(timeout));
        }
        if total.load(Ordering::Relaxed) > opts.output_limit {
            let _ = child.kill();
            let _ = reader.join();
            return Err(ExecError::OutputLimit(total.load(Ordering::Relaxed)));
        }
        thread::sleep(Duration::from_millis(10));
    }

    let _ = reader.join();
    let out = out_buf.lock().unwrap();
    Ok(String::from_utf8_lossy(&out).to_string())
}
