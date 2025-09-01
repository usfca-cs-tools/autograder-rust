use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::cmd::{exec_capture, ExecOptions};
use crate::config::TestCfg;
use crate::util::{expand_tilde, format_pass_fail, normalize_lines, print_green, print_red, print_yellow};

#[derive(Debug, Deserialize, Clone)]
pub struct ProjectCfg {
    #[serde(default = "default_build")] pub build: String,
    #[serde(default)] pub strip_output: Option<String>,
    #[serde(default)] pub subdir: Option<String>,
    #[serde(default = "default_timeout")] pub timeout: u64,
    #[serde(default = "default_capture_stderr")] pub capture_stderr: bool,
}
fn default_build() -> String { "make".into() }
fn default_timeout() -> u64 { 60 }
fn default_capture_stderr() -> bool { true }

#[derive(Debug, Deserialize, Clone)]
pub struct TestCaseCfg {
    #[serde(default)] pub case_sensitive: bool,
    pub expected: String,
    pub input: Vec<String>,
    pub name: String,
    #[serde(default = "default_output")] pub output: String,
    #[serde(default)] pub rubric: i64,
}
fn default_output() -> String { "stdout".into() }

#[derive(Debug, Deserialize, Clone)]
pub struct ProjectToml {
    #[serde(default)] pub project: Option<ProjectCfg>,
    #[serde(default)] pub tests: Vec<TestCaseCfg>,
}

#[derive(Debug, Clone)]
pub struct Repo {
    pub student: Option<String>,
    pub display_label: String,
    pub local_path: PathBuf,
}

