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

/// Find directories under the worktree base that are not recognized by git as worktrees.
pub fn find_orphan_worktree_dirs(repo_path: &Path) -> Vec<PathBuf> {
    let base = worktree_base_path(repo_path);
    if !base.exists() {
        return vec![];
    }
    let known: std::collections::HashSet<PathBuf> = list_worktrees(repo_path)
        .into_iter()
        .filter_map(|wt| wt.path.canonicalize().ok())
        .collect();

    let mut orphans = vec![];
    let branch_dirs = match std::fs::read_dir(&base) {
        Ok(d) => d,
        Err(_) => return vec![],
    };
    for branch_entry in branch_dirs.flatten() {
        let branch_path = branch_entry.path();
        if !branch_path.is_dir() {
            continue;
        }
        let repo_dirs = match std::fs::read_dir(&branch_path) {
            Ok(d) => d,
            Err(_) => continue,
        };
        for repo_entry in repo_dirs.flatten() {
            let path = repo_entry.path();
            if !path.is_dir() || !path.join(".git").is_file() {
                continue;
            }
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !known.contains(&canonical) {
                orphans.push(path);
            }
        }
    }
    orphans
}

/// Encode a branch name for use as a single flat path component.
/// Replaces `/` with `--`. This means wormhole cannot distinguish
/// branches `foo/bar` and `foo--bar`; we accept this trade-off.
pub fn encode_branch_for_path(branch: &str) -> String {
    branch.replace('/', "--")
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

/// Migrate worktrees from old layouts to current layout.
///
/// Handles legacy layouts:
/// 1. Ancient flat: `worktrees/$branch/.git` → `worktrees/$encoded/$repo/.git`
/// 2. Percent-encoded: `worktrees/feat%2Fbar/` → `worktrees/feat--bar/`
/// 3. Nested (from short-lived intermediate layout): `worktrees/feat/bar/` → `worktrees/feat--bar/`
pub fn migrate_worktrees(repo_name: &str, repo_path: &Path) -> Result<usize, String> {
    let base = worktree_base_path(repo_path);
    if !base.exists() {
        return Ok(0);
    }
    let mut new_paths: Vec<PathBuf> = Vec::new();

    // Pass 1: ancient flat layout ($base/$branch/.git → $base/$branch/$repo/.git)
    let entries: Vec<_> = std::fs::read_dir(&base)
        .map_err(|e| format!("Failed to read {}: {}", base.display(), e))?
        .filter_map(|e| e.ok())
        .collect();
    for entry in entries {
        let old = entry.path();
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

    // Pass 2: percent-encoded dirs (feat%2Fbar/ → feat--bar/)
    new_paths.extend(migrate_encoded_worktree_dirs(&base, repo_name)?);

    // Pass 3: nested dirs from intermediate layout (feat/bar/$repo → feat--bar/$repo)
    new_paths.extend(migrate_nested_worktree_dirs(&base, repo_name, repo_path)?);

    if !new_paths.is_empty() {
        repair_worktrees(repo_path, &new_paths)?;
    }
    Ok(new_paths.len())
}

/// Rename worktree branch dirs that use %2F encoding to -- encoding.
fn migrate_encoded_worktree_dirs(base: &Path, repo_name: &str) -> Result<Vec<PathBuf>, String> {
    let mut moved = vec![];
    let entries: Vec<_> = std::fs::read_dir(base)
        .map_err(|e| format!("Failed to read {}: {}", base.display(), e))?
        .filter_map(|e| e.ok())
        .collect();
    for entry in entries {
        let old = entry.path();
        let name = match old.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !name.contains("%2F") && !name.contains("%2f") {
            continue;
        }
        if !old.is_dir() {
            continue;
        }
        let new_name = name.replace("%2F", "--").replace("%2f", "--");
        let new_dir = base.join(&new_name);
        if new_dir == old {
            continue;
        }
        std::fs::rename(&old, &new_dir).map_err(|e| {
            format!(
                "Failed to rename {} -> {}: {}",
                old.display(),
                new_dir.display(),
                e
            )
        })?;
        let repo_wt = new_dir.join(repo_name);
        if repo_wt.is_dir() && repo_wt.join(".git").is_file() {
            moved.push(repo_wt);
        }
    }
    Ok(moved)
}

/// Migrate worktrees from nested layout (branch `/` created subdirs) to flat `--` layout.
/// E.g. `worktrees/user/topic/repo` → `worktrees/user--topic/repo`
fn migrate_nested_worktree_dirs(
    base: &Path,
    repo_name: &str,
    repo_path: &Path,
) -> Result<Vec<PathBuf>, String> {
    let mut moved = vec![];
    for wt in list_worktrees(repo_path) {
        if !wt.path.starts_with(base) {
            continue;
        }
        let branch = match &wt.branch {
            Some(b) if b.contains('/') => b,
            _ => continue,
        };
        let expected = base.join(encode_branch_for_path(branch)).join(repo_name);
        if wt.path == expected {
            continue;
        }
        let new_branch_dir = base.join(encode_branch_for_path(branch));
        if new_branch_dir.exists() {
            continue;
        }
        // The nested layout has branch components as subdirs: base/user/topic/repo.
        // The top-level dir to rename is base/user (first component after base).
        let relative = match wt.path.strip_prefix(base) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let top_component = match relative.components().next() {
            Some(c) => c.as_os_str(),
            None => continue,
        };
        let old_top = base.join(top_component);

        // Move contents: create new flat dir, move repo worktree into it
        std::fs::create_dir_all(&new_branch_dir)
            .map_err(|e| format!("Failed to create {}: {}", new_branch_dir.display(), e))?;
        let old_repo = wt.path.clone();
        let new_repo = new_branch_dir.join(repo_name);
        std::fs::rename(&old_repo, &new_repo).map_err(|e| {
            format!(
                "Failed to rename {} -> {}: {}",
                old_repo.display(),
                new_repo.display(),
                e
            )
        })?;
        // Clean up the old nested directory tree
        let _ = std::fs::remove_dir_all(&old_top);
        moved.push(new_repo);
    }
    Ok(moved)
}

fn repair_worktrees(repo_path: &Path, paths: &[PathBuf]) -> Result<(), String> {
    let mut args: Vec<&str> = vec!["worktree", "repair"];
    let path_strs: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
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
    Ok(())
}

/// Migrate files in `dir` from legacy naming (%2F or nested subdirs) to flat `--` naming.
pub fn migrate_legacy_files(dir: &Path) -> Result<usize, String> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0;

    // Rename %2F-encoded files
    let entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read {}: {}", dir.display(), e))?
        .filter_map(|e| e.ok())
        .collect();
    for entry in entries {
        let old = entry.path();
        let name = match old.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if !name.contains("%2F") && !name.contains("%2f") {
            continue;
        }
        let new_name = name.replace("%2F", "--").replace("%2f", "--");
        let new = dir.join(&new_name);
        if new != old {
            std::fs::rename(&old, &new).map_err(|e| {
                format!(
                    "Failed to rename {} -> {}: {}",
                    old.display(),
                    new.display(),
                    e
                )
            })?;
            count += 1;
        }
    }

    // Flatten nested subdirectories: move files from subdirs up with -- separator
    let entries: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("Failed to read {}: {}", dir.display(), e))?
        .filter_map(|e| e.ok())
        .collect();
    for entry in entries {
        let subdir = entry.path();
        if !subdir.is_dir() {
            continue;
        }
        let prefix = match subdir.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        let sub_entries: Vec<_> = match std::fs::read_dir(&subdir) {
            Ok(d) => d.filter_map(|e| e.ok()).collect(),
            Err(_) => continue,
        };
        for sub_entry in sub_entries {
            let old = sub_entry.path();
            if old.is_dir() {
                continue;
            }
            let file_name = match old.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let new_name = format!("{}--{}", prefix, file_name);
            let new = dir.join(&new_name);
            std::fs::rename(&old, &new).map_err(|e| {
                format!(
                    "Failed to rename {} -> {}: {}",
                    old.display(),
                    new.display(),
                    e
                )
            })?;
            count += 1;
        }
        let _ = std::fs::remove_dir(&subdir);
    }

    Ok(count)
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
            .args([
                "-c",
                "protocol.file.allow=always",
                "submodule",
                "add",
                child_src.to_str().unwrap(),
                "child",
            ])
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
    fn test_find_orphan_worktree_dirs() {
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

        let base = worktree_base_path(&repo);

        // Create a real worktree
        Command::new("git")
            .args(["branch", "real-branch"])
            .current_dir(&repo)
            .output()
            .unwrap();
        let real_wt = base.join("real-branch/repo");
        create_worktree(&repo, &real_wt, "real-branch").unwrap();

        // Create an orphan directory (looks like a worktree but not known to git)
        let orphan = base.join("stale-branch/repo");
        fs::create_dir_all(&orphan).unwrap();
        fs::write(orphan.join(".git"), "gitdir: fake").unwrap();

        let orphans = find_orphan_worktree_dirs(&repo);
        assert_eq!(orphans, vec![orphan]);
    }

    #[test]
    fn test_find_orphan_worktree_dirs_empty() {
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

        assert!(find_orphan_worktree_dirs(&repo).is_empty());
    }

    #[test]
    fn test_encode_branch_for_path() {
        assert_eq!(encode_branch_for_path("main"), "main");
        assert_eq!(encode_branch_for_path("ACT-123"), "ACT-123");
        assert_eq!(encode_branch_for_path("feature/auth"), "feature--auth");
        assert_eq!(
            encode_branch_for_path("user/nested/deep"),
            "user--nested--deep"
        );
    }
}
