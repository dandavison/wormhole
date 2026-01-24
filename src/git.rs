use std::path::{Path, PathBuf};
use std::process::Command;

pub fn is_git_repo(path: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub struct Worktree {
    pub path: PathBuf,
    #[allow(dead_code)]
    pub branch: Option<String>,
}

pub fn list_worktrees(repo_path: &Path) -> Vec<Worktree> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_path)
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return vec![],
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_worktree_list(&stdout)
}

fn parse_worktree_list(output: &str) -> Vec<Worktree> {
    let mut worktrees = vec![];
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;

    for line in output.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(path) = current_path.take() {
                worktrees.push(Worktree {
                    path,
                    branch: current_branch.take(),
                });
            }
            current_path = Some(PathBuf::from(path));
            current_branch = None;
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            current_branch = Some(branch.to_string());
        }
    }

    if let Some(path) = current_path {
        worktrees.push(Worktree {
            path,
            branch: current_branch,
        });
    }

    worktrees
}

pub fn create_worktree(
    repo_path: &Path,
    worktree_path: &Path,
    branch_name: &str,
) -> Result<(), String> {
    if let Some(parent) = worktree_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            branch_name,
            worktree_path.to_str().unwrap(),
            "origin/main",
        ])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git worktree: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("git worktree add failed: {}", stderr.trim()))
    }
}

pub fn worktree_base_path(repo_path: &Path) -> PathBuf {
    repo_path.join(".tmp").join("worktrees")
}

pub fn remove_worktree(repo_path: &Path, worktree_path: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .args(["worktree", "remove", worktree_path.to_str().unwrap()])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git worktree remove: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("git worktree remove failed: {}", stderr.trim()))
    }
}

pub fn delete_branch(repo_path: &Path, branch_name: &str) -> Result<(), String> {
    let output = Command::new("git")
        .args(["branch", "-d", branch_name])
        .current_dir(repo_path)
        .output()
        .map_err(|e| format!("Failed to run git branch -d: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("git branch -d failed: {}", stderr.trim()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_list() {
        let output = r#"worktree /Users/dan/src/temporal
HEAD abc123
branch refs/heads/main

worktree /Users/dan/src/temporal/.tmp/worktrees/ACT-1234
HEAD def456
branch refs/heads/ACT-1234

worktree /Users/dan/src/temporal/.tmp/worktrees/ACT-5678
HEAD 789abc
detached
"#;
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 3);
        assert_eq!(
            worktrees[0].path,
            PathBuf::from("/Users/dan/src/temporal")
        );
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert_eq!(
            worktrees[1].path,
            PathBuf::from("/Users/dan/src/temporal/.tmp/worktrees/ACT-1234")
        );
        assert_eq!(worktrees[1].branch, Some("ACT-1234".to_string()));
        assert_eq!(
            worktrees[2].path,
            PathBuf::from("/Users/dan/src/temporal/.tmp/worktrees/ACT-5678")
        );
        assert_eq!(worktrees[2].branch, None);
    }
}
