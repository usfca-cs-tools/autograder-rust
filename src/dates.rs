use serde::Deserialize;
use std::fs;
use std::path::Path;

use crate::util::print_yellow;

#[derive(Debug, Deserialize, Clone)]
pub struct DateItem {
    pub suffix: String,
    pub date: String,
    pub percentage: f64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct DatesTable { pub dates: Vec<DateItem> }

#[derive(Debug, Deserialize, Clone)]
pub struct DatesDoc { #[serde(flatten)] pub projects: std::collections::HashMap<String, DatesTable> }

pub struct Dates { pub items: Vec<DateItem> }

impl Dates {
    pub fn from_tests_path(tests_path: &str, project: &str) -> anyhow::Result<Self> {
        let path = Path::new(tests_path).join("dates.toml");
        let content = fs::read_to_string(&path)?;
        let doc: DatesDoc = toml::from_str(&content)?;
        let table = doc.projects.get(project).ok_or_else(|| anyhow::anyhow!("No dates for project {} in {}", project, path.display()))?;
        Ok(Dates { items: table.dates.clone() })
    }

    pub fn select(&self) -> Option<&DateItem> {
        if self.items.is_empty() { return None; }
        if self.items.len() == 1 { return self.items.get(0); }
        match arrow_select(&self.items) {
            Some(i) => self.items.get(i),
            None => {
                // Fallback to numeric prompt
                println!("Select date:");
                for (i, d) in self.items.iter().enumerate() {
                    println!("{}: {} {}", i + 1, d.suffix, d.date);
                }
                print!("Enter choice [1-{}]: ", self.items.len());
                use std::io::{self, Write};
                let _ = io::stdout().flush();
                let mut s = String::new();
                let _ = io::stdin().read_line(&mut s);
                let idx: usize = s.trim().parse().unwrap_or(0);
                if idx == 0 || idx > self.items.len() { print_yellow("No selection made\n"); return None; }
                self.items.get(idx - 1)
            }
        }
    }
}

#[cfg(unix)]
fn is_tty() -> bool { unsafe { libc::isatty(0) == 1 } }

#[cfg(not(unix))]
fn is_tty() -> bool { false }

#[cfg(unix)]
fn set_raw_mode(enable: bool, old: &mut libc::termios) -> std::io::Result<()> {
    use std::mem::MaybeUninit;
    unsafe {
        if enable {
            let mut t = MaybeUninit::<libc::termios>::uninit();
            if libc::tcgetattr(0, t.as_mut_ptr()) != 0 { return Err(std::io::Error::last_os_error()); }
            *old = t.assume_init();
            let mut raw = old.clone();
            raw.c_lflag &= !(libc::ICANON | libc::ECHO);
            raw.c_cc[libc::VMIN] = 1;
            raw.c_cc[libc::VTIME] = 0;
            if libc::tcsetattr(0, libc::TCSANOW, &raw) != 0 { return Err(std::io::Error::last_os_error()); }
        } else {
            if libc::tcsetattr(0, libc::TCSANOW, old) != 0 { return Err(std::io::Error::last_os_error()); }
        }
    }
    Ok(())
}

fn arrow_select(items: &[DateItem]) -> Option<usize> {
    if !is_tty() { return None; }
    #[cfg(unix)]
    unsafe {
        use std::io::{Read, Write};
        let mut old = std::mem::zeroed();
        if set_raw_mode(true, &mut old).is_err() { return None; }
        struct RawGuard<'a> { old: &'a mut libc::termios }
        impl<'a> Drop for RawGuard<'a> { fn drop(&mut self) { let _ = set_raw_mode(false, self.old); } }
        let _guard = RawGuard { old: &mut old };

        // Hide cursor
        print!("\x1b[?25l");
        let _ = std::io::stdout().flush();
        struct CursorGuard;
        impl Drop for CursorGuard { fn drop(&mut self) { let _ = std::io::Write::write_all(&mut std::io::stdout(), b"\x1b[?25h\n"); let _ = std::io::stdout().flush(); } }
        let _cg = CursorGuard;

        let mut sel: isize = 0;
        // Initial draw
        fn draw(items: &[DateItem], sel: isize) {
            println!("Select date (use ↑/↓, Enter):");
            for (i, d) in items.iter().enumerate() {
                if i as isize == sel { print!("\x1b[7m> {} {}\x1b[27m\n", d.suffix, d.date); }
                else { println!("  {} {}", d.suffix, d.date); }
            }
            let _ = std::io::stdout().flush();
        }
        draw(items, sel);
        let mut stdin = std::io::stdin();
        let mut buf = [0u8; 3];
        loop {
            if stdin.read(&mut buf).ok()? == 0 { return None; }
            match buf {
                [b'\r', ..] | [b'\n', ..] => { println!(""); return Some(sel as usize); }
                [0x1b, b'[', b'A'] => { if sel > 0 { sel -= 1; } }
                [0x1b, b'[', b'B'] => { if sel < (items.len() as isize - 1) { sel += 1; } }
                [b'q', ..] => { print_yellow("No selection made\n"); return None; }
                _ => {}
            }
            // Move cursor up to redraw (1 for prompt + N items)
            let up = items.len() + 1;
            print!("\x1b[{}A", up);
            for _ in 0..up { print!("\x1b[2K\r\n"); }
            print!("\x1b[{}A", up);
            draw(items, sel);
            buf = [0;3];
        }
    }
    #[allow(unreachable_code)]
    None
}

#[cfg(not(unix))]
fn arrow_select(_items: &[DateItem]) -> Option<usize> { None }
