mod harness;

use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::Duration;
use tempfile::TempDir;

struct ServerTest {
    _temp: TempDir,
    port: u16,
    repos_dir: PathBuf,
    server_process: Option<Child>,
}

impl ServerTest {
    fn new(port: u16) -> Self {
        let temp = TempDir::new().unwrap();
        let repos_dir = temp.path().join("repos");
        fs::create_dir_all(&repos_dir).unwrap();

        Self {
            _temp: temp,
            port,
            repos_dir,
            server_process: None,
        }
    }

    fn create_repo(&self, name: &str) -> PathBuf {
        let repo_path = self.repos_dir.join(name);
        fs::create_dir_all(&repo_path).unwrap();

        Command::new("git")
            .args(["init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        // Create initial commit so worktrees work
        Command::new("git")
            .args(["commit", "--allow-empty", "-m", "init"])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        repo_path
    }

    fn create_worktree(&self, repo_name: &str, branch: &str) -> PathBuf {
        let repo_path = self.repos_dir.join(repo_name);
        let worktree_path = self.repos_dir.join(branch);

        Command::new("git")
            .args(["worktree", "add", "-b", branch, worktree_path.to_str().unwrap()])
            .current_dir(&repo_path)
            .output()
            .unwrap();

        worktree_path
    }

    fn start_server(&mut self) {
        let wormhole_path = self.repos_dir.to_str().unwrap();

        let child = Command::new("./target/debug/wormhole")
            .env("WORMHOLE_PORT", self.port.to_string())
            .env("WORMHOLE_PATH", wormhole_path)
            .env("WORMHOLE_EDITOR", "none")
            .env("WORMHOLE_TERMINAL", "none")
            .spawn()
            .unwrap();

        self.server_process = Some(child);
        wormhole::daemon::wait_for_ready(self.port, Duration::from_secs(5));
    }

    fn get(&self, path: &str) -> Value {
        let url = format!("http://127.0.0.1:{}{}", self.port, path);
        let output = Command::new("curl")
            .args(["-s", &url])
            .output()
            .unwrap();
        let body = String::from_utf8(output.stdout).unwrap();
        serde_json::from_str(&body).unwrap_or_else(|_| Value::String(body))
    }

    fn post(&self, path: &str) {
        let url = format!("http://127.0.0.1:{}{}", self.port, path);
        let _ = Command::new("curl")
            .args(["-s", "-X", "POST", &url])
            .output();
    }
}

impl Drop for ServerTest {
    fn drop(&mut self) {
        if let Some(ref mut child) = self.server_process {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

#[test]
fn test_project_list_excludes_worktrees_as_projects() {
    let mut test = ServerTest::new(18901);

    // Create a repo
    test.create_repo("my-repo");

    // Create a worktree (sibling to repo)
    test.create_worktree("my-repo", "my-branch");

    test.start_server();

    let response = test.get("/project/list");
    let available = response["available"].as_array().unwrap();

    // my-repo should be in available
    assert!(
        available.iter().any(|v| v.as_str() == Some("my-repo")),
        "my-repo should be in available projects"
    );

    // my-branch (the worktree) should NOT be in available
    assert!(
        !available.iter().any(|v| v.as_str() == Some("my-branch")),
        "worktree 'my-branch' should not appear as a project"
    );
}

#[test]
fn test_project_list_shows_tasks_with_repo_branch_format() {
    let mut test = ServerTest::new(18902);

    test.create_repo("cli");
    test.create_worktree("cli", "feature-branch");

    test.start_server();

    // Switch to the task to add it to the ring
    test.get("/project/switch/cli:feature-branch");

    std::thread::sleep(Duration::from_millis(500));

    let response = test.get("/project/list");
    let current = response["current"].as_array().unwrap();

    // Should have a task with name=cli, branch=feature-branch
    let task = current.iter().find(|item| {
        item["name"].as_str() == Some("cli") && item["branch"].as_str() == Some("feature-branch")
    });

    assert!(task.is_some(), "Task cli:feature-branch should be in current list");
}
