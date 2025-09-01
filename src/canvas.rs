use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

use crate::config::{CanvasCfg, CanvasMapperCfg};
use crate::util::{expand_tilde, print_green, print_red, print_yellow};

pub struct CanvasMapper {
    map: HashMap<String, String>, // github -> login_id
}

impl CanvasMapper {
    pub fn from_cfg(cfg: &CanvasMapperCfg) -> anyhow::Result<Self> {
        let path = expand_tilde(&cfg.map_path);
        let mut map = HashMap::new();
        let gh_col = cfg.github_col_name.clone();
        let login_col = cfg.login_col_name.clone();
        let mut rdr = csv::Reader::from_path(&path)?;
        let headers = rdr.headers()?.clone();
        let gh_idx = headers.iter().position(|h| h == gh_col).ok_or_else(|| anyhow::anyhow!("GitHub column not found"))?;
        let login_idx = headers.iter().position(|h| h == login_col).ok_or_else(|| anyhow::anyhow!("Login column not found"))?;
        for result in rdr.records() {
            let record = result?;
            let github = record.get(gh_idx).unwrap_or("").trim().to_string();
            let login = record.get(login_idx).unwrap_or("").trim().to_string();
            if !github.is_empty() {
                map.insert(github, login);
            } else {
                print_yellow(&format!("No GitHub ID for login {}\n", login));
            }
        }
        Ok(CanvasMapper { map })
    }

    pub fn lookup(&self, github: &str) -> Option<String> { self.map.get(github).cloned() }
}

pub struct CanvasClient {
    cfg: CanvasCfg,
    client: reqwest::blocking::Client,
    _verbose: bool,
}

impl CanvasClient {
    pub fn new(cfg: CanvasCfg, verbose: bool) -> anyhow::Result<Self> {
        let client = reqwest::blocking::Client::builder().user_agent("autograder-rust/0.1").build()?;
        Ok(CanvasClient { cfg, client, _verbose: verbose })
    }

    fn base(&self) -> String {
        if self.cfg.host_name.contains("://") { self.cfg.host_name.clone() } else { format!("https://{}", self.cfg.host_name) }
    }

    fn auth_headers(&self) -> reqwest::header::HeaderMap {
        use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
        let mut headers = HeaderMap::new();
        let token = format!("Bearer {}", self.cfg.access_token);
        headers.insert(AUTHORIZATION, HeaderValue::from_str(&token).unwrap_or(HeaderValue::from_static("")));
        headers
    }

    fn url(&self, path: &str) -> String { format!("{}/{}", self.base(), path) }

    pub fn get_course_id(&self) -> anyhow::Result<i64> {
        let mut url = self.url("api/v1/courses?per_page=100");
        loop {
            let res = self.client.get(&url).headers(self.auth_headers()).send()?;
            if !res.status().is_success() { anyhow::bail!("courses GET failed: {}", res.status()); }
            let link_header = res.headers().get("Link").cloned();
            let courses: serde_json::Value = res.json()?;
            for c in courses.as_array().unwrap_or(&vec![]) {
                if c.get("name").and_then(|v| v.as_str()) == Some(self.cfg.course_name.as_str()) {
                    if let Some(id) = c.get("id").and_then(|v| v.as_i64()) { return Ok(id); }
                }
            }
            if let Some(next) = next_link_from_header(link_header.as_ref()) { url = format!("{}{}", self.base(), next); } else { break; }
        }
        anyhow::bail!("course not found: {}", self.cfg.course_name)
    }

    pub fn get_assignment_id(&self, course_id: i64, assignment_name: &str) -> anyhow::Result<i64> {
        let mut url = self.url(&format!("api/v1/courses/{}/assignments?per_page=50", course_id));
        loop {
            let res = self.client.get(&url).headers(self.auth_headers()).send()?;
            if !res.status().is_success() { anyhow::bail!("assignments GET failed: {}", res.status()); }
            let link_header = res.headers().get("Link").cloned();
            let assigns: serde_json::Value = res.json()?;
            for a in assigns.as_array().unwrap_or(&vec![]) {
                if a.get("name").and_then(|v| v.as_str()) == Some(assignment_name) {
                    if let Some(id) = a.get("id").and_then(|v| v.as_i64()) { return Ok(id); }
                }
            }
            if let Some(next) = next_link_from_header(link_header.as_ref()) { url = format!("{}{}", self.base(), next); } else { break; }
        }
        anyhow::bail!("assignment not found: {}", assignment_name)
    }

