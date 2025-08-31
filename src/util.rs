use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

static COLOR_ENABLED: OnceLock<AtomicBool> = OnceLock::new();

fn color_on() -> bool {
    COLOR_ENABLED.get_or_init(|| AtomicBool::new(true)).load(Ordering::Relaxed)
}

pub fn set_color_enabled(enabled: bool) {
    COLOR_ENABLED.get_or_init(|| AtomicBool::new(true)).store(enabled, Ordering::Relaxed);
}

pub fn print_green(s: &str) { if color_on() { print!("\x1b[92m{}\x1b[0m", s); } else { print!("{}", s); } }
pub fn print_yellow(s: &str) { if color_on() { print!("\x1b[93m{}\x1b[0m", s); } else { print!("{}", s); } }
pub fn print_red(s: &str) { if color_on() { print!("\x1b[91m{}\x1b[0m", s); } else { print!("{}", s); } }

pub fn print_justified(s: &str, longest: usize) {
    print!("{}", s);
    if longest > s.len() {
        let pad = longest - s.len();
        for _ in 0..pad { print!(" "); }
    }
}

pub fn expand_tilde(p: &str) -> String {
    if let Some(stripped) = p.strip_prefix("~/") {
        return format!("{}/{}", home_dir().display(), stripped);
    }
    p.to_string()
}

pub fn home_dir() -> PathBuf {
    dirs_next::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

pub fn normalize_lines(s: &str, case_sensitive: bool) -> Vec<String> {
    s.split('\n')
        .map(|line| {
            let mut t = line.trim().to_string();
            if !case_sensitive { t = t.to_lowercase(); }
            t.push('\n');
            t
        })
        .collect()
}

pub fn format_pass_fail(name: &str, rubric: i64, score: i64) -> String {
    let max_len = format!("{}({}/{}) ", name, rubric, rubric).len();
    let mut s = format!("{}({}/{}) ", name, score, rubric);
    if s.len() < max_len { s.push_str(&" ".repeat(max_len - s.len())); }
    s
}

#[allow(dead_code)]
pub fn project_from_cwd() -> String {
    let cwd = std::env::current_dir().unwrap_or_default();
    let name = cwd.file_name().and_then(|n| n.to_str()).unwrap_or(".");
    if let Some(i) = name.find('-') { name[..i].to_string() } else { name.to_string() }
}

pub fn print_diff_header(name: &str, cmd: &str) {
    println!("\n\n===[{}]===diff\n$ {}", name, cmd);
}

pub fn simple_diff(expected: &[String], actual: &[String], max_lines: usize) {
    use std::cmp::min;
    let mut shown = 0;
    let len = min(expected.len().max(actual.len()), max_lines);
    for i in 0..len {
        let e = expected.get(i).map(|s| s.as_str()).unwrap_or("");
        let a = actual.get(i).map(|s| s.as_str()).unwrap_or("");
        if e != a {
            println!("- {}", e.trim_end());
            println!("+ {}", a.trim_end());
            shown += 1;
            if shown >= max_lines { break; }
        }
    }
}

// Minimal dependency wrapper: use dirs-next only here
mod dirs_next {
    pub fn home_dir() -> Option<std::path::PathBuf> {
        std::env::var_os("HOME").map(std::path::PathBuf::from)
    }
}
