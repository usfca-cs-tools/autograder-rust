use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::dates::DateItem;
// no extra imports

#[derive(Debug, Deserialize, Clone)]
struct RepoScore {
    student: Option<String>,
    score: i64,
    comment: String,
}

#[derive(Debug, Serialize, Clone)]
struct RolledItem {
    student: String,
    score: f64,
    comment: String,
}

pub fn rollup(project: &str, dates: &[DateItem]) -> anyhow::Result<()> {
    // by_student[student][suffix] = RepoScore
    let mut by_student: BTreeMap<String, BTreeMap<String, RepoScore>> = BTreeMap::new();

    for d in dates {
        let fname = format!("{}-{}.json", project, d.suffix);
        let path = PathBuf::from(fname);
        if !path.exists() { continue; }
        let data = fs::read_to_string(&path)?;
        let list: Vec<RepoScore> = serde_json::from_str(&data)?;
        for r in list.into_iter() {
            let student = r.student.clone().unwrap_or_default();
            if student.is_empty() { continue; }
            by_student.entry(student).or_default().insert(d.suffix.clone(), r);
        }
    }

    let mut out: Vec<RolledItem> = vec![];
    for (student, map) in by_student.iter() {
        let mut rolled_score: f64 = 0.0;
        let mut prev_score: i64 = 0;
        let mut rolled_comment = String::new();
        for d in dates {
            if let Some(r) = map.get(&d.suffix) {
                let score = r.score;
                let comment_line = format!("{}: {} + ({} - {}) * {} = ", d.suffix, rolled_score, score, rolled_score, d.percentage);
                if score != prev_score {
                    rolled_score += (score as f64 - rolled_score) * d.percentage;
                }
                let final_line = format!("{}\n", rolled_score);
                println!("{}: {}{}", student, comment_line, rolled_score);
                rolled_comment.push_str(&r.comment);
                rolled_comment.push_str("\n\n");
                rolled_comment.push_str(&comment_line);
                rolled_comment.push_str(&final_line);
                rolled_comment.push_str("\n");
                prev_score = score;
            }
        }
        out.push(RolledItem { student: student.clone(), score: rolled_score, comment: rolled_comment });
    }

    let outpath = format!("{}-rollup.json", project);
    fs::write(&outpath, serde_json::to_string_pretty(&out)?)?;
    Ok(())
}
