use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

/// Test suite that uses real Cursor windows and Hammerspoon for assertions
/// Run with: cargo test --test test_real_editor -- --test-threads=1

struct RealEditorTest {
    session_name: String,
    socket_name: String,
    port: u16,
    initial_cursor_windows: Vec<String>,
}

impl RealEditorTest {
    fn new(name: &str, port: u16) -> Self {
        let session_name = format!("wormhole-real-test-{}", name);
        let socket_name = format!("wormhole-real-socket-{}", name);

        // Kill any existing test server
        let _ = Command::new("tmux")
            .args(&["-L", &socket_name, "kill-server"])
            .output();

        // Get initial Cursor windows so we can close only our test windows
        let initial_cursor_windows = Self::get_cursor_windows();

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

        let test = RealEditorTest {
            session_name,
            socket_name,
            port,
            initial_cursor_windows,
        };

        // Start wormhole in real editor mode
        test.start_wormhole();
        test.wait_for_wormhole();

        test
    }

    fn start_wormhole(&self) {
        // Start wormhole with real editor test mode
        let cmd = format!(
            "WORMHOLE_PORT={} WORMHOLE_REAL_EDITOR_TEST_MODE=1 ./target/debug/wormhole",
            self.port
        );
        self.send_tmux_keys(&cmd);
        self.send_tmux_keys("Enter");
    }

    fn wait_for_wormhole(&self) {
        for _ in 0..20 {
            if self.is_wormhole_ready() {
                return;
            }
            thread::sleep(Duration::from_millis(250));
        }
        panic!("Wormhole failed to start on port {}", self.port);
    }

    fn is_wormhole_ready(&self) -> bool {
        ureq::get(&format!("http://localhost:{}/list-projects/", self.port))
            .timeout(Duration::from_millis(500))
            .call()
            .is_ok()
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

    fn add_project(&self, name: &str, path: &str) {
        let url = format!(
            "http://localhost:{}/add-project/{}?name={}",
            self.port, path, name
        );
        ureq::post(&url)
            .call()
            .expect(&format!("Failed to add project {}", name));
    }

    fn open_project(&self, name: &str, focus: Option<&str>) {
        let url = if let Some(focus) = focus {
            format!(
                "http://localhost:{}/open-project/{}?land-in={}",
                self.port, name, focus
            )
        } else {
            format!("http://localhost:{}/open-project/{}", self.port, name)
        };
        ureq::get(&url)
            .call()
            .expect(&format!("Failed to open project {}", name));
    }

    /// Get the currently focused application using Hammerspoon
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

    /// Get list of Cursor window titles using Hammerspoon
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

    /// Close Cursor windows that were opened during this test
    fn close_test_cursor_windows(&self) {
        let current_windows = Self::get_cursor_windows();
        for window_title in current_windows {
            if !self.initial_cursor_windows.contains(&window_title) {
                // This window was created during our test, close it
                let _ = Command::new("hs")
                    .args(&[
                        "-c",
                        &format!(
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
                            window_title
                        ),
                    ])
                    .output();
            }
        }
    }

    /// Wait until a condition is met or timeout
    fn wait_until<F>(condition: F, timeout: Duration, check_interval: Duration) -> bool
    where
        F: Fn() -> bool,
    {
        let start = Instant::now();
        while start.elapsed() < timeout {
            if condition() {
                return true;
            }
            thread::sleep(check_interval);
        }
        false
    }

    /// Wait for the focused application to be the expected one
    fn wait_for_focus(app_name: &str, timeout: Duration) -> bool {
        Self::wait_until(
            || {
                let focused = Self::get_focused_app();
                println!(
                    "  Checking focus: current={}, expected={}",
                    focused, app_name
                );
                focused == app_name
            },
            timeout,
            Duration::from_millis(100),
        )
    }

    /// Focus a specific application using Hammerspoon
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
        // Give the app time to actually get focus
        thread::sleep(Duration::from_millis(200));
    }

    /// Wait for a Cursor window with the given title substring to exist
    fn wait_for_cursor_window_containing(substring: &str, timeout: Duration) -> bool {
        Self::wait_until(
            || {
                let windows = Self::get_cursor_windows();
                windows.iter().any(|title| title.contains(substring))
            },
            timeout,
            Duration::from_millis(100),
        )
    }
}

impl Drop for RealEditorTest {
    fn drop(&mut self) {
        // Close any Cursor windows we opened
        self.close_test_cursor_windows();

        // Clean up tmux session
        let _ = Command::new("tmux")
            .args(&["-L", &self.socket_name, "kill-server"])
            .output();
    }
}

