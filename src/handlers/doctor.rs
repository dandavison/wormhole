use hyper::{Body, Response, StatusCode};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{config, git, task};

#[derive(Serialize, Deserialize)]
pub struct ConformResult {
    pub results: Vec<ConformTaskResult>,
}

#[derive(Serialize, Deserialize)]
pub struct ConformTaskResult {
    pub task: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn conform() -> Response<Body> {
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
                    match task::setup_task_worktree(&wt.path, name, branch) {
                        Ok(()) => Some(ConformTaskResult {
                            task: task_key,
                            error: None,
                        }),
                        Err(e) => Some(ConformTaskResult {
                            task: task_key,
                            error: Some(e),
                        }),
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let result = ConformResult { results };
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
