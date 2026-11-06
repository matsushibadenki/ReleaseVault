use crate::vault::R;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: u64,
    pub name: String,
    pub branch: String,
    pub sha: String,
    pub event: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub updated: String,
    pub attempt: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Report {
    pub repo: String,
    pub checked: String,
    pub runs: Vec<Run>,
}
pub fn parse(repo: &str, data: Value) -> R<Report> {
    crate::integrations::dest(repo, "")?;
    let rows = data["workflow_runs"].as_array().ok_or("app-api")?;
    if rows.len() > 20 {
        return Err("app-api".into());
    }
    let mut runs = Vec::new();
    for row in rows {
        let text = |field: &str| -> R<String> {
            let text = row[field].as_str().unwrap_or("");
            if text.len() > 1024 {
                return Err("app-api".into());
            }
            Ok(text.into())
        };
        let id = row["id"].as_u64().filter(|id| *id > 0).ok_or("app-api")?;
        if !row["repository"]["full_name"]
            .as_str()
            .is_some_and(|name| name.eq_ignore_ascii_case(repo))
        {
            return Err("app-api".into());
        }
        runs.push(Run {
            id,
            name: text("name")?,
            branch: text("head_branch")?,
            sha: text("head_sha")?,
            event: text("event")?,
            status: text("status")?,
            conclusion: row["conclusion"].as_str().map(str::to_owned),
            updated: text("updated_at")?,
            attempt: row["run_attempt"].as_u64().unwrap_or(1),
        });
    }
    Ok(Report {
        repo: repo.into(),
        checked: chrono::Utc::now().to_rfc3339(),
        runs,
    })
}