    pub fn get_enrollment(&self, course_id: i64) -> anyhow::Result<Vec<Enrollment>> {
        let url = self.url(&format!("api/v1/courses/{}/enrollments?per_page=50", course_id));
        let res = self.client.get(url).headers(self.auth_headers()).send()?;
        if !res.status().is_success() { anyhow::bail!("enrollments GET failed: {}", res.status()); }
        let list: Vec<Enrollment> = res.json()?;
        Ok(list)
    }

    pub fn get_submission_score(&self, course_id: i64, assignment_id: i64, user_id: i64) -> anyhow::Result<Option<f64>> {
        let url = self.url(&format!("api/v1/courses/{}/assignments/{}/submissions/{}", course_id, assignment_id, user_id));
        let res = self.client.get(url).headers(self.auth_headers()).send()?;
        if !res.status().is_success() { anyhow::bail!("submission GET failed: {}", res.status()); }
        let v: serde_json::Value = res.json()?;
        Ok(v.get("score").and_then(|s| s.as_f64()))
    }

    pub fn put_submission(&self, course_id: i64, assignment_id: i64, user_id: i64, score: i64, comment: &str) -> anyhow::Result<bool> {
        let url = self.url(&format!("api/v1/courses/{}/assignments/{}/submissions/{}", course_id, assignment_id, user_id));
        let params = [
            ("submission[posted_grade]", score.to_string()),
            ("comment[text_comment]", comment.to_string()),
        ];
        let res = self.client.put(url).headers(self.auth_headers()).form(&params).send()?;
        Ok(res.status().is_success())
    }
}

#[derive(Debug, Deserialize)]
pub struct Enrollment { user: EnrollmentUser, user_id: i64 }
#[derive(Debug, Deserialize)]
struct EnrollmentUser { login_id: String }

fn next_link_from_header(link: Option<&reqwest::header::HeaderValue>) -> Option<String> {
    let val = link?.to_str().ok()?;
    for part in val.split(',') {
        let part = part.trim();
        if part.contains("rel=next") {
            let start = part.find('<')? + 1;
            let end = part.find('>')?;
            return Some(part[start..end].to_string());
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct ClassResultItem {
    student: Option<String>,
    score: i64,
    comment: String,
}

pub fn upload_class(canvas: CanvasCfg, mapper_cfg: CanvasMapperCfg, project: &str, file: Option<&str>, verbose: bool) -> anyhow::Result<()> {
    let json_path = file.map(|s| s.to_string()).unwrap_or_else(|| format!("{}.json", project));
    let data = fs::read_to_string(&json_path).map_err(|e| anyhow::anyhow!("{} does not exist. Run \"grade-rs class -p {}\" first ({})", json_path, project, e))?;
    let items: Vec<ClassResultItem> = serde_json::from_str(&data)?;
    if verbose { println!("Uploading from {} ({} results)", json_path, items.len()); }

    let mapper = CanvasMapper::from_cfg(&mapper_cfg)?;
    let client = CanvasClient::new(canvas, verbose)?;
    let course_id = client.get_course_id()?;
    let assignment_id = client.get_assignment_id(course_id, project)?;
    if verbose { println!("Course ID: {}, Assignment ID: {}", course_id, assignment_id); }
    let enrollment = client.get_enrollment(course_id)?;

    // Map login_id -> user_id
    let mut id_map: HashMap<String, i64> = HashMap::new();
    for e in enrollment { id_map.insert(e.user.login_id, e.user_id); }

    for it in items {
        let Some(student) = it.student else { continue };
        let Some(login_id) = mapper.lookup(&student) else { print_red(&format!("no mapping for {}\n", student)); continue; };
        let Some(user_id) = id_map.get(&login_id).copied() else { print_red(&format!("{} not enrolled\n", login_id)); continue; };
        if verbose { println!("Map: {} -> {} (user_id {})", student, login_id, user_id); }
        print!("Uploading {} {} ", login_id, it.score);
        // Skip if same as Canvas
        if let Ok(Some(cur)) = client.get_submission_score(course_id, assignment_id, user_id) {
            if (cur - (it.score as f64)).abs() < f64::EPSILON { println!("skipping: new score == score in Canvas"); continue; }
            if verbose { println!("(current Canvas score: {})", cur); }
        }
        match client.put_submission(course_id, assignment_id, user_id, it.score, &it.comment) {
            Ok(true) => print_green("ok\n"),
            _ => print_red("failed\n"),
        }
    }

    Ok(())
}
