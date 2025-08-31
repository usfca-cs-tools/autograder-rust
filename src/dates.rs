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
        Some(&self.items[idx - 1])
    }
}
