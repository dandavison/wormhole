use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

pub const TEST_PREFIX: &str = "wh-test-";

pub struct WormholeTest {
    port: u16,
    tmux_socket: String,
}

impl WormholeTest {
    pub fn new(port: u16) -> Self {
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
        let lua = format!(
            r#"local s, b = require("hs.http").get("http://127.0.0.1:{}{}"); if s == 200 then return b else error("HTTP " .. s) end"#,
            self.port, path
        );
        self.run_hs(&lua)
    }

    pub fn hs_post(&self, path: &str) -> Result<String, String> {
        let lua = format!(
            r#"local s, b = require("hs.http").post("http://127.0.0.1:{}{}", "", nil); if s == 200 then return b else error("HTTP " .. s) end"#,
            self.port, path
        );
        self.run_hs(&lua)
    }

    #[allow(dead_code)]
    pub fn get_focused_app(&self) -> String {
        let lua = r#"local w = hs.window.focusedWindow(); if w then return w:application():title() else return "" end"#;
        self.run_hs(lua).unwrap_or_default()
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

    #[allow(dead_code)]
    pub fn wait_for_app_focus(&self, expected_app: &str, timeout_secs: u64) -> bool {
        self.wait_until(|| self.get_focused_app() == expected_app, timeout_secs)
    }

    #[allow(dead_code)]
    pub fn assert_editor_has_focus(&self) {
        assert!(
            self.wait_for_app_focus("Cursor", 5),
            "Expected Cursor to have focus, but {} has focus",
            self.get_focused_app()
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
        let _ = Command::new("tmux")
            .args(["-L", &self.tmux_socket, "kill-server"])
            .output();
    }
}
