use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "grade-rs", version, about = "Rust autograder (MVP)")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum Commands {
    Test {
        #[arg(short = 'p', long = "project")]
        project: Option<String>,
        #[arg(short = 'n', long = "test-name")]
        test_name: Option<String>,
        #[arg(short = 'v', long = "verbose")]
        verbose: bool,
        #[arg(long = "very-verbose")]
        very_verbose: bool,
        #[arg(long = "unified-diff", help = "When verbose, print full unified diffs on mismatch")]
        unified_diff: bool,
        #[arg(long = "quiet", help = "Suppress per-test-case pass/fail lines")]
        quiet: bool,
        #[arg(long = "no-color", help = "Disable ANSI color output")]
        no_color: bool,
    },
    Class {
        #[arg(short = 'p', long = "project")]
        project: Option<String>,
        #[arg(short = 'v', long = "verbose")]
        verbose: bool,
        #[arg(long = "very-verbose")]
        very_verbose: bool,
        #[arg(long = "unified-diff", help = "When verbose, print full unified diffs on mismatch")]
        unified_diff: bool,
        #[arg(short = 'g', long = "github-action")]
        github_action: bool,
        #[arg(short = 's', long = "students")]
        students: Option<Vec<String>>,
        #[arg(short = 'd', long = "by-date", help = "Select a date from dates.toml for suffix and checkout context")]
        by_date: bool,
        #[arg(short = 'j', long = "jobs", help = "Number of parallel jobs (default: CPUs)")]
        jobs: Option<usize>,
        #[arg(long = "quiet", help = "Suppress per-test-case pass/fail lines")]
        quiet: bool,
        #[arg(long = "no-color", help = "Disable ANSI color output")]
        no_color: bool,
    },
    Exec {
        #[arg(short = 'p', long = "project")]
        project: Option<String>,
        #[arg(short = 'e', long = "exec")]
        exec_cmd: String,
        #[arg(short = 's', long = "students")]
        students: Option<Vec<String>>,
    },
    Clone {
        #[arg(short = 'p', long = "project")]
        project: Option<String>,
        #[arg(short = 's', long = "students")]
        students: Option<Vec<String>>,
        #[arg(short = 'v', long = "verbose")]
        verbose: bool,
        #[arg(long = "date", help = "Checkout commit before this 'YYYY-MM-DD[ HH:MM:SS]' date")] 
        date: Option<String>,
        #[arg(short = 'd', long = "by-date", help = "Select a date from dates.toml for suffix and checkout context")] 
        by_date: bool,
    },
    Pull {
        #[arg(short = 'p', long = "project")]
        project: Option<String>,
        #[arg(short = 's', long = "students")]
        students: Option<Vec<String>>,
    },
    Upload {
        #[arg(short = 'p', long = "project")]
        project: Option<String>,
        #[arg(long = "file", help = "Path to class results JSON; defaults to <project>.json")]
        file: Option<String>,
        #[arg(short = 'v', long = "verbose")]
        verbose: bool,
    },
    Rollup {
        #[arg(short = 'p', long = "project")]
        project: Option<String>,
        #[arg(short = 'd', long = "by-date", help = "Use dates.toml to aggregate JSONs into a rollup")]
        by_date: bool,
    },
}

impl Cli {
    pub fn parse() -> Self { <Cli as Parser>::parse() }
}
