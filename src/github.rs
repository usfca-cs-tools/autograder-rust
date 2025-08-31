use serde::Deserialize;

use crate::config::GithubCfg;
use crate::testcases::RepoResult;
use crate::util::{print_yellow};

#[derive(Debug, Deserialize)]
struct ArtifactsList { artifacts: Vec<Artifact> }

#[derive(Debug, Deserialize, Clone)]
struct WorkflowRun { id: u64 }

#[derive(Debug, Deserialize, Clone)]
struct Artifact {
    #[allow(dead_code)]
    id: u64,
    archive_download_url: String,
    workflow_run: WorkflowRun,
}

#[derive(Debug, Deserialize)]
struct JobsList { jobs: Vec<Job> }

#[derive(Debug, Deserialize)]
struct Job { id: u64 }

pub struct Github {
    cfg: GithubCfg,
    org: String,
    project: String,
    _verbose: bool,
    client: reqwest::blocking::Client,
}

impl Github {
    pub fn new(cfg: GithubCfg, org: String, project: String, verbose: bool) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::builder()
            .user_agent("autograder-rust/0.1")
            .build()?;
        Ok(Github { cfg, org, project, _verbose: verbose, client })
    }

    fn base(&self) -> String {
        if self.cfg.host_name.contains("://") { self.cfg.host_name.clone() } else { format!("https://{}", self.cfg.host_name) }
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
        let token = format!("Bearer {}", self.cfg.access_token);
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&token).unwrap_or(HeaderValue::from_static("")));
        headers
    }

    fn make_artifacts_url(&self, student: &str) -> String {
        format!("{}/repos/{}/{}-{}/actions/artifacts", self.base(), self.org, self.project, student)
    }

    fn make_runs_url(&self, student: &str) -> String {
        format!("{}/repos/{}/{}-{}/actions/runs", self.base(), self.org, self.project, student)
    }

    fn get_first_artifact(&self, student: &str) -> anyhow::Result<Option<Artifact>> {
        let url = self.make_artifacts_url(student);
        let res = self.client.get(url).headers(self.headers()).send()?;
        if !res.status().is_success() {
            print_yellow(&format!("Accessing artifacts for {} returned {}\n", student, res.status()));
            return Ok(None);
        }
        let list: ArtifactsList = res.json()?;
        Ok(list.artifacts.into_iter().next())
    }

    fn get_action_run_summary_url(&self, student: &str, run_id: u64) -> anyhow::Result<String> {
        let jobs_url = format!("{}/{}/jobs", self.make_runs_url(student), run_id);
        let res = self.client.get(jobs_url).headers(self.headers()).send()?;
        if !res.status().is_success() {
            print_yellow(&format!("No jobs found for {} run {}\n", student, run_id));
            return Ok(format!("https://github.com/{}/{}-{}/actions/runs/{}", self.org, self.project, student, run_id));
        }
        let jobs: JobsList = res.json()?;
        let job_id = jobs.jobs.get(0).map(|j| j.id).unwrap_or(0);
        Ok(format!("https://github.com/{}/{}-{}/actions/runs/{}#summary-{}", self.org, self.project, student, run_id, job_id))
    }

    fn download_artifact_grade(&self, artifact: &Artifact) -> anyhow::Result<f64> {
        let res = self.client.get(&artifact.archive_download_url).headers(self.headers()).send()?;
        if !res.status().is_success() { anyhow::bail!("Failed to download artifact"); }
        let bytes = res.bytes()?;
        let reader = std::io::Cursor::new(bytes);
        let mut zip = zip::ZipArchive::new(reader)?;
        let mut file = zip.by_name("grade-results.json")?;
        let mut s = String::new();
        use std::io::Read;
        file.read_to_string(&mut s)?;
        let v: serde_json::Value = serde_json::from_str(&s)?;
        let grade = v.get("grade").and_then(|g| g.as_f64()).unwrap_or(0.0);
        Ok(grade)
    }

    pub fn get_action_results(&self, student: &str) -> RepoResult {
        // Minimal RepoResult: score + comment link
        let mut rr = RepoResult { comment: String::new(), results: vec![], score: 0, student: Some(student.to_string()), build_err: None };
        match self.get_first_artifact(student) {
            Ok(Some(artifact)) => {
                let run_id = artifact.workflow_run.id;
                let grade = self.download_artifact_grade(&artifact).unwrap_or_else(|e| { print_yellow(&format!("Analyzing artifact: {}\n", e)); 0.0 });
                let link = self.get_action_run_summary_url(student, run_id).unwrap_or_else(|_| String::from(""));
                rr.score = grade.round() as i64; // cast to i64 to fit current schema
                rr.comment = link;
            }
            Ok(None) => {
                rr.comment = "No artifacts found".into();
                print_yellow("No artifacts found\n");
            }
            Err(e) => {
                rr.comment = format!("GitHub error: {}", e);
                print_yellow(&format!("get_action_results: {}\n", e));
            }
        }
        rr
    }
}
