mod cli;
mod config;
mod util;
mod cmd;
mod testcases;
mod git;
mod github;
mod canvas;
mod dates;
mod rollup;

use crate::cli::{Cli, Commands};
use crate::config::Config;
use crate::testcases::{TestRunner, Repo};
use crate::util::{print_green, print_red};
use crossbeam_channel;

fn main() {
    let cli = Cli::parse();

    // Load config (create defaults if missing)
    let cfg_path = config::resolve_config_path();
    let config = match Config::load_or_create(&cfg_path) {
        Ok(c) => c,
        Err(e) => {
            print_red(&format!("Failed to load config: {}\n", e));
            std::process::exit(1);
        }
    };

    match &cli.command {
        Commands::Test { project, test_name, verbose, very_verbose, unified_diff, quiet, no_color } => {
            util::set_color_enabled(!*no_color && std::env::var("NO_COLOR").is_err());
            if *verbose {
                if let Some(dir) = cfg_path.parent() {
                    println!("Config directory: {}", dir.display());
                }
            }
            let project_name = project.clone().unwrap_or_else(|| util::project_from_cwd());
            let mut runner = TestRunner::new(&config.test, *verbose, *very_verbose, *unified_diff, project_name.clone());
            if *quiet { runner.set_quiet(true); }
            let repo = Repo::local(".".into(), runner.project_subdir());
            let res = runner.test_repo(&repo, test_name.as_deref());
            if let Err(e) = res {
                print_red(&format!("{}\n", e));
                std::process::exit(1);
            }
        }
        Commands::Class { project, verbose, very_verbose, unified_diff, github_action, students, by_date, jobs, quiet, no_color } => {
            util::set_color_enabled(!*no_color && std::env::var("NO_COLOR").is_err());
            let list: Vec<String> = if let Some(list) = students { list.clone() } else { config.config.students.clone() };
            if list.is_empty() { print_red("No students provided and Config.students is empty\n"); std::process::exit(2); }

            let project_name = project.clone().unwrap_or_else(|| util::project_from_cwd());

            if *github_action {
                // Use GitHub API path
                let gh = match github::Github::new(config.github.clone(), config.git.org.clone(), project_name.clone(), *verbose) {
                    Ok(g) => g,
                    Err(e) => { print_red(&format!("GitHub client init failed: {}\n", e)); std::process::exit(2); }
                };
                let mut class_results = vec![];
                let repos: Vec<Repo> = list.iter().map(|s| Repo::student(project_name.clone(), s.clone(), None, None)).collect();
                let longest = repos.iter().map(|r| r.display_label.len()).max().unwrap_or(0) + 1;
                for s in list.iter() {
                    let r = Repo::student(project_name.clone(), s.clone(), None, None);
                    util::print_justified(&r.display_label, longest);
                    let rr = gh.get_action_results(s);
                    println!("{}", rr.score);
                    class_results.push(rr);
                }
                // Persist results
                let runner = TestRunner::new(&config.test, *verbose, *very_verbose, *unified_diff, project_name.clone());
                if let Err(e) = runner.write_class_json(&class_results, None) { print_red(&format!("{}\n", e)); std::process::exit(3); }
            } else {
                // Local test runner path
                let mut runner = TestRunner::new(&config.test, *verbose, *very_verbose, *unified_diff, project_name.clone());
                // Suppress internal per-test and trailing prints; we'll print summaries ourselves
                runner.set_quiet(true);
                let (suffix_opt, _date_opt) = if *by_date {
                let d = match dates::Dates::from_tests_path(&runner.tests_path, &project_name) {
                    Ok(d) => d,
                    Err(e) => { print_red(&format!("{}\n", e)); std::process::exit(2); }
                };
                match d.select() { Some(sel) => (Some(sel.suffix.clone()), Some(sel.date.clone())), None => { return; } }
            } else { (None, None) };
                let repos: Vec<Repo> = list.into_iter().map(|s| Repo::student(project_name.clone(), s, runner.project_subdir(), suffix_opt.clone())).collect();
                let longest = repos.iter().map(|r| r.display_label.len()).max().unwrap_or(0) + 1;
                // Avoid interleaved stdout noise when verbose; run single-threaded then
                let threads = if *verbose || *very_verbose { 1 } else { jobs.unwrap_or_else(num_cpus) };
                let mut class_results: Vec<(Repo, testcases::RepoResult)> = vec![];
                if threads == 1 {
                    // Sequential execution to avoid deadlock when the scope runs on the single worker
                    for r in &repos {
                        let mut runner_local = TestRunner::new(&config.test, *verbose, *very_verbose, *unified_diff, project_name.clone());
                        runner_local.set_quiet(true);
                        match runner_local.test_repo(r, None) {
                            Ok(rr) => {
                                util::print_justified(&r.display_label, longest);
                                if rr.results.is_empty() { println!("{}", rr.comment); }
                                else {
                                    for t in &rr.results {
                                        let tok = crate::util::format_pass_fail(&t.test, t.rubric, t.score);
                                        if t.score == t.rubric { crate::util::print_green(&tok); } else { crate::util::print_red(&tok); }
                                    }
                                    println!("{}", crate::testcases::TestRunner::make_earned_avail_static(&rr.results));
                                }
                                class_results.push((r.clone(), rr));
                            }
                            Err(e) => print_red(&format!("{}\n", e)),
                        }
                    }
                } else {
                    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
                    let mut next_to_print: usize = 0;
                    let mut pending: std::collections::HashMap<String, testcases::RepoResult> = std::collections::HashMap::new();
                    pool.scope(|s| {
                        let (tx, rx) = crossbeam_channel::unbounded();
                        for r in &repos {
                            let r = r.clone();
                            let tx = tx.clone();
                            // Clone minimal runner state per thread by creating a new runner
                            let mut runner_local = TestRunner::new(&config.test, *verbose, *very_verbose, *unified_diff, project_name.clone());
                            runner_local.set_quiet(true);
                            s.spawn(move |_| {
                                let res = runner_local.test_repo(&r, None).map(|rr| (r, rr));
                                let _ = tx.send(res);
                            });
                        }
                        drop(tx);
                        for res in rx.iter() {
                            match res {
                                Ok((repo_done, rr)) => {
                                    // Buffer result, then print any ready entries in original order
                                    pending.insert(repo_done.display_label.clone(), rr.clone());
                                    class_results.push((repo_done, rr));
                                    while next_to_print < repos.len() {
                                        let lbl = &repos[next_to_print].display_label;
                                        if let Some(rrp) = pending.remove(lbl) {
                                            util::print_justified(lbl, longest);
                                            if rrp.results.is_empty() {
                                                println!("{}", rrp.comment);
                                            } else {
                                                for t in &rrp.results {
                                                    let tok = crate::util::format_pass_fail(&t.test, t.rubric, t.score);
                                                    if t.score == t.rubric { crate::util::print_green(&tok); } else { crate::util::print_red(&tok); }
                                                }
                                                println!("{}", crate::testcases::TestRunner::make_earned_avail_static(&rrp.results));
                                            }
                                            next_to_print += 1;
                                        } else { break; }
                                    }
                                }
                                Err(e) => print_red(&format!("{}\n", e)),
                            }
                        }
                    });
                }
                // Persist histogram and JSON
                let mut only_results: Vec<testcases::RepoResult> = class_results.into_iter().map(|(_, rr)| rr).collect();
                // Prepend commit header to comments for JSON persistence
                for rr in &mut only_results {
                    if let Some(stu) = &rr.student {
                        let repo_dir = std::path::PathBuf::from(format!("./{}-{}{}", project_name, stu, suffix_opt.as_deref().map(|s| format!("-{}", s)).unwrap_or_default()));
                        if repo_dir.is_dir() {
                            if let Some(h) = git::Git::get_short_hash(&repo_dir) {
                                let url = format!("https://github.com/{}/{}-{}/tree/{}", config.git.org, project_name, stu, h);
                                let header = format!("Test results for repo as of this commit: {}\n\n", url);
                                if !rr.comment.starts_with("Test results for repo as of this commit:") {
                                    rr.comment = format!("{}{}", header, rr.comment.clone());
                                }
                            }
                        }
                    }
                }
                runner.print_histogram(&only_results);
                if let Err(e) = runner.write_class_json(&only_results, suffix_opt.as_deref()) { print_red(&format!("{}\n", e)); std::process::exit(3); }
            }
        }
        Commands::Exec { project, exec_cmd, students, jobs, by_date } => {
            // Build repo list from students (like pull), honoring project subdir
            let list: Vec<String> = if let Some(list) = students { list.clone() } else { config.config.students.clone() };
            if list.is_empty() {
                print_red("No students provided and Config.students is empty\n");
                std::process::exit(2);
            }
            let project_name = project.clone().unwrap_or_else(|| util::project_from_cwd());
            let runner = TestRunner::new(&config.test, false, false, false, project_name.clone());
            let suffix_opt: Option<String> = if *by_date {
                let d = match dates::Dates::from_tests_path(&runner.tests_path, &project_name) {
                    Ok(d) => d,
                    Err(e) => { print_red(&format!("{}\n", e)); std::process::exit(2); }
                };
                match d.select() { Some(sel) => Some(sel.suffix.clone()), None => { return; } }
            } else { None };
            let mut repos = vec![];
            for s in list { repos.push(testcases::Repo::student(project_name.clone(), s, runner.project_subdir(), suffix_opt.clone())); }
            let longest = repos.iter().map(|r| r.display_label.len()).max().unwrap_or(0) + 1;

            // Parallel execution with ordered, incremental printing
            let threads = jobs.unwrap_or_else(num_cpus);
            if threads == 1 {
                for r in &repos {
                    use crate::cmd::{exec_capture, ExecOptions};
                    util::print_justified(&r.display_label, longest);
                    let opts = ExecOptions { cwd: Some(r.local_path.to_string_lossy().to_string()), ..Default::default() };
                    let cmdline = vec![String::from("/bin/sh"), String::from("-c"), exec_cmd.clone()];
                    match exec_capture(&cmdline, &opts) {
                        Ok(out) => println!("{}", out),
                        Err(e) => println!("{}", match e {
                            crate::cmd::ExecError::Timeout(_) => "Command timed out".into(),
                            crate::cmd::ExecError::OutputLimit(_) => "Output limit exceeded".into(),
                            crate::cmd::ExecError::Io(ioe) => format!("IO error: {}", ioe),
                        }),
                    }
                }
            } else {
                let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
                let mut next_to_print: usize = 0;
                let mut pending: std::collections::HashMap<String, String> = std::collections::HashMap::new();
                pool.scope(|s| {
                    let (tx, rx) = crossbeam_channel::unbounded();
                    for r in &repos {
                        let r = r.clone();
                        let tx = tx.clone();
                        let cmd = exec_cmd.clone();
                        s.spawn(move |_| {
                            use crate::cmd::{exec_capture, ExecOptions};
                            let opts = ExecOptions { cwd: Some(r.local_path.to_string_lossy().to_string()), ..Default::default() };
                            let cmdline = vec![String::from("/bin/sh"), String::from("-c"), cmd];
                            let output = match exec_capture(&cmdline, &opts) {
                                Ok(out) => out,
                                Err(e) => match e {
                                    crate::cmd::ExecError::Timeout(_) => "Command timed out".into(),
                                    crate::cmd::ExecError::OutputLimit(_) => "Output limit exceeded".into(),
                                    crate::cmd::ExecError::Io(ioe) => format!("IO error: {}", ioe),
                                }
                            };
                            let _ = tx.send((r.display_label.clone(), output));
                        });
                    }
                    drop(tx);
                    for (lbl, out) in rx.iter() {
                        pending.insert(lbl.clone(), out);
                        while next_to_print < repos.len() {
                            let elbl = &repos[next_to_print].display_label;
                            if let Some(text) = pending.remove(elbl) {
                                util::print_justified(elbl, longest);
                                println!("{}", text);
                                next_to_print += 1;
                            } else { break; }
                        }
                    }
                });
            }
        }
        Commands::Clone { project, students, verbose, date, by_date } => {
            let list: Vec<String> = if let Some(list) = students { list.clone() } else { config.config.students.clone() };
            if list.is_empty() {
                print_red("No students provided and Config.students is empty\n");
                std::process::exit(2);
            }
            let g = git::Git::new(config.git.clone());
            let project_name = project.clone().unwrap_or_else(|| util::project_from_cwd());
            let (suffix_opt, date_opt) = if *by_date {
                let runner = TestRunner::new(&config.test, false, false, false, project_name.clone());
                let d = match dates::Dates::from_tests_path(&runner.tests_path, &project_name) {
                    Ok(d) => d,
                    Err(e) => { print_red(&format!("{}\n", e)); std::process::exit(2); }
                };
                match d.select() { Some(sel) => (Some(sel.suffix.clone()), Some(sel.date.clone())), None => { return; } }
            } else { (None, date.clone()) };

            let mut repos = vec![];
            for s in list { repos.push(testcases::Repo::student(project_name.clone(), s, None, suffix_opt.clone())); }
            let longest = repos.iter().map(|r| r.display_label.len()).max().unwrap_or(0) + 1;
            for r in repos.iter() {
                util::print_justified(&r.display_label, longest);
                g.clone_repo(&project_name, r, date_opt.as_deref(), *verbose);
            }
        }
        Commands::Pull { project, students } => {
            let list: Vec<String> = if let Some(list) = students { list.clone() } else { config.config.students.clone() };
            if list.is_empty() {
                print_red("No students provided and Config.students is empty\n");
                std::process::exit(2);
            }
            let g = git::Git::new(config.git.clone());
            let project_name = project.clone().unwrap_or_else(|| util::project_from_cwd());
            let mut repos = vec![];
            for s in list { repos.push(testcases::Repo::student(project_name.clone(), s, None, None)); }
            let longest = repos.iter().map(|r| r.display_label.len()).max().unwrap_or(0) + 1;
            for r in repos.iter() {
                util::print_justified(&r.display_label, longest);
                g.pull_repo(r);
                println!();
            }
        }
        Commands::Upload { project, file, verbose } => {
            let project_name = project.clone().unwrap_or_else(|| util::project_from_cwd());
            if let Err(e) = canvas::upload_class(config.canvas.clone(), config.canvas_mapper.clone(), &project_name, file.as_deref(), *verbose) {
                print_red(&format!("{}\n", e));
                std::process::exit(64);
            }
        }
        Commands::Rollup { project, by_date: _ } => {
                let project_name = project.clone().unwrap_or_else(|| util::project_from_cwd());
                let runner = TestRunner::new(&config.test, false, false, false, project_name.clone());
                let d = match dates::Dates::from_tests_path(&runner.tests_path, &project_name) { Ok(d) => d, Err(e) => { print_red(&format!("{}\n", e)); std::process::exit(2); } };
                if let Err(e) = rollup::rollup(&project_name, &d.items) { print_red(&format!("{}\n", e)); std::process::exit(64); }
        }
    }

    print_green("\nDone\n");
}

fn num_cpus() -> usize { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) }
