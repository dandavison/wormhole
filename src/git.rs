use std::path::{Path, PathBuf};
use std::process::Command;

/// Returns the common git directory shared by all worktrees.
/// Uses `git rev-parse --git-common-dir` which handles regular repos,
/// submodules, and worktrees correctly.
pub fn git_common_dir(repo_path: &Path) -> PathBuf {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .current_dir(repo_path)
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let path_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let path = PathBuf::from(&path_str);
            if path.is_absolute() {
                return path;
            } else {
                return repo_path.join(path);
            }
        }
    }
    repo_path.join(".git")
}

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
            "HEAD",
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
    git_common_dir(repo_path).join("wormhole/worktrees")
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_list() {
        let output = r#"worktree /Users/dan/src/temporal
HEAD abc123
branch refs/heads/main

worktree /Users/dan/src/temporal/.git/wormhole/worktrees/ACT-1234
HEAD def456
branch refs/heads/ACT-1234

worktree /Users/dan/src/temporal/.git/wormhole/worktrees/ACT-5678
HEAD 789abc
detached
"#;
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 3);
        assert_eq!(worktrees[0].path, PathBuf::from("/Users/dan/src/temporal"));
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert_eq!(
            worktrees[1].path,
            PathBuf::from("/Users/dan/src/temporal/.git/wormhole/worktrees/ACT-1234")
        );
        assert_eq!(worktrees[1].branch, Some("ACT-1234".to_string()));
        assert_eq!(
            worktrees[2].path,
            PathBuf::from("/Users/dan/src/temporal/.git/wormhole/worktrees/ACT-5678")
        );
        assert_eq!(worktrees[2].branch, None);
    }

    #[test]
    fn test_git_common_dir_submodule() {
        use std::fs;

        let temp = tempfile::tempdir().unwrap();
        let parent = temp.path().join("parent");
        let child_src = temp.path().join("child_src");

        // Create child repo
        fs::create_dir_all(&child_src).unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(&child_src)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(&child_src)
            .output()
            .unwrap();

        // Create parent repo with submodule
        fs::create_dir_all(&parent).unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(&parent)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(&parent)
            .output()
            .unwrap();
        Command::new("git")
            .args(["submodule", "add", child_src.to_str().unwrap(), "child"])
            .current_dir(&parent)
            .output()
            .unwrap();

        let submodule = parent.join("child");
        assert!(
            submodule.join(".git").is_file(),
            ".git should be a file in submodule"
        );

        // git_common_dir should return parent's modules dir
        let common = git_common_dir(&submodule);
        assert!(
            common.to_string_lossy().contains("modules/child"),
            "common dir should be in parent's modules: {:?}",
            common
        );

        // worktree_base_path should work
        let base = worktree_base_path(&submodule);
        assert!(
            base.to_string_lossy()
                .contains("modules/child/wormhole/worktrees"),
            "worktree base should be in parent's modules: {:?}",
            base
        );
    }
}
