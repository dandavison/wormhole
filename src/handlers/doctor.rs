use hyper::{Body, Response, StatusCode};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::pst::TerminalHyperlink;
use crate::{config, git, task};

#[derive(Serialize, Deserialize)]
pub struct ConformResult {
    pub dry_run: bool,
    pub results: Vec<ConformTaskResult>,
}

#[derive(Serialize, Deserialize)]
pub struct ConformTaskResult {
    pub task: String,
    pub actions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl ConformResult {
    pub fn render_terminal(&self) -> String {
        let mut lines = Vec::new();
        let mut ok = 0;
        let mut errs = 0;
        for r in &self.results {
            if let Some(ref e) = r.error {
                lines.push(format!("  {} error: {}", r.task, e));
                errs += 1;
            } else {
                let key = crate::project::ProjectKey::parse(&r.task);
                lines.push(format!("  {}", key.hyperlink()));
                for action in &r.actions {
                    lines.push(format!("    {}", action));
                }
                ok += 1;
            }
        }
        let verb = if self.dry_run {
            "Would conform"
        } else {
            "Conformed"
        };
        lines.push(format!("{} {} task(s), {} error(s).", verb, ok, errs));
        lines.join("\n")
    }
}

pub fn conform(dry_run: bool) -> Response<Body> {
    let available = config::available_projects();
    let repo_paths: Vec<_> = available.into_iter().collect();

    let results: Vec<ConformTaskResult> = repo_paths
        .par_iter()
        .filter(|(_, path)| git::is_git_repo(path))
        .flat_map(|(name, path)| {
            let worktree_base = git::worktree_base_path(path);
            git::list_worktrees(path)
                .into_iter()
                .filter(|wt| wt.path.starts_with(&worktree_base))
                .filter_map(|wt| {
                    let branch = wt.branch.as_deref()?;
                    let task_key = format!("{}:{}", name, branch);
                    match task::conform_task_worktree(&wt.path, name, branch, dry_run) {
                        Ok(actions) => Some(ConformTaskResult {
                            task: task_key,
                            actions,
                            error: None,
                        }),
                        Err(e) => Some(ConformTaskResult {
                            task: task_key,
                            actions: vec![],
                            error: Some(e),
                        }),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let result = ConformResult { dry_run, results };
    match serde_json::to_string(&result) {
        Ok(json) => Response::builder()
            .header("Content-Type", "application/json")
            .body(Body::from(json))
            .unwrap(),
        Err(e) => Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from(format!("Failed to serialize: {}", e)))
            .unwrap(),
    }
}
