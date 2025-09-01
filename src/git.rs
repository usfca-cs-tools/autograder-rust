use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::config::GitCfg;
use crate::testcases::Repo;
use crate::util::{print_red};

pub struct Git {
    cfg: GitCfg,
}

impl Git {
    pub fn new(cfg: GitCfg) -> Self { Git { cfg } }

    fn remote_url(&self, repo_path: &str) -> String {
        match self.cfg.credentials.as_str() {
            "ssh" => format!("git@github.com:{}/{}.git", self.cfg.org, repo_path),
            "https" => format!("https://github.com/{}/{}", self.cfg.org, repo_path),
            other => {
                print_red(&format!("unknown Git.credentials: {}\n", other));
                format!("git@github.com:{}/{}.git", self.cfg.org, repo_path)
            }
        }
    }

    fn run_capture(args: &[&str], cwd: Option<&PathBuf>) -> anyhow::Result<String> {
        let mut cmd = Command::new(args[0]);
        if args.len() > 1 { cmd.args(&args[1..]); }
        if let Some(dir) = cwd { cmd.current_dir(dir); }
        let out = cmd.output()?;
        let mut text = String::from_utf8_lossy(&out.stdout).to_string();
        if !out.status.success() {
            // Include stderr for context
            let err = String::from_utf8_lossy(&out.stderr);
            text.push_str(&err);
        }
        Ok(text)
    }

    fn run_ok(args: &[&str], cwd: Option<&PathBuf>) -> anyhow::Result<bool> {
        let mut cmd = Command::new(args[0]);
        if args.len() > 1 { cmd.args(&args[1..]); }
        if let Some(dir) = cwd { cmd.current_dir(dir); }
        let status = cmd.status()?;
        Ok(status.success())
    }

    fn run_ok_quiet(args: &[&str], cwd: Option<&PathBuf>) -> anyhow::Result<bool> {
        let mut cmd = Command::new(args[0]);
        if args.len() > 1 { cmd.args(&args[1..]); }
        if let Some(dir) = cwd { cmd.current_dir(dir); }
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());
        let status = cmd.status()?;
        Ok(status.success())
    }

    fn get_default_branch(local: &PathBuf) -> anyhow::Result<String> {
        let s = Self::run_capture(&["git", "remote", "show", "origin"], Some(local))?;
        for line in s.lines() {
            if let Some(idx) = line.find("HEAD branch:") {
                let b = line[idx + "HEAD branch:".len()..].trim();
                if !b.is_empty() { return Ok(b.to_string()); }
            }
        }
        anyhow::bail!("No default branch detected")
    }

    fn get_commit_before(local: &PathBuf, branch: &str, date: &str) -> anyhow::Result<String> {
        // If time not present, default to midnight
        let has_time = date.contains(' ');
        let before = if has_time { date.to_string() } else { format!("{} 00:00:00", date) };
        // Use a simpler form that returns only hash
        let arg = format!("--before={}", before);
        let s = Self::run_capture(&["git", "rev-list", "-n", "1", "--first-parent", &arg, branch], Some(local))?;
        let hash = s.lines().next().unwrap_or("").trim().to_string();
        if hash.is_empty() { anyhow::bail!("No commits in date range"); }
        Ok(hash)
    }

    pub fn get_short_hash(local: &PathBuf) -> Option<String> {
        if !local.is_dir() { return None; }
        let s = Self::run_capture(&["git", "rev-parse", "--short", "HEAD"], Some(local)).ok()?;
        let h = s.lines().next().unwrap_or("").trim().to_string();
        if h.is_empty() { None } else { Some(h) }
    }

    pub fn clone_repo(&self, project: &str, repo: &Repo, date: Option<&str>, verbose: bool) {
        // repo.local_path is ./project-student
        if repo.local_path.is_dir() {
            println!("Already exists: {}", repo.local_path.display());
            return;
        }
        let remote = format!("{}-{}", project, repo.student.clone().unwrap_or_default());
        let url = self.remote_url(&remote);
        let ok = if verbose {
            Self::run_ok(&["git", "clone", &url, &repo.local_path.to_string_lossy()], None)
        } else {
            Self::run_ok_quiet(&["git", "clone", &url, &repo.local_path.to_string_lossy()], None)
        }
            .unwrap_or(false);
        if !ok { print_red("No remote repo or clone failed\n"); return; }

        if let Some(d) = date {
            // Checkout commit before date on default branch
            match Self::get_default_branch(&repo.local_path) {
                Ok(branch) => match Self::get_commit_before(&repo.local_path, &branch, d) {
                    Ok(hash) => {
                        let _ = if verbose {
                            Self::run_ok(&["git", "checkout", &hash], Some(&repo.local_path))
                        } else {
                            Self::run_ok_quiet(&["git", "checkout", &hash], Some(&repo.local_path))
                        };
                    },
                    Err(_) => {
                        // No commits in range: remove local repo to match Python behavior
                        let _ = Self::run_ok_quiet(&["rm", "-rf", &repo.local_path.to_string_lossy()], None);
                    }
                },
                Err(_) => {}
            }
        }
        println!();
    }

    pub fn pull_repo(&self, repo: &Repo) {
        if !repo.local_path.is_dir() {
            print_red(&format!("Local repo {} does not exist\n", repo.local_path.display()));
            return;
        }
        if let Ok(branch) = Self::get_default_branch(&repo.local_path) {
            let _ = Self::run_ok(&["git", "checkout", &branch], Some(&repo.local_path));
        }
        let _ = Self::run_ok(&["git", "pull"], Some(&repo.local_path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn remote_url_formats() {
        let g_ssh = Git::new(GitCfg { org: "orgx".into(), credentials: "ssh".into() });
        let g_https = Git::new(GitCfg { org: "orgx".into(), credentials: "https".into() });
        assert_eq!(g_ssh.remote_url("projx-alice"), "git@github.com:orgx/projx-alice.git");
        assert_eq!(g_https.remote_url("projx-alice"), "https://github.com/orgx/projx-alice");
    }

    #[test]
    fn default_branch_parsing_via_fake_git() {
        let tmp = tempfile::tempdir().unwrap();
        // Place a fake git binary at the front of PATH
        let bindir = tmp.path().join("bin");
        fs::create_dir_all(&bindir).unwrap();
        let fake_git = bindir.join("git");
        let script = "#!/bin/sh\nif [ \"$1\" = remote ] && [ \"$2\" = show ] && [ \"$3\" = origin ]; then\n  echo '  HEAD branch: main'\n  exit 0\nfi\necho unknown >&2\nexit 1\n";
        fs::write(&fake_git, script).unwrap();
        let mut perm = fs::metadata(&fake_git).unwrap().permissions();
        #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; perm.set_mode(0o755); }
        fs::set_permissions(&fake_git, perm).unwrap();

        // Prepend to PATH
        let old_path = std::env::var("PATH").unwrap_or_default();
        let new_path = format!("{}:{}", bindir.display(), old_path);
        std::env::set_var("PATH", &new_path);

        let cwd = tmp.path().join("repo");
        fs::create_dir_all(&cwd).unwrap();
        let branch = Git::get_default_branch(&cwd).unwrap();
        assert_eq!(branch, "main");
    }
}
