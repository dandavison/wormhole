use serde_json;
use std::process::Command;
use std::thread;
use std::time::Duration;

/// Realistic integration test that makes HTTP requests through Hammerspoon
/// This simulates actual usage and can catch deadlock issues

struct HammerspoonTest {
    session_name: String,
    socket_name: String,
    port: u16,
}

impl HammerspoonTest {
    fn new(name: &str, port: u16) -> Self {
        let session_name = format!("wormhole-hs-test-{}", name);
        let socket_name = format!("wormhole-hs-socket-{}", name);

        // Check if wormhole binary exists
        if !std::path::Path::new("./target/debug/wormhole").exists() {
            panic!("Wormhole binary not found. Run 'cargo build' first.");
        }

        // Kill any existing test server
        let _ = Command::new("tmux")
            .args(&["-L", &socket_name, "kill-server"])
            .output();

        // Create tmux session in wormhole directory
        let current_dir = std::env::current_dir().expect("Failed to get current directory");

        Command::new("tmux")
            .args(&[
                "-L",
                &socket_name,
                "new-session",
                "-d",
                "-s",
                &session_name,
                "-c",
                &current_dir.to_str().unwrap(),
            ])
            .output()
            .expect("Failed to create tmux session");

        thread::sleep(Duration::from_millis(100));

        let test = HammerspoonTest {
            session_name,
            socket_name,
            port,
        };

        // Start wormhole in real editor mode
        test.start_wormhole();
        test.wait_for_wormhole();

        test
    }

    fn start_wormhole(&self) {
        let cmd = format!(
            "WORMHOLE_PORT={} WORMHOLE_REAL_EDITOR_TEST_MODE=1 ./target/debug/wormhole",
            self.port
        );
        self.send_tmux_keys(&cmd);
        self.send_tmux_keys("Enter");
    }

    fn wait_for_wormhole(&self) {
        println!("Waiting for wormhole to start on port {}...", self.port);
        for i in 0..20 {
            if self.is_wormhole_ready() {
                println!("Wormhole ready after {} attempts", i + 1);
                return;
            }
            thread::sleep(Duration::from_millis(250));
        }

        // Try a direct HTTP request to see if wormhole is up but Hammerspoon is the issue
        if let Ok(resp) = ureq::get(&format!("http://localhost:{}/list-projects/", self.port))
            .timeout(Duration::from_millis(500))
            .call()
        {
            println!("Wormhole is running but Hammerspoon HTTP requests are failing");
            println!("Response from direct HTTP: {:?}", resp.status());
            panic!("Hammerspoon integration not working. Is Hammerspoon installed and running?");
        }

        panic!("Wormhole failed to start on port {}", self.port);
    }

    fn is_wormhole_ready(&self) -> bool {
        // Use Hammerspoon to check if wormhole is ready
        match self.hs_http_get("/list-projects/") {
            Ok(_) => true,
            Err(e) => {
                // Expected during startup, only log if verbose
                if std::env::var("TEST_DEBUG").is_ok() {
                    println!("Wormhole not ready yet: {}", e);
                }
                false
            }
        }
    }

    fn send_tmux_keys(&self, keys: &str) {
        Command::new("tmux")
            .args(&[
                "-L",
                &self.socket_name,
                "send-keys",
                "-t",
                &self.session_name,
                keys,
            ])
            .output()
            .expect("Failed to send keys");
    }

