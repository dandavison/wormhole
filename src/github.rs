use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[derive(Clone, Debug, Deserialize, serde::Serialize)]
pub struct PrStatus {
    pub number: u64,
    pub state: String,
    #[serde(rename = "isDraft")]
    pub is_draft: bool,
    pub url: String,
}

impl PrStatus {
    pub fn display(&self) -> String {
        let state = if self.is_draft {
            "draft"
        } else {
            match self.state.as_str() {
                "OPEN" => "open",
                "MERGED" => "merged",
                "CLOSED" => "closed",
                _ => &self.state,
            }
        };
        format!("#{} ({})", self.number, state)
    }
}

pub fn get_pr_status(project_path: &Path) -> Option<PrStatus> {
    let output = Command::new("gh")
        .args(["pr", "view", "--json", "number,state,isDraft,url"])
        .current_dir(project_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    serde_json::from_slice(&output.stdout).ok()
}

pub fn get_pr_for_branch(repo: &str, branch: &str) -> Option<PrStatus> {
    let output = Command::new("gh")
        .args([
            "pr",
            "list",
            "--repo",
            repo,
            "--head",
            branch,
            "--json",
            "number,state,isDraft,url",
            "--limit",
            "1",
        ])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let prs: Vec<PrStatus> = serde_json::from_slice(&output.stdout).ok()?;
    prs.into_iter().next()
}
