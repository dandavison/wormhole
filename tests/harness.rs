use core::panic;
use serde_json;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

pub struct WormholeTest {
    port: u16,
    tmux_socket: String,
    initial_cursor_windows: Vec<String>,
}

impl WormholeTest {
    pub fn new(port: u16) -> Self {
        let tmux_socket = format!("wormhole-test-{}", port);
        let _ = Command::new("tmux")
            .args(&["-L", &tmux_socket, "kill-server"])
            .output();

        let initial_cursor_windows = Self::get_cursor_windows();

        // Start wormhole in tmux
        let current_dir =
            std::env::current_dir().unwrap_or_else(|_| panic!("Failed to get current directory"));
        Command::new("tmux")
            .args(&[
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

        let test = WormholeTest {
            port,
            tmux_socket,
            initial_cursor_windows,
        };

        // Wait for wormhole to be ready
        for _ in 0..20 {
            if test.hs_get("/list-projects/").is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(250));
        }

        test
    }

    fn get_cursor_windows() -> Vec<String> {
        let lua = r#"
            local cursor = hs.application.find('Cursor')
            if cursor then
                local windows = cursor:allWindows()
                local titles = {}
                for i, window in ipairs(windows) do
                    table.insert(titles, window:title())
                end
                return hs.json.encode(titles)
            else
                return '[]'
            end
        "#;

        Command::new("hs")
            .args(&["-c", lua])
            .output()
            .ok()
            .and_then(|output| {
                if output.status.success() {
                    let json = String::from_utf8_lossy(&output.stdout);
                    serde_json::from_str(&json).ok()
                } else {
                    None
                }
            })
            .unwrap_or_else(Vec::new)
    }

    fn close_test_cursor_windows(&self) {
        let current_windows = Self::get_cursor_windows();

        for window_title in current_windows {
            if !self.initial_cursor_windows.contains(&window_title) {
                // This window was created during our test, close it
                let lua = format!(
                    r#"
                    local cursor = hs.application.find('Cursor')
                    if cursor then
                        local windows = cursor:allWindows()
                        for i, window in ipairs(windows) do
                            if window:title() == '{}' then
                                window:close()
                            end
                        end
                    end
                "#,
                    window_title.replace("'", "\\'").replace("\"", "\\\"")
                );

                let _ = Command::new("hs").args(&["-c", &lua]).output();
            }
        }
    }

    pub fn hs_get(&self, path: &str) -> Result<String, String> {
        let lua = format!(
            r#"local s, b = require("hs.http").get("http://127.0.0.1:{}{}");
               if s == 200 then return b else error("HTTP " .. s) end"#,
            self.port, path
        );
        self.run_hs(&lua)
    }

    pub fn hs_post(&self, path: &str) -> Result<String, String> {
        let lua = format!(
            r#"local s, b = require("hs.http").post("http://127.0.0.1:{}{}", "", nil);
               if s == 200 then return b else error("HTTP " .. s) end"#,
            self.port, path
        );
        self.run_hs(&lua)
    }

    pub fn get_focused_app(&self) -> String {
        let lua = r#"
            local focusedWindow = hs.window.focusedWindow()
            if focusedWindow then
                return focusedWindow:application():title()
            else
                return ""
            end
        "#;
        self.run_hs(lua).unwrap_or_else(|_| String::new())
    }

    #[allow(dead_code)]
    pub fn wait_until<F>(&self, mut predicate: F, timeout_secs: u64) -> bool
    where
        F: FnMut() -> bool,
    {
        let timeout = Duration::from_secs(timeout_secs);
        let start = Instant::now();

        while start.elapsed() < timeout {
            if predicate() {
                return true;
            }
            thread::sleep(Duration::from_millis(100));
        }
        false
    }

    fn run_hs(&self, lua: &str) -> Result<String, String> {
        let output = Command::new("hs")
            .args(&["-c", lua])
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
        self.close_test_cursor_windows();
        let _ = Command::new("tmux")
            .args(&["-L", &self.tmux_socket, "kill-server"])
            .output();
    }
}
