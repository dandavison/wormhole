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

/// Returns true if the path is a git worktree (not a main repo or submodule).
/// Worktrees have .git as a file pointing to a .git/worktrees/ directory.
/// Submodules also have .git as a file, but point to .git/modules/.
pub fn is_worktree(path: &Path) -> bool {
    let git_path = path.join(".git");
    if !git_path.is_file() {
        return false;
    }
    // Read the .git file to check if it points to a worktrees directory
    if let Ok(content) = std::fs::read_to_string(&git_path) {
        // Format: "gitdir: /path/to/repo/.git/worktrees/branch-name"
        if let Some(gitdir) = content.strip_prefix("gitdir:") {
            return gitdir.trim().contains("/worktrees/");
        }
    }
    false
}

pub fn github_repo_from_remote(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    parse_github_repo(&url)
}

fn parse_github_repo(url: &str) -> Option<String> {
    let rest = if let Some(r) = url.strip_prefix("git@github.com:") {
        r
    } else if let Some(r) = url.strip_prefix("https://github.com/") {
        r
    } else if url.contains("@github.com:") {
        // Handle org-*@github.com:owner/repo format (GitHub App SSH URLs)
        url.split("@github.com:").nth(1)?
    } else {
        return None;
    };
    Some(rest.strip_suffix(".git").unwrap_or(rest).to_string())
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

    let args = if branch_exists(repo_path, branch_name) {
        if let Some(existing_path) = branch_checked_out_at(repo_path, branch_name) {
            return Err(format!(
                "Branch '{}' is already checked out at {}. \
                 Switch to a different branch there first, or use that location directly.",
                branch_name,
                existing_path.display()
            ));
        }
        vec![
            "worktree",
            "add",
            worktree_path.to_str().unwrap(),
            branch_name,
        ]
    } else {
        vec![
            "worktree",
            "add",
            "-b",
            branch_name,
            worktree_path.to_str().unwrap(),
            "HEAD",
        ]
    };

    let output = Command::new("git")
        .args(&args)
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

fn branch_exists(repo_path: &Path, branch_name: &str) -> bool {
    Command::new("git")
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{}", branch_name),
        ])
        .current_dir(repo_path)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn branch_checked_out_at(repo_path: &Path, branch_name: &str) -> Option<PathBuf> {
    for wt in list_worktrees(repo_path) {
        if wt.branch.as_deref() == Some(branch_name) {
            return Some(wt.path);
        }
    }
    None
}

pub fn worktree_base_path(repo_path: &Path) -> PathBuf {
    git_common_dir(repo_path).join("wormhole/worktrees")
}

pub fn encode_branch_for_path(branch: &str) -> String {
    url::form_urlencoded::byte_serialize(branch.as_bytes()).collect()
}

pub fn list_branches(repo_path: &Path) -> Vec<String> {
    let output = Command::new("git")
        .args(["for-each-ref", "--format=%(refname:short)", "refs/heads/"])
        .current_dir(repo_path)
        .output();

    match output {
        Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect(),
        _ => vec![],
    }
}

/// Migrate worktrees from old layout (`worktrees/$branch`) to new layout
/// (`worktrees/$branch/$repo_name`). Returns the number of worktrees migrated.
pub fn migrate_worktrees(repo_name: &str, repo_path: &Path) -> Result<usize, String> {
    let base = worktree_base_path(repo_path);
    if !base.exists() {
        return Ok(0);
    }
    let mut new_paths: Vec<PathBuf> = Vec::new();
    let entries: Vec<_> = std::fs::read_dir(&base)
        .map_err(|e| format!("Failed to read {}: {}", base.display(), e))?
        .filter_map(|e| e.ok())
        .collect();
    for entry in entries {
        let old = entry.path();
        // Old layout: $base/$encoded_branch/.git exists (it's a worktree)
        if !old.join(".git").is_file() {
            continue;
        }
        let tmp = old.with_extension("wh-migrate-tmp");
        std::fs::rename(&old, &tmp)
            .map_err(|e| format!("Failed to rename {}: {}", old.display(), e))?;
        let new = old.join(repo_name);
        std::fs::create_dir_all(&old)
            .map_err(|e| format!("Failed to create dir {}: {}", old.display(), e))?;
        std::fs::rename(&tmp, &new).map_err(|e| {
            format!(
                "Failed to rename {} -> {}: {}",
                tmp.display(),
                new.display(),
                e
            )
        })?;
        new_paths.push(new);
    }
    if !new_paths.is_empty() {
        let mut args: Vec<&str> = vec!["worktree", "repair"];
        let path_strs: Vec<String> = new_paths.iter().map(|p| p.display().to_string()).collect();
        args.extend(path_strs.iter().map(|s| s.as_str()));
        let output = Command::new("git")
            .args(&args)
            .current_dir(repo_path)
            .output()
            .map_err(|e| format!("git worktree repair failed: {}", e))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git worktree repair failed: {}", stderr.trim()));
        }
    }
    Ok(new_paths.len())
}

