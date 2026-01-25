#![allow(dead_code)]

use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

pub const TEST_PREFIX: &str = "wh-test-";

const NOTIFICATION_GROUP: &str = "wormhole-test";

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
    tmux_socket: String,
}

impl WormholeTest {
    pub fn new(port: u16) -> Self {
        notify_start();

        let tmux_socket = format!("wormhole-test-{}", port);
        let _ = Command::new("tmux")
            .args(["-L", &tmux_socket, "kill-server"])
            .output();

        let current_dir =
            std::env::current_dir().unwrap_or_else(|_| panic!("Failed to get current directory"));
        Command::new("tmux")
            .args([
                "-L",
                &tmux_socket,
                "new-session",
                "-d",
                "-c",
                current_dir.to_str().unwrap(),
                "./target/debug/wormhole",
            ])
            .env("WORMHOLE_PORT", port.to_string())
            .output()
            .unwrap_or_else(|_| panic!("Failed to start wormhole in tmux"));

        let test = WormholeTest { port, tmux_socket };

        for _ in 0..20 {
            if test.hs_get("/list-projects/").is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(250));
        }

        test
    }

    pub fn hs_get(&self, path: &str) -> Result<String, String> {
        self.hs_request_with_body("get", path, "")
    }

    pub fn hs_put(&self, path: &str, body: &str) -> Result<String, String> {
        self.hs_request_with_body("put", path, body)
    }

    pub fn hs_post(&self, path: &str) -> Result<String, String> {
        self.hs_request_with_body("post", path, "")
    }

    fn hs_request_with_body(&self, method: &str, path: &str, body: &str) -> Result<String, String> {
        let url = format!("http://127.0.0.1:{}{}", self.port, path);
        let call = match method {
            "get" => format!(r#"hs.http.get("{}", nil)"#, url),
            _ => format!(r#"hs.http.{}("{}", "{}", nil)"#, method, url, body),
        };
        let lua = format!(
            r#"local s, b = {}; if s == 200 then return b else error("HTTP " .. s) end"#,
            call
        );
        self.run_hs(&lua)
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
        let lua = r#"hs.application.launchOrFocus("/Applications/Alacritty.app")"#;
        self.run_hs(lua).unwrap();
        assert!(
            self.wait_for_app_focus("Alacritty", 5),
            "Failed to focus terminal"
        );
    }

    pub fn create_project(&self, dir: &str, name: &str) {
        self.hs_get(&format!("/project/{}?name={}", dir, name))
            .unwrap();
        assert!(
            self.wait_for_window_containing(name, 10),
            "Project window '{}' did not appear",
            name
        );
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
            .args(["-L", &self.tmux_socket, "display-message", "-p", "#W"])
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
        self.close_cursor_window(TEST_PREFIX);
        let _ = Command::new("tmux")
            .args(["-L", &self.tmux_socket, "kill-server"])
            .output();
        let _ = std::fs::remove_file("/tmp/wormhole.env");
        self.focus_terminal();
        notify_end();
    }
}
