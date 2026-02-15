#![allow(dead_code)]

use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};
use wormhole::daemon::{wait_for_ready, TmuxSession};

pub const TEST_PREFIX: &str = "wh-test-";

const NOTIFICATION_GROUP: &str = "wormhole-test";

/// Initialize a git repo at the given directory, removing any existing content.
pub fn init_git_repo(dir: &str) {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "--allow-empty", "-m", "init"])
        .current_dir(dir)
        .output()
        .unwrap();
}

fn editor_is_none() -> bool {
    std::env::var("WORMHOLE_EDITOR").ok().as_deref() == Some("none")
}

fn notify_start() {
    let _ = Command::new("terminal-notifier")
        .args([
            "-message",
            "wormhole test running",
            "-title",
            "wormhole test",
            "-group",
            NOTIFICATION_GROUP,
        ])
        .spawn();
}

fn notify_end() {
    let _ = Command::new("terminal-notifier")
        .args(["-remove", NOTIFICATION_GROUP])
        .output();
}
pub enum Focus<'a> {
    Editor(&'a str),
    Terminal(&'a str),
}

pub struct WormholeTest {
    port: u16,
    tmux: TmuxSession,
}

impl WormholeTest {
    pub fn new(port: u16) -> Self {
        if std::env::var("WORMHOLE_TEST").is_err() {
            panic!(
                "Set WORMHOLE_TEST=1 to run tests. \
                If you are an AI you must set WORMHOLE_EDITOR=none to avoid focusing application windows."
            );
        }
        if !editor_is_none() {
            notify_start();
        }

        let socket_name = format!("wormhole-test-{}", port);
        let tmux = TmuxSession::new(&socket_name, "wormhole");
        let current_dir =
            std::env::current_dir().unwrap_or_else(|_| panic!("Failed to get current directory"));

        // Construct the tmux socket path that the daemon should use
        let uid = Command::new("id").args(["-u"]).output().unwrap();
        let uid = String::from_utf8_lossy(&uid.stdout).trim().to_string();
        let socket_path = format!("/private/tmp/tmux-{}/{}", uid, socket_name);

        let mut env_vars: Vec<(&str, &str)> =
            vec![("WORMHOLE_TMUX", &socket_path), ("WORMHOLE_OFFLINE", "1")];
        let wormhole_editor = std::env::var("WORMHOLE_EDITOR").ok();
        if let Some(ref editor) = wormhole_editor {
            env_vars.push(("WORMHOLE_EDITOR", editor));
        }
        tmux.start(
            "./target/debug/wormhole",
            Some(port),
            Some(current_dir.to_str().unwrap()),
            &env_vars,
        )
        .expect("Failed to start wormhole in tmux");

        wait_for_ready(port, Duration::from_secs(5));

        WormholeTest { port, tmux }
    }

    pub fn http_get(&self, path: &str) -> Result<String, String> {
        let url = format!("http://127.0.0.1:{}{}", self.port, path);
        let output = Command::new("curl")
            .args(["-s", "-f", &url])
            .output()
            .map_err(|e| format!("curl failed: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(format!(
                "HTTP error: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub fn http_put(&self, path: &str, body: &str) -> Result<String, String> {
        let url = format!("http://127.0.0.1:{}{}", self.port, path);
        let output = Command::new("curl")
            .args(["-s", "-f", "-X", "PUT", "-d", body, &url])
            .output()
            .map_err(|e| format!("curl failed: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(format!(
                "HTTP error: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub fn http_post(&self, path: &str) -> Result<String, String> {
        let url = format!("http://127.0.0.1:{}{}", self.port, path);
        let output = Command::new("curl")
            .args(["-s", "-f", "-X", "POST", &url])
            .output()
            .map_err(|e| format!("curl failed: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(format!(
                "HTTP error: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    pub fn http_post_json(&self, path: &str, body: &str) -> Result<String, String> {
        let url = format!("http://127.0.0.1:{}{}", self.port, path);
        let output = Command::new("curl")
            .args([
                "-s",
                "-f",
                "-X",
                "POST",
                "-H",
                "Content-Type: application/json",
                "-d",
                body,
                &url,
            ])
            .output()
            .map_err(|e| format!("curl failed: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            Err(format!(
                "HTTP error: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// HTTP GET with custom header. Returns (body, headers) tuple.
    pub fn http_get_with_header(
        &self,
        path: &str,
        header: &str,
    ) -> Result<(String, String), String> {
        let url = format!("http://127.0.0.1:{}{}", self.port, path);
        let output = Command::new("curl")
            .args(["-s", "-i", "-H", header, &url])
            .output()
            .map_err(|e| format!("curl failed: {}", e))?;
        let full = String::from_utf8_lossy(&output.stdout).to_string();
        // Split headers and body (separated by \r\n\r\n or \n\n)
        let (headers, body) = if let Some(pos) = full.find("\r\n\r\n") {
            (full[..pos].to_string(), full[pos + 4..].to_string())
        } else if let Some(pos) = full.find("\n\n") {
            (full[..pos].to_string(), full[pos + 2..].to_string())
        } else {
            (String::new(), full)
        };
        Ok((body, headers))
    }

    pub fn cli(&self, cmd: &str) -> Result<String, String> {
        let full_cmd = format!(
            "timeout 30 sh -c 'WORMHOLE_PORT={} ./target/debug/{}'",
            self.port, cmd
        );
        let output = Command::new("sh")
            .args(["-c", &full_cmd])
            .output()
            .map_err(|e| format!("cli failed: {}", e))?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if output.status.code() == Some(124) {
                Err(format!("cli timed out after 30s: {}", cmd))
            } else {
                Err(format!("cli error: {}", stderr))
            }
        }
    }

    pub fn task_in_list(&self, repo: &str, branch: &str) -> bool {
        let expected_key = format!("{}:{}", repo, branch);
        let response = self.http_get("/project/list").unwrap();
        let data: serde_json::Value = serde_json::from_str(&response).unwrap();
        data["current"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .any(|e| e["project_key"].as_str() == Some(expected_key.as_str()))
            })
            .unwrap_or(false)
    }

    pub fn project_in_list(&self, name: &str) -> bool {
        let response = self.http_get("/project/list").unwrap();
        let data: serde_json::Value = serde_json::from_str(&response).unwrap();
        data["current"]
            .as_array()
            .map(|arr| arr.iter().any(|e| e["project_key"].as_str() == Some(name)))
            .unwrap_or(false)
    }

    pub fn get_focused_app(&self) -> String {
        let lua = r#"local w = hs.window.focusedWindow(); if w then return w:application():title() else return "" end"#;
        self.run_hs(lua).unwrap_or_default()
    }

    pub fn get_focused_window_title(&self) -> String {
        let lua =
            r#"local w = hs.window.focusedWindow(); if w then return w:title() else return "" end"#;
        self.run_hs(lua).unwrap_or_default()
    }

    pub fn focused_window_contains(&self, name: &str) -> bool {
        self.get_focused_window_title().contains(name)
    }

    pub fn window_exists(&self, name: &str) -> bool {
        let lua_pattern = name.replace("-", "%-");
        let lua = format!(
            r#"local cursor = hs.application.find('Cursor'); if cursor then for _, w in ipairs(cursor:allWindows()) do if string.find(w:title(), "{}") then return "true" end end end; return "false""#,
            lua_pattern
        );
        self.run_hs(&lua).map(|s| s == "true").unwrap_or(false)
    }

    pub fn close_cursor_window(&self, name: &str) {
        let lua_pattern = name.replace("-", "%-");
        let lua = format!(
            r#"local cursor = hs.application.find('Cursor'); if cursor then for _, w in ipairs(cursor:allWindows()) do if string.find(w:title(), "{}") then w:close() end end end"#,
            lua_pattern
        );
        let _ = self.run_hs(&lua);
    }

    pub fn wait_until<F>(&self, mut predicate: F, timeout_secs: u64) -> bool
    where
        F: FnMut() -> bool,
    {
        let start = Instant::now();
        let timeout = Duration::from_secs(timeout_secs);
        while start.elapsed() < timeout {
            if predicate() {
                return true;
            }
            thread::sleep(Duration::from_millis(100));
        }
        false
    }

    pub fn wait_for_window_containing(&self, name: &str, timeout_secs: u64) -> bool {
        let name = name.to_string();
        self.wait_until(|| self.window_exists(&name), timeout_secs)
    }

    pub fn wait_for_app_focus(&self, expected_app: &str, timeout_secs: u64) -> bool {
        self.wait_until(|| self.get_focused_app() == expected_app, timeout_secs)
    }

    #[track_caller]
    pub fn assert_focus(&self, focus: Focus) {
        let project = match focus {
            Focus::Editor(p) => p,
            Focus::Terminal(p) => p,
        };
        self.assert_tmux_window(project);

        if editor_is_none() {
            return;
        }

        match focus {
            Focus::Editor(expected_window) => {
                let expected = expected_window.to_string();
                assert!(
                    self.wait_until(|| self.focused_window_contains(&expected), 5),
                    "Expected Cursor window containing '{}' to have focus, got '{}'",
                    expected_window,
                    self.get_focused_window_title()
                );
            }
            Focus::Terminal(_) => {
                assert!(
                    self.wait_for_app_focus("Alacritty", 5),
                    "Expected Alacritty to have focus, but {} has focus",
                    self.get_focused_app()
                );
            }
        }
    }

    pub fn focus_terminal(&self) {
        if editor_is_none() {
            return;
        }
        let lua = r#"hs.application.launchOrFocus("/Applications/Alacritty.app")"#;
        self.run_hs(lua)
            .expect("Failed to run Hammerspoon focus command");
        assert!(
            self.wait_for_app_focus("Alacritty", 5),
            "Failed to focus terminal, got '{}'",
            self.get_focused_app()
        );
    }

    fn focus_terminal_graceful(&self) {
        if editor_is_none() {
            return;
        }
        let lua = r#"hs.application.launchOrFocus("/Applications/Alacritty.app")"#;
        let _ = self.run_hs(lua);
        let _ = self.wait_for_app_focus("Alacritty", 5);
    }

    pub fn create_project(&self, dir: &str, name: &str) {
        self.http_get(&format!("/project/switch/{}?name={}&sync=true", dir, name))
            .unwrap();
        if editor_is_none() {
            self.assert_tmux_window(name);
        } else {
            assert!(
                self.wait_for_window_containing(name, 10),
                "Project window '{}' did not appear",
                name
            );
        }
    }

    /// Create a task. In the new model, branch is the task identity.
    /// The task_id parameter is used as the branch name.
    pub fn create_task(&self, task_id: &str, home_project: &str) {
        // Use sync=1 to get errors if task creation fails
        let response = self
            .http_get(&format!(
                "/project/switch/?home-project={}&branch={}&sync=1",
                home_project, task_id
            ))
            .unwrap();
        assert!(
            response.contains("ok") || response.is_empty(),
            "Task creation failed: {}",
            response
        );
        // Wait a bit for the task to be created and registered
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    /// Create a worktree directly with git (bypassing wormhole), so no terminal
    /// window is created. Returns the worktree path.
    pub fn create_worktree_directly(
        &self,
        home_dir: &str,
        repo_name: &str,
        branch: &str,
    ) -> String {
        let worktrees_dir = format!("{}/.git/wormhole/worktrees", home_dir);
        std::fs::create_dir_all(&worktrees_dir).unwrap();
        let worktree_path = format!("{}/{}/{}", worktrees_dir, branch, repo_name);
        let output = Command::new("git")
            .args(["worktree", "add", "-b", branch, &worktree_path])
            .current_dir(home_dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Failed to create worktree: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        worktree_path
    }

    /// Get the store key for a task (repo:branch format)
    pub fn task_store_key(&self, branch: &str, repo: &str) -> String {
        format!("{}:{}", repo, branch)
    }

    pub fn wait_for_kv(&self, project: &str, key: &str, expected: &str, timeout_secs: u64) -> bool {
        let url = format!("http://127.0.0.1:{}/kv/{}/{}", self.port, project, key);
        self.wait_until(
            || {
                Command::new("curl")
                    .args(["-s", &url])
                    .output()
                    .ok()
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .map(|s| s.trim() == expected)
                    .unwrap_or(false)
            },
            timeout_secs,
        )
    }

    pub fn get_tmux_window_name(&self) -> String {
        let output = Command::new("tmux")
            .args(["-L", &self.tmux.socket, "display-message", "-p", "#W"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    /// Kill a tmux window by name directly (bypassing wormhole)
    pub fn kill_tmux_window(&self, name: &str) {
        // Get window index by name (avoids : being parsed as session:window separator)
        let output = Command::new("tmux")
            .args([
                "-L",
                &self.tmux.socket,
                "list-windows",
                "-F",
                "#{window_index}\t#{window_name}",
            ])
            .output()
            .unwrap();
        let list = String::from_utf8_lossy(&output.stdout);
        for line in list.lines() {
            if let Some((idx, window_name)) = line.split_once('\t') {
                if window_name == name {
                    let _ = Command::new("tmux")
                        .args(["-L", &self.tmux.socket, "kill-window", "-t", idx])
                        .output();
                    return;
                }
            }
        }
    }

    /// Check if a tmux window with the given name exists
    pub fn tmux_window_exists(&self, name: &str) -> bool {
        self.list_tmux_windows().iter().any(|w| w == name)
    }

    /// List all tmux window names
    pub fn list_tmux_windows(&self) -> Vec<String> {
        let output = Command::new("tmux")
            .args(["-L", &self.tmux.socket, "list-windows", "-F", "#W"])
            .output()
            .unwrap();
        let windows = String::from_utf8_lossy(&output.stdout);
        windows.lines().map(|s| s.to_string()).collect()
    }

    pub fn get_tmux_pane_cwd(&self) -> String {
        let output = Command::new("tmux")
            .args([
                "-L",
                &self.tmux.socket,
                "display-message",
                "-p",
                "#{pane_current_path}",
            ])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[track_caller]
    pub fn assert_tmux_window(&self, expected: &str) {
        let expected = expected.to_string();
        assert!(
            self.wait_until(|| self.get_tmux_window_name().contains(&expected), 5),
            "Expected tmux window containing '{}', got '{}'",
            expected,
            self.get_tmux_window_name()
        );
    }

    #[track_caller]
    pub fn assert_tmux_cwd(&self, expected_path: &str) {
        let expected = std::fs::canonicalize(expected_path)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| expected_path.to_string());
        assert!(
            self.wait_until(
                || {
                    let actual = self.get_tmux_pane_cwd();
                    std::fs::canonicalize(&actual)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or(actual)
                        == expected
                },
                5
            ),
            "Expected tmux cwd '{}', got '{}'",
            expected,
            self.get_tmux_pane_cwd()
        );
    }

    fn run_hs(&self, lua: &str) -> Result<String, String> {
        let output = Command::new("hs")
            .args(["-c", lua])
            .output()
            .map_err(|e| format!("Failed to run Hammerspoon: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }
}

impl Drop for WormholeTest {
    fn drop(&mut self) {
        if !editor_is_none() {
            let _ = self.wait_until(
                || {
                    self.close_cursor_window(TEST_PREFIX);
                    !self.window_exists(TEST_PREFIX)
                },
                10,
            );
        }
        self.tmux.stop();
        let _ = std::fs::remove_file("/tmp/wormhole.env");
        if !editor_is_none() {
            self.focus_terminal_graceful();
            notify_end();
        }
    }
}