impl Repo {
    pub fn local(local: String, subdir: Option<String>) -> Self {
        let mut p = PathBuf::from(local);
        if let Some(sd) = subdir { p.push(sd); }
        let lbl = p.to_string_lossy().to_string();
        Repo { student: None, display_label: lbl, local_path: p }
    }
    pub fn student(project: String, student: String, subdir: Option<String>, suffix: Option<String>) -> Self {
        let mut path = format!("{}-{}", project, student);
        if let Some(suf) = suffix { if !suf.is_empty() { path = format!("{}-{}", path, suf); } }
        let mut p = PathBuf::from(".");
        p.push(&path);
        if let Some(sd) = subdir { p.push(sd); }
        Repo { student: Some(student), display_label: p.to_string_lossy().to_string(), local_path: p }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TcResult {
    pub rubric: i64,
    pub score: i64,
    pub test: String,
    #[serde(skip_serializing_if = "Option::is_none")] pub test_err: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RepoResult {
    pub comment: String,
    pub results: Vec<TcResult>,
    pub score: i64,
    #[serde(skip_serializing_if = "Option::is_none")] pub student: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")] pub build_err: Option<String>,
}

pub struct TestRunner {
    pub(crate) tests_path: String,
    digital_path: String,
    verbose: bool,
    very_verbose: bool,
    unified_diff: bool,
    quiet: bool,
    project: String,
    project_cfg: ProjectCfg,
    testcases: Vec<TestCaseCfg>,
}

impl TestRunner {
    pub fn new(cfg: &TestCfg, verbose: bool, very_verbose: bool, unified_diff: bool, project: String) -> Self {
        let tests_path = expand_tilde(&cfg.tests_path);
        let digital_path = expand_tilde(&cfg.digital_path);
        TestRunner { tests_path, digital_path, verbose, very_verbose, unified_diff, quiet: false, project, project_cfg: ProjectCfg { build: default_build(), strip_output: None, subdir: None, timeout: default_timeout(), capture_stderr: default_capture_stderr() }, testcases: vec![] }
    }

    pub fn set_quiet(&mut self, quiet: bool) { self.quiet = quiet; }

    pub fn project_subdir(&self) -> Option<String> { self.project_cfg.subdir.clone() }

    fn load_testcases(&mut self) -> anyhow::Result<()> {
        let path = Path::new(&self.tests_path).join(&self.project).join(format!("{}.toml", &self.project));
        let content = fs::read_to_string(&path).map_err(|e| anyhow::anyhow!("File not found: {} ({})", path.display(), e))?;
        let doc: ProjectToml = toml::from_str(&content).map_err(|e| anyhow::anyhow!("Failed to parse {}: {}", path.display(), e))?;
        if let Some(pcfg) = doc.project { self.project_cfg = pcfg; }
        self.testcases = doc.tests;
        if self.testcases.is_empty() {
            print_yellow(&format!("No test cases found: {}\n", path.display()));
        }
        Ok(())
    }

    fn interpolate(&self, s: &str, tc_name: &str) -> String {
        let mut out = s.replace("$project", &self.project);
        let proj_tests = Path::new(&self.tests_path).join(&self.project).to_string_lossy().to_string();
        out = out.replace("$project_tests", &proj_tests);
        out = out.replace("$digital", &self.digital_path);
        out = out.replace("$name", tc_name);
        out
    }

    fn build(&self, repo: &Repo) -> Option<String> {
        match self.project_cfg.build.as_str() {
            "none" => None,
            "make" => {
                if !repo.local_path.exists() { return Some(format!("Repo not found: {}", repo.local_path.display())); }
                let mfu = repo.local_path.join("Makefile");
                let mfl = repo.local_path.join("makefile");
                if !mfu.is_file() && !mfl.is_file() { return Some(format!("Makefile not found: {}", mfu.display())); }
                let cmd = vec!["make".to_string(), "-C".to_string(), repo.local_path.to_string_lossy().to_string()];
                let opts = ExecOptions { cwd: None, timeout: Duration::from_secs(30), capture_stderr: true, output_limit: 220_000 };
                match crate::cmd::exec_capture_with_status(&cmd, &opts) {
                    Ok((_out, true, _)) => None,
                    Ok((_out, false, _)) => Some("Program did not make successfully".into()),
                    Err(_) => Some("Program did not make successfully".into()),
                }
            }
            other => Some(format!("Unknown build plan: \"{}\"", other)),
        }
    }

    fn run_one_test(&self, repo: &Repo, tc: &TestCaseCfg) -> TcResult {
        let mut result = TcResult { rubric: tc.rubric, score: 0, test: tc.name.clone(), test_err: None };
        let timeout = Duration::from_secs(self.project_cfg.timeout);
        let opts = ExecOptions { cwd: Some(repo.local_path.to_string_lossy().to_string()), timeout, capture_stderr: self.project_cfg.capture_stderr, output_limit: 220_000 };

        let mut cmdline: Vec<String> = vec![];
        for i in tc.input.iter() { cmdline.push(self.interpolate(i, &tc.name)); }
        let actual_res = if tc.output == "stdout" {
            match crate::cmd::exec_capture_with_status(&cmdline, &opts) {
                Ok((out, true, _)) => Ok(out),
                Ok((out, false, code)) => {
                    let lower = out.to_lowercase();
                    let enoexec_like = lower.contains("exec format error") || matches!(code, Some(126)|Some(193));
                    if enoexec_like { Err(crate::cmd::ExecError::Io(std::io::Error::from_raw_os_error(8))) } else { Ok(out) }
                }
                Err(e) => Err(e),
            }
        } else {
            let _ = exec_capture(&cmdline, &opts);
            let f = repo.local_path.join(&tc.output);
            fs::read_to_string(&f).map_err(|e| crate::cmd::ExecError::Io(e))
        };

        match actual_res {
            Ok(mut actual) => {
                if let Some(strip) = &self.project_cfg.strip_output { actual = actual.replace(strip, ""); }
                if self.match_expected(&tc, &actual) { result.score = tc.rubric; }
                self.print_verbose(&tc, &cmdline, &actual);
            }
            Err(e) => {
                let msg = match e {
                    crate::cmd::ExecError::Timeout(_) => "Program timed out (infinite loop?)".to_string(),
                    crate::cmd::ExecError::OutputLimit(_) => "Program produced too much output (infinite loop?)".to_string(),
                    crate::cmd::ExecError::Io(ref ioe) if ioe.raw_os_error() == Some(8) => {
                        let exe = cmdline.get(0).cloned().unwrap_or_else(|| "./program".into());
                        format!("OSError: [Errno 8] Exec format error: '{}'", exe)
                    }
                    crate::cmd::ExecError::Io(ref ioe) if ioe.kind() == std::io::ErrorKind::NotFound =>
                        "Program not found (build failed?)".to_string(),
                    crate::cmd::ExecError::Io(ioe) => format!("IO error: {}", ioe),
                };
                result.test_err = Some(msg);
            }
        }

        if !self.quiet {
            let fmt = format_pass_fail(&result.test, result.rubric, result.score);
            if result.score == 0 { print_red(&format!("{}\n", fmt)); } else { print_green(&format!("{}\n", fmt)); }
        }
        result
    }

    fn match_expected(&self, tc: &TestCaseCfg, actual: &str) -> bool {
        let exp = self.interpolate(&tc.expected, &tc.name);
        let lhs = normalize_lines(&exp.trim_end(), tc.case_sensitive);
        let rhs = normalize_lines(&actual.trim_end(), tc.case_sensitive);
        lhs == rhs
    }

    fn print_verbose(&self, tc: &TestCaseCfg, cmdline: &[String], actual: &str) {
        let cmd_display = cmdline.iter().map(|s| if s.contains(' ') { format!("\"{}\"", s) } else { s.clone() }).collect::<Vec<_>>().join(" ");
        if self.very_verbose {
            println!("\n\n===[{}]===expected\n$ {}\n{}", tc.name, cmd_display, self.interpolate(&tc.expected, &tc.name));
            println!("\n===[{}]===actual\n$ {}\n{}", tc.name, cmd_display, actual);
        }
        if self.verbose {
            let exp = self.interpolate(&tc.expected, &tc.name);
            let lhs = normalize_lines(&exp.trim_end(), tc.case_sensitive);
            let rhs = normalize_lines(&actual.trim_end(), tc.case_sensitive);
            if lhs != rhs {
                if self.unified_diff {
                    use similar::{ChangeTag, TextDiff};
                    println!("--- expected\n+++ actual");
                    let diff = TextDiff::from_lines(exp.as_str(), actual);
                    for change in diff.iter_all_changes() {
                        match change.tag() {
                            ChangeTag::Delete => print!("-"),
                            ChangeTag::Insert => print!("+"),
                            ChangeTag::Equal => print!(" "),
                        }
                        print!("{}", change);
                    }
                } else {
                    crate::util::print_diff_header(&tc.name, &cmd_display);
                    crate::util::simple_diff(&lhs, &rhs, 50);
                }
            }
        }
    }

    fn make_comment(&self, repo_result: &RepoResult) -> String {
        // Match Python formatting of comment body
        let mut out = String::new();
        let mut pass_concat = String::new();
        let mut prefix = String::new();
        if let Some(be) = &repo_result.build_err { prefix.push_str(be); prefix.push(' '); }
        for r in &repo_result.results {
            let label = format_pass_fail(&r.test, r.rubric, r.score);
            if let Some(e) = &r.test_err {
                // If we have accumulated pass labels, keep them on the same line
                // with the first error label and message.
                if !pass_concat.is_empty() {
                    out.push_str(&prefix);
                    out.push_str(&pass_concat);
                    pass_concat.clear();
                    prefix.clear();
                } else {
                    out.push_str(&prefix);
                }
                // For error lines, trim trailing padding from the label to match Python
                out.push_str(label.trim_end());
                out.push_str("    ");
                out.push_str(e);
                out.push('\n');
                // After the first error, subsequent errors each go on their own line
                prefix.clear();
            } else {
                pass_concat.push_str(&label);
            }
        }
        let earned = self.make_earned_avail(repo_result);
        if !pass_concat.is_empty() {
            out.push_str(&prefix);
            out.push_str(&pass_concat);
            out.push_str(&earned);
        } else {
            if out.is_empty() { out.push_str(&prefix); }
            out.push_str(&earned);
        }
        out
    }

    fn make_earned_avail(&self, repo_result: &RepoResult) -> String {
        format!("{}/{}", repo_result.score, self.total_rubric())
    }

    pub fn make_earned_avail_static(results: &[TcResult]) -> String {
        let avail: i64 = results.iter().map(|t| t.rubric).sum();
        let earned: i64 = results.iter().map(|t| t.score).sum();
        format!("{}/{}", earned, avail)
    }

    pub fn test_repo(&mut self, repo: &Repo, only_name: Option<&str>) -> anyhow::Result<RepoResult> {
        self.load_testcases()?;
        if !repo.local_path.is_dir() {
            let msg = format!("Local repo {} does not exist", repo.local_path.display());
            if !self.quiet { print_red(&format!("{}\n", msg)); }
            return Ok(RepoResult { comment: msg, results: vec![], score: 0, student: repo.student.clone(), build_err: None });
        }

        let mut build_err = self.build(repo);
        let mut results = vec![];
        let iter = self.testcases.iter().filter(|tc| only_name.map(|n| n == tc.name).unwrap_or(true));
        for tc in iter { results.push(self.run_one_test(repo, tc)); }
        let score = results.iter().map(|r| r.score).sum();
        let mut repo_result = RepoResult { comment: String::new(), results, score, student: repo.student.clone(), build_err };
        repo_result.comment = self.make_comment(&repo_result);
        if !self.quiet { println!("{}", self.make_earned_avail(&repo_result)); }
        Ok(repo_result)
    }

    pub fn total_rubric(&self) -> i64 { self.testcases.iter().map(|tc| tc.rubric).sum() }

    pub fn print_histogram(&self, class_results: &[RepoResult]) {
        // Derive available points from any non-empty result set
        let mut avail = 0;
        for r in class_results {
            let s: i64 = r.results.iter().map(|t| t.rubric).sum();
            if s > 0 { avail = s; break; }
        }
        let mut freqs: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
        for r in class_results { *freqs.entry(r.score).or_default() += 1; }
        println!("\nScore frequency (n = {})", class_results.len());
        let mut items: Vec<(i64, usize)> = freqs.into_iter().collect();
        items.sort_by(|a, b| b.0.cmp(&a.0)); // descending by score
        for (score, freq) in items { let pct = (freq as f64) / (class_results.len() as f64) * 100.0; println!("{}/{}: {}  ({:.1}%)", score, avail, freq, pct); }
    }

    pub fn write_class_json(&self, class_results: &[RepoResult], suffix: Option<&str>) -> anyhow::Result<()> {
        let fname = if let Some(s) = suffix { format!("{}-{}.json", self.project, s) } else { format!("{}.json", self.project) };
        let data = serde_json::to_string_pretty(class_results)?;
        fs::write(&fname, data)?;
        Ok(())
    }
}
