use std::collections::HashSet;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::thread;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::OnceLock;

#[cfg(unix)]
use std::os::unix::process::CommandExt;
#[cfg(unix)]
use libc::{self, pid_t};

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

#[cfg(unix)]
static PGIDS: OnceLock<Mutex<HashSet<pid_t>>> = OnceLock::new();
#[cfg(unix)]
fn track_pgid(pgid: pid_t) {
    let set = PGIDS.get_or_init(|| {
        // Install ctrl-c handler once to terminate process groups
        let s = Mutex::new(HashSet::new());
        let set_ptr: &'static Mutex<HashSet<pid_t>> = unsafe { std::mem::transmute(&s) };
        let _ = ctrlc::set_handler(move || {
            if let Ok(guard) = set_ptr.lock() {
                for &pg in guard.iter() { unsafe { libc::kill(-pg, libc::SIGTERM); } }
            }
        });
        s
    });
    if let Ok(mut guard) = set.lock() { guard.insert(pgid); }
}
#[cfg(unix)]
fn untrack_pgid(pgid: pid_t) {
    if let Some(m) = PGIDS.get() { if let Ok(mut guard) = m.lock() { guard.remove(&pgid); } }
}

pub fn exec_capture(cmdline: &[String], opts: &ExecOptions) -> Result<String, ExecError> {
    if cmdline.is_empty() { return Ok(String::new()); }
    let mut c = Command::new(&cmdline[0]);
    if cmdline.len() > 1 { c.args(&cmdline[1..]); }
    if let Some(cwd) = &opts.cwd { c.current_dir(cwd); }
    c.stdout(Stdio::piped());
    if opts.capture_stderr { c.stderr(Stdio::piped()); } else { c.stderr(Stdio::null()); }

    // Start new session/process group on Unix so we can terminate the whole group
    #[cfg(unix)]
    {
        unsafe {
            c.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

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

    // Reader thread for stderr (if captured)
    let err_handle = if opts.capture_stderr {
        let total_clone = total.clone();
        let out_clone = out_buf.clone();
        let mut stderr = child.stderr.take();
        Some(thread::spawn(move || {
            if let Some(mut s) = stderr.take() {
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
        }))
    } else { None };

    // Determine process group id (Unix)
    #[cfg(unix)]
    let pgid: Option<pid_t> = unsafe {
        let res = libc::getpgid(child.id() as pid_t);
        if res == -1 { None } else { Some(res) }
    };
    #[cfg(unix)]
    if let Some(pg) = pgid { track_pgid(pg); }

    // Poll for exit, timeout, or output limit
    loop {
        if let Some(_status) = child.try_wait().unwrap_or(None) { break; }
        if start.elapsed() > timeout {
            // Kill process group if available, escalate to SIGKILL after short grace period
            #[cfg(unix)]
            if let Some(pg) = pgid {
                unsafe { libc::kill(-pg, libc::SIGTERM); }
                thread::sleep(Duration::from_millis(200));
                // Only escalate if still running
                if child.try_wait().ok().flatten().is_none() {
                    crate::util::print_yellow("Escalating to SIGKILL\n");
                    unsafe { libc::kill(-pg, libc::SIGKILL); }
                }
            }
            let _ = child.kill();
            let _ = reader.join();
            if let Some(h) = err_handle { let _ = h.join(); }
            return Err(ExecError::Timeout(timeout));
        }
        if total.load(Ordering::Relaxed) > opts.output_limit {
            #[cfg(unix)]
            if let Some(pg) = pgid {
                unsafe { libc::kill(-pg, libc::SIGTERM); }
                thread::sleep(Duration::from_millis(200));
                if child.try_wait().ok().flatten().is_none() {
                    crate::util::print_yellow("Escalating to SIGKILL\n");
                    unsafe { libc::kill(-pg, libc::SIGKILL); }
                }
            }
            let _ = child.kill();
            let _ = reader.join();
            if let Some(h) = err_handle { let _ = h.join(); }
            return Err(ExecError::OutputLimit(total.load(Ordering::Relaxed)));
        }
        thread::sleep(Duration::from_millis(10));
    }

    let _ = reader.join();
    if let Some(h) = err_handle { let _ = h.join(); }
    #[cfg(unix)]
    if let Some(pg) = pgid { untrack_pgid(pg); }
    let out = out_buf.lock().unwrap();
    Ok(String::from_utf8_lossy(&out).to_string())
}
