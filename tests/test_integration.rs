use serde_json;
use std::process::Command;
use std::thread;
use std::time::Duration;

/// Minimal integration test using Hammerspoon for all HTTP requests
/// This tests the actual code paths used in production

struct WormholeTest {
    port: u16,
    tmux_socket: String,
    initial_cursor_windows: Vec<String>,
}

impl WormholeTest {
    fn new(port: u16) -> Self {
        let tmux_socket = format!("wormhole-test-{}", port);

        // Get list of Cursor windows before test
        let initial_cursor_windows = Self::get_cursor_windows();

        // Kill any existing test tmux
        let _ = Command::new("tmux")
            .args(&["-L", &tmux_socket, "kill-server"])
            .output();

        // Start wormhole in tmux
        let current_dir = std::env::current_dir().expect("Failed to get current directory");

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
            .env("WORMHOLE_REAL_EDITOR_TEST_MODE", "1")
            .output()
            .expect("Failed to start wormhole in tmux");

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

    /// Make GET request through Hammerspoon
    fn hs_get(&self, path: &str) -> Result<String, String> {
        let lua = format!(
            r#"local s, b = require("hs.http").get("http://127.0.0.1:{}{}");
               if s == 200 then return b else error("HTTP " .. s) end"#,
            self.port, path
        );
        self.run_hs(&lua)
    }

    /// Make POST request through Hammerspoon
    fn hs_post(&self, path: &str) -> Result<String, String> {
        let lua = format!(
            r#"local s, b = require("hs.http").post("http://127.0.0.1:{}{}", "", nil);
               if s == 200 then return b else error("HTTP " .. s) end"#,
            self.port, path
        );
        self.run_hs(&lua)
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
        // Close any Cursor windows opened during the test
        self.close_test_cursor_windows();

        // Kill tmux server
        let _ = Command::new("tmux")
            .args(&["-L", &self.tmux_socket, "kill-server"])
            .output();
    }
}

#[test]
fn test_navigation_no_deadlock() {
    // The key test: navigation requests through Hammerspoon shouldn't deadlock
    // This would have caught the issue where wormhole tried to query Hammerspoon
    // synchronously while Hammerspoon was waiting for the HTTP response

    let test = WormholeTest::new(8930);

    // Basic navigation commands through Hammerspoon
    // These should complete without timeout
    match test.hs_get("/previous-project/") {
        Ok(_) => println!("✓ Previous-project succeeded"),
        Err(e) if e.contains("timeout") => panic!("Deadlock detected! {}", e),
        Err(_) => println!("  (No previous project, but no deadlock)"),
    }

    match test.hs_get("/next-project/") {
        Ok(_) => println!("✓ Next-project succeeded"),
        Err(e) if e.contains("timeout") => panic!("Deadlock detected! {}", e),
        Err(_) => println!("  (No next project, but no deadlock)"),
    }

    println!("✓ No Hammerspoon deadlock - test passed!");
}

#[test]
fn test_file_opens_in_editor() {
    std::fs::create_dir_all("/tmp/test-file-proj").ok();
    std::fs::write("/tmp/test-file-proj/test.rs", "fn main() {}").ok();

    let test = WormholeTest::new(8931);

    // Add project
    test.hs_post("/add-project//tmp/test-file-proj?name=file-proj")
        .expect("Failed to add project");

    // Open a file (should default to editor)
    test.hs_get("/file//tmp/test-file-proj/test.rs")
        .expect("Failed to open file");

    thread::sleep(Duration::from_secs(2));

    println!("✓ File opening test passed!");
}
