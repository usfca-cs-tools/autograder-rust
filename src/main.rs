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
            let mut runner = TestRunner::new(&config.test, *verbose, *very_verbose, *unified_diff, project.clone());
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

            if *github_action {
                // Use GitHub API path
                let gh = match github::Github::new(config.github.clone(), config.git.org.clone(), project.clone(), *verbose) {
                    Ok(g) => g,
                    Err(e) => { print_red(&format!("GitHub client init failed: {}\n", e)); std::process::exit(2); }
                };
                let mut class_results = vec![];
                let repos: Vec<Repo> = list.iter().map(|s| Repo::student(project.clone(), s.clone(), None, None)).collect();
                let longest = repos.iter().map(|r| r.display_label.len()).max().unwrap_or(0) + 1;
                for s in list.iter() {
                    let r = Repo::student(project.clone(), s.clone(), None, None);
                    util::print_justified(&r.display_label, longest);
                    let rr = gh.get_action_results(s);
                    println!("{}", rr.score);
                    class_results.push(rr);
                }
                // Persist results
                let runner = TestRunner::new(&config.test, *verbose, *very_verbose, *unified_diff, project.clone());
                if let Err(e) = runner.write_class_json(&class_results, None) { print_red(&format!("{}\n", e)); std::process::exit(3); }
            } else {
                // Local test runner path
                let mut runner = TestRunner::new(&config.test, *verbose, *very_verbose, *unified_diff, project.clone());
                if *quiet { runner.set_quiet(true); }
                let (suffix_opt, _date_opt) = if *by_date {
                    let d = match dates::Dates::from_tests_path(&runner.tests_path, project) {
                        Ok(d) => d,
                        Err(e) => { print_red(&format!("{}\n", e)); std::process::exit(2); }
                    };
                    match d.select() { Some(sel) => (Some(sel.suffix.clone()), Some(sel.date.clone())), None => (None, None) }
                } else { (None, None) };
                let repos: Vec<Repo> = list.into_iter().map(|s| Repo::student(project.clone(), s, runner.project_subdir(), suffix_opt.clone())).collect();
                let longest = repos.iter().map(|r| r.display_label.len()).max().unwrap_or(0) + 1;
                let pool = rayon::ThreadPoolBuilder::new().num_threads(jobs.unwrap_or_else(num_cpus)).build().unwrap();
                let mut class_results: Vec<testcases::RepoResult> = vec![];
                pool.scope(|s| {
                    let (tx, rx) = crossbeam_channel::unbounded();
                    for r in &repos {
                        let r = r.clone();
                        let tx = tx.clone();
                        // Clone minimal runner state per thread by creating a new runner
                        let mut runner_local = TestRunner::new(&config.test, *verbose, *very_verbose, *unified_diff, project.clone());
                        if *quiet { runner_local.set_quiet(true); }
                        s.spawn(move |_| {
                            util::print_justified(&r.display_label, longest);
                            let res = runner_local.test_repo(&r, None);
                            let _ = tx.send(res);
                        });
                    }
                    drop(tx);
                    for res in rx.iter() {
                        match res { Ok(rr) => class_results.push(rr), Err(e) => print_red(&format!("{}\n", e)) }
                    }
                });
                runner.print_histogram(&class_results);
                if let Err(e) = runner.write_class_json(&class_results, suffix_opt.as_deref()) { print_red(&format!("{}\n", e)); std::process::exit(3); }
            }
        }
        Commands::Exec { project: _, exec_cmd } => {
            // Placeholder (Phase 2+). For now, inform the user.
            print_red(&format!("exec not implemented yet: {}\n", exec_cmd));
            std::process::exit(64);
        }
        Commands::Clone { project, students, date, by_date } => {
            let list: Vec<String> = if let Some(list) = students { list.clone() } else { config.config.students.clone() };
            if list.is_empty() {
                print_red("No students provided and Config.students is empty\n");
                std::process::exit(2);
            }
            let g = git::Git::new(config.git.clone());
            let (suffix_opt, date_opt) = if *by_date {
                let runner = TestRunner::new(&config.test, false, false, false, project.clone());
                let d = match dates::Dates::from_tests_path(&runner.tests_path, project) {
                    Ok(d) => d,
                    Err(e) => { print_red(&format!("{}\n", e)); std::process::exit(2); }
                };
                match d.select() { Some(sel) => (Some(sel.suffix.clone()), Some(sel.date.clone())), None => (None, None) }
            } else { (None, date.clone()) };

            let mut repos = vec![];
            for s in list { repos.push(testcases::Repo::student(project.clone(), s, None, suffix_opt.clone())); }
            let longest = repos.iter().map(|r| r.display_label.len()).max().unwrap_or(0) + 1;
            for r in repos.iter() {
                util::print_justified(&r.display_label, longest);
                g.clone_repo(project, r, date_opt.as_deref());
            }
        }
        Commands::Pull { project, students } => {
            let list: Vec<String> = if let Some(list) = students { list.clone() } else { config.config.students.clone() };
            if list.is_empty() {
                print_red("No students provided and Config.students is empty\n");
                std::process::exit(2);
            }
            let g = git::Git::new(config.git.clone());
            let mut repos = vec![];
            for s in list { repos.push(testcases::Repo::student(project.clone(), s, None, None)); }
            let longest = repos.iter().map(|r| r.display_label.len()).max().unwrap_or(0) + 1;
            for r in repos.iter() {
                util::print_justified(&r.display_label, longest);
                g.pull_repo(r);
                println!();
            }
        }
        Commands::Upload { project, file } => {
            if let Err(e) = canvas::upload_class(config.canvas.clone(), config.canvas_mapper.clone(), project, file.as_deref(), false) {
                print_red(&format!("{}\n", e));
                std::process::exit(64);
            }
        }
        Commands::Rollup { project, by_date: _ } => {
                let runner = TestRunner::new(&config.test, false, false, false, project.clone());
                let d = match dates::Dates::from_tests_path(&runner.tests_path, project) { Ok(d) => d, Err(e) => { print_red(&format!("{}\n", e)); std::process::exit(2); } };
                if let Err(e) = rollup::rollup(project, &d.items) { print_red(&format!("{}\n", e)); std::process::exit(64); }
        }
    }

    print_green("\nDone\n");
}

fn num_cpus() -> usize { std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) }