    /// Make an HTTP request through Hammerspoon (simulates real usage)
    fn hs_http_get(&self, path: &str) -> Result<String, String> {
        // Hammerspoon's http.get actually returns (status_code, body, headers) when successful
        // or (error_code, nil) when failed (error_code is negative or 0)
        let lua_script = format!(
            r#"
            local http = require("hs.http")
            local status, body = http.get("http://127.0.0.1:{}{}")
            if status == 200 and type(body) == "string" then
                return body
            elseif status > 0 then
                error("HTTP request failed with status: " .. tostring(status))
            else
                error("HTTP request failed with error code: " .. tostring(status))
            end
            "#,
            self.port, path
        );

        let output = Command::new("hs")
            .args(&["-c", &lua_script])
            .output()
            .map_err(|e| format!("Failed to run Hammerspoon: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }

    /// Make an HTTP POST request through Hammerspoon
    fn hs_http_post(&self, path: &str) -> Result<String, String> {
        let lua_script = format!(
            r#"
            local http = require("hs.http")
            local status, body = http.post("http://127.0.0.1:{}{}", "", nil)
            if status == 200 and type(body) == "string" then
                return body
            elseif status > 0 then
                error("HTTP POST failed with status: " .. tostring(status))
            else
                error("HTTP POST failed with error code: " .. tostring(status))
            end
            "#,
            self.port, path
        );

        let output = Command::new("hs")
            .args(&["-c", &lua_script])
            .output()
            .map_err(|e| format!("Failed to run Hammerspoon: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }

    /// Make an HTTP PUT request through Hammerspoon
    fn hs_http_put(&self, path: &str, data: &str) -> Result<String, String> {
        let lua_script = format!(
            r#"
            local http = require("hs.http")
            local headers = {{["Content-Type"] = "text/plain"}}
            local status, body = http.doRequest("http://127.0.0.1:{}{}", "PUT", "{}", headers)
            if status == 200 and type(body) == "string" then
                return body
            elseif status > 0 then
                error("HTTP PUT failed with status: " .. tostring(status))
            else
                error("HTTP PUT failed with error code: " .. tostring(status))
            end
            "#,
            self.port, path, data
        );

        let output = Command::new("hs")
            .args(&["-c", &lua_script])
            .output()
            .map_err(|e| format!("Failed to run Hammerspoon: {}", e))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
        }
    }

    fn get_focused_app() -> String {
        let output = Command::new("hs")
            .args(&["-c", "hs.window.focusedWindow():application():name()"])
            .output()
            .expect("Failed to run Hammerspoon command");

        String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_matches('"')
            .to_string()
    }

    fn focus_app(app_name: &str) {
        let _ = Command::new("hs")
            .args(&[
                "-c",
                &format!(
                    r#"
                    local app = hs.application.find('{}')
                    if app then
                        app:activate()
                        return true
                    else
                        return false
                    end
                "#,
                    app_name
                ),
            ])
            .output();
        thread::sleep(Duration::from_millis(200));
    }

    fn get_cursor_windows() -> Vec<String> {
        let output = Command::new("hs")
            .args(&[
                "-c",
                r#"
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
            "#,
            ])
            .output()
            .expect("Failed to run Hammerspoon command");

        let json_str = String::from_utf8_lossy(&output.stdout);
        serde_json::from_str(&json_str).unwrap_or_else(|_| vec![])
    }
}

impl Drop for HammerspoonTest {
    fn drop(&mut self) {
        // Clean up tmux session
        let _ = Command::new("tmux")
            .args(&["-L", &self.socket_name, "kill-server"])
            .output();
    }
}

#[test]
fn test_navigation_via_hammerspoon() {
    // Create test directories
    std::fs::create_dir_all("/tmp/hs-test-project-a").ok();
    std::fs::create_dir_all("/tmp/hs-test-project-b").ok();

    let test = HammerspoonTest::new("hs-nav", 8920);

    println!("\n=== Hammerspoon Integration Test ===");

    // List initial projects
    let initial_projects = test
        .hs_http_get("/list-projects/")
        .expect("Failed to list initial projects");
    println!("Initial projects: {}", initial_projects);

    // Add projects through Hammerspoon
    println!("Adding projects via Hammerspoon HTTP requests...");
    let add_result_a = test
        .hs_http_post("/add-project//tmp/hs-test-project-a?name=proj-a")
        .expect("Failed to add project-a");
    println!("Add project-a result: {}", add_result_a);

    let add_result_b = test
        .hs_http_post("/add-project//tmp/hs-test-project-b?name=proj-b")
        .expect("Failed to add project-b");
    println!("Add project-b result: {}", add_result_b);

    // List projects to verify
    let projects = test
        .hs_http_get("/list-projects/")
        .expect("Failed to list projects");
    println!("Projects after adding: {}", projects);
    assert!(
        projects.contains("proj-a"),
        "Projects should contain proj-a"
    );
    assert!(
        projects.contains("proj-b"),
        "Projects should contain proj-b"
    );

    // Set KV for proj-b to land in editor
    println!("Setting proj-b land-in=editor via Hammerspoon...");
    test.hs_http_put("/kv/proj-b/land-in", "editor")
        .expect("Failed to set KV");

    // Open proj-b (should land in editor due to KV)
    println!("Opening proj-b via Hammerspoon...");
    test.hs_http_get("/project/proj-b")
        .expect("Failed to open proj-b");

    thread::sleep(Duration::from_millis(3000));

    // Verify window opened
    let windows = HammerspoonTest::get_cursor_windows();
    assert!(
        windows.iter().any(|w| w.contains("hs-test-project-b")),
        "proj-b window should be open"
    );

    let focus = HammerspoonTest::get_focused_app();
    println!("Focus after opening proj-b (has land-in=editor): {}", focus);

    // Navigate to previous project FROM EDITOR through Hammerspoon
    // This is the critical test - would have caught the deadlock
    println!("\nNavigating to previous project via Hammerspoon (from editor)...");
    HammerspoonTest::focus_app("Cursor");
    thread::sleep(Duration::from_millis(500));

    let before_nav = HammerspoonTest::get_focused_app();
    println!("Focus before navigation: {}", before_nav);

    // THIS WOULD HAVE DEADLOCKED with the previous implementation!
    test.hs_http_get("/previous-project/")
        .expect("Failed to navigate to previous project");

    thread::sleep(Duration::from_millis(3000));

    let after_nav = HammerspoonTest::get_focused_app();
    println!(
        "Focus after /previous-project/ via Hammerspoon: {}",
        after_nav
    );

    // Should stay in editor when navigating from editor
    if after_nav == "Cursor" {
        println!("✓ Stayed in editor when navigating from editor");
    }

    // Navigate again FROM TERMINAL
    println!("\nNavigating to next project via Hammerspoon (from terminal)...");

    // Focus terminal first
    let terminal = detect_terminal_app();
    HammerspoonTest::focus_app(&terminal);
    thread::sleep(Duration::from_millis(500));

    let before_nav2 = HammerspoonTest::get_focused_app();
    println!("Focus before navigation: {}", before_nav2);

    test.hs_http_get("/next-project/")
        .expect("Failed to navigate to next project");

    thread::sleep(Duration::from_millis(3000));

    let after_nav2 = HammerspoonTest::get_focused_app();
    println!("Focus after /next-project/ via Hammerspoon: {}", after_nav2);

    // Should stay in terminal when navigating from terminal
    if after_nav2 == terminal {
        println!("✓ Stayed in terminal when navigating from terminal");
    }

    println!("\n✓ Hammerspoon integration test completed successfully!");
    println!("This test makes HTTP requests through Hammerspoon, just like real usage.");
}

#[test]
fn test_link_simulation() {
    // Simulate clicking on a wormhole link
    // In practice, these might be GitHub URLs that redirect to wormhole

    std::fs::create_dir_all("/tmp/link-test-project").ok();
    std::fs::write("/tmp/link-test-project/test.rs", "fn main() {}").ok();

    let test = HammerspoonTest::new("link", 8921);

    println!("\n=== Link Click Simulation Test ===");

    // Add project
    test.hs_http_post("/add-project//tmp/link-test-project?name=link-proj")
        .expect("Failed to add project");

    // Simulate clicking on a file link (like from GitHub)
    // These typically open in editor
    println!("Simulating file link click via Hammerspoon...");
    test.hs_http_get("/file//tmp/link-test-project/test.rs?line=1")
        .expect("Failed to open file");

    thread::sleep(Duration::from_millis(3000));

    // Verify the file opened in Cursor
    let windows = HammerspoonTest::get_cursor_windows();
    assert!(
        windows
            .iter()
            .any(|w| w.contains("test.rs") || w.contains("link-test-project")),
        "File should be open in Cursor"
    );

    let focus = HammerspoonTest::get_focused_app();
    println!("Focus after file link click: {}", focus);

    // Files should default to editor focus
    if focus == "Cursor" {
        println!("✓ File link correctly focused editor");
    }

    println!("\n✓ Link simulation test completed!");
}

fn detect_terminal_app() -> String {
    match Command::new("hs")
        .args(&[
            "-c",
            r#"
            local terminals = {'Alacritty', 'Terminal', 'iTerm2', 'kitty', 'WezTerm'}
            for _, name in ipairs(terminals) do
                local app = hs.application.find(name)
                if app and app:isRunning() then
                    return name
                end
            end
            return 'Alacritty'
        "#,
        ])
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_matches('"')
            .to_string(),
        Err(_) => "Alacritty".to_string(),
    }
}