#[test]
fn test_real_editor_focus_switching() {
    // Create test project directory
    std::fs::create_dir_all("/tmp/real-editor-test-project").ok();

    let test = RealEditorTest::new("focus", 8900);

    // Add project
    test.add_project("test-project", "/tmp/real-editor-test-project");

    // Determine what terminal app is being used
    let terminal_app = match Command::new("hs")
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
            return 'Alacritty'  -- fallback
        "#,
        ])
        .output()
    {
        Ok(output) => String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_matches('"')
            .to_string(),
        Err(_) => "Alacritty".to_string(),
    };

    println!("Detected terminal application: {}", terminal_app);

    // TEST 1: Open with editor focus (should focus Cursor)
    println!("\nTest 1: Opening project with land-in=editor...");

    // Start from known state - focus terminal first
    RealEditorTest::focus_app(&terminal_app);
    assert_eq!(
        RealEditorTest::get_focused_app(),
        terminal_app,
        "Should start with terminal focused"
    );

    test.open_project("test-project", Some("editor"));

    // Wait for Cursor window to appear
    assert!(
        RealEditorTest::wait_for_cursor_window_containing(
            "real-editor-test-project",
            Duration::from_secs(10),
        ),
        "Cursor window should appear"
    );

    // Verify Cursor gets focus
    assert!(
        RealEditorTest::wait_for_focus("Cursor", Duration::from_secs(3)),
        "Cursor should get focus when opening with land-in=editor"
    );
    println!("✓ Cursor correctly received focus with land-in=editor");

    // TEST 2: Re-open with terminal focus (should focus terminal)
    println!("\nTest 2: Re-opening project with land-in=terminal...");

    // Ensure we start with Cursor focused this time
    RealEditorTest::focus_app("Cursor");
    assert_eq!(
        RealEditorTest::get_focused_app(),
        "Cursor",
        "Should have Cursor focused before test"
    );

    test.open_project("test-project", Some("terminal"));

    // Give time for focus change
    thread::sleep(Duration::from_millis(500));

    // Verify terminal gets focus
    assert!(
        RealEditorTest::wait_for_focus(&terminal_app, Duration::from_secs(3)),
        "Terminal should get focus when opening with land-in=terminal"
    );
    println!("✓ Terminal correctly received focus with land-in=terminal");

    println!("\n✓ Focus switching test completed successfully!");
}

#[test]
fn test_real_editor_default_focus_behavior() {
    // Create test directory with a file
    std::fs::create_dir_all("/tmp/real-default-test").ok();
    std::fs::write("/tmp/real-default-test/test.rs", "fn main() {}").ok();

    let test = RealEditorTest::new("default", 8901);

    // Add project
    test.add_project("default-test", "/tmp/real-default-test");

    // Detect terminal app
    let terminal_app = match Command::new("hs")
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
    };

    println!("Using terminal: {}", terminal_app);

    // TEST 1: Opening a file should default to editor focus
    println!("\nTest 1: Opening a file (should default to editor focus)...");

    // Start with terminal focused
    RealEditorTest::focus_app(&terminal_app);

    // Open a specific file (without specifying land-in)
    let file_url = format!("http://localhost:8901/file//tmp/real-default-test/test.rs");
    ureq::get(&file_url).call().expect("Failed to open file");

    // Wait for Cursor window
    assert!(
        RealEditorTest::wait_for_cursor_window_containing("test.rs", Duration::from_secs(10)),
        "Cursor window should open for file"
    );

    // Verify Cursor gets focus (files default to editor)
    assert!(
        RealEditorTest::wait_for_focus("Cursor", Duration::from_secs(3)),
        "Opening a file should focus Cursor by default"
    );
    println!("✓ File correctly defaulted to editor focus");

    // TEST 2: Opening a project without land-in uses system default
    println!("\nTest 2: Opening project without land-in (uses default)...");

    // Focus terminal again
    RealEditorTest::focus_app(&terminal_app);

    // Open project without specifying land-in
    test.open_project("default-test", None);

    thread::sleep(Duration::from_millis(500));

    // Just verify the window opened - default behavior may vary
    let windows = RealEditorTest::get_cursor_windows();
    assert!(
        windows.iter().any(|w| w.contains("real-default-test")),
        "Project window should open"
    );

    let focused = RealEditorTest::get_focused_app();
    println!("Project opened with focus on: {}", focused);

    println!("\n✓ Default focus behavior test completed!");
}