pub fn remove_worktree(repo_path: &Path, worktree_path: &Path) -> Result<(), String> {
    let output = Command::new("git")
        .args([
            "worktree",
            "remove",
            "--force",
            worktree_path.to_str().unwrap(),
        ])
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

pub fn github_file_url(repo_path: &Path, file_path: &str) -> Option<String> {
    let remote = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(repo_path)
        .output()
        .ok()?;
    if !remote.status.success() {
        return None;
    }
    let remote_url = String::from_utf8_lossy(&remote.stdout).trim().to_string();

    let branch = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()?;
    if !branch.status.success() {
        return None;
    }
    let branch_name = String::from_utf8_lossy(&branch.stdout).trim().to_string();

    // Convert git URL to GitHub blob URL
    // Handles: git@github.com:owner/repo.git, https://github.com/owner/repo.git
    let github_base = if remote_url.starts_with("git@github.com:") {
        remote_url
            .strip_prefix("git@github.com:")?
            .strip_suffix(".git")
            .or(Some(remote_url.strip_prefix("git@github.com:")?))?
            .to_string()
    } else if remote_url.starts_with("https://github.com/") {
        remote_url
            .strip_prefix("https://github.com/")?
            .strip_suffix(".git")
            .or(Some(remote_url.strip_prefix("https://github.com/")?))?
            .to_string()
    } else {
        return None;
    };

    Some(format!(
        "https://github.com/{}/blob/{}/{}",
        github_base, branch_name, file_path
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_worktree_list() {
        let output = r#"worktree /Users/dan/src/temporal
HEAD abc123
branch refs/heads/main

worktree /Users/dan/src/temporal/.git/wormhole/worktrees/ACT-1234/temporal
HEAD def456
branch refs/heads/ACT-1234

worktree /Users/dan/src/temporal/.git/wormhole/worktrees/ACT-5678/temporal
HEAD 789abc
detached
"#;
        let worktrees = parse_worktree_list(output);
        assert_eq!(worktrees.len(), 3);
        assert_eq!(worktrees[0].path, PathBuf::from("/Users/dan/src/temporal"));
        assert_eq!(worktrees[0].branch, Some("main".to_string()));
        assert_eq!(
            worktrees[1].path,
            PathBuf::from("/Users/dan/src/temporal/.git/wormhole/worktrees/ACT-1234/temporal")
        );
        assert_eq!(worktrees[1].branch, Some("ACT-1234".to_string()));
        assert_eq!(
            worktrees[2].path,
            PathBuf::from("/Users/dan/src/temporal/.git/wormhole/worktrees/ACT-5678/temporal")
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

    #[test]
    fn test_create_worktree_existing_branch() {
        use std::fs;

        let temp = tempfile::tempdir().unwrap();
        let repo = temp.path().join("repo");

        fs::create_dir_all(&repo).unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(&repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(&repo)
            .output()
            .unwrap();

        // Create branch without worktree
        Command::new("git")
            .args(["branch", "ACT-123"])
            .current_dir(&repo)
            .output()
            .unwrap();

        assert!(branch_exists(&repo, "ACT-123"));

        let worktree_path = repo.join("worktrees/ACT-123");
        let result = create_worktree(&repo, &worktree_path, "ACT-123");
        assert!(result.is_ok(), "create_worktree failed: {:?}", result);
        assert!(worktree_path.exists());
    }

    #[test]
    fn test_encode_branch_for_path() {
        // Simple branch names pass through unchanged
        assert_eq!(encode_branch_for_path("main"), "main");
        assert_eq!(encode_branch_for_path("ACT-123"), "ACT-123");

        // Branch names with / are URL-encoded to stay flat
        assert_eq!(encode_branch_for_path("feature/auth"), "feature%2Fauth");
        assert_eq!(
            encode_branch_for_path("feature/nested/deep"),
            "feature%2Fnested%2Fdeep"
        );

        // Other special characters that might appear in branch names
        assert_eq!(encode_branch_for_path("fix-bug#123"), "fix-bug%23123");
    }
}
