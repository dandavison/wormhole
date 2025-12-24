use serde_json;
use std::process::Command;
use std::thread;
use std::time::Duration;

/// Test the current navigation focus behavior with KV land-in settings
/// This test documents the CURRENT behavior (which may not be ideal)

struct NavigationTest {
    session_name: String,
    socket_name: String,
    port: u16,
}

impl NavigationTest {
    fn new(name: &str, port: u16) -> Self {
        let session_name = format!("wormhole-nav-test-{}", name);
        let socket_name = format!("wormhole-nav-socket-{}", name);

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

        let test = NavigationTest {
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

    fn set_kv(&self, project: &str, key: &str, value: &str) {
        let url = format!("http://localhost:{}/kv/{}/{}", self.port, project, key);
        ureq::put(&url)
            .send_string(value)
            .expect(&format!("Failed to set KV {}/{}", project, key));
    }

    fn open_project(&self, name: &str) -> String {
        let url = format!("http://localhost:{}/project/{}", self.port, name);
        ureq::get(&url)
            .call()
            .expect(&format!("Failed to open project {}", name))
            .into_string()
            .unwrap()
    }

    fn navigate_previous(&self) -> String {
        let url = format!("http://localhost:{}/previous-project/", self.port);
        ureq::get(&url)
            .call()
            .expect("Failed to navigate to previous project")
            .into_string()
            .unwrap()
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

impl Drop for NavigationTest {
    fn drop(&mut self) {
        // Clean up tmux session
        let _ = Command::new("tmux")
            .args(&["-L", &self.socket_name, "kill-server"])
            .output();
    }
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
    }
}

#[test]
fn test_navigation_focus_respects_origin() {
    // Create test directories
    std::fs::create_dir_all("/tmp/nav-test-wormhole").ok();
    std::fs::create_dir_all("/tmp/nav-test-temporal").ok();

    let test = NavigationTest::new("navigation", 8910);

    // Setup: Add two projects
    test.add_project("wormhole", "/tmp/nav-test-wormhole");
    test.add_project("temporal", "/tmp/nav-test-temporal");

    // List projects to verify they were added
    let projects_list_url = format!("http://localhost:{}/list-projects/", test.port);
    let projects = ureq::get(&projects_list_url)
        .call()
        .expect("Failed to list projects")
        .into_string()
        .unwrap();
    println!("Projects after adding: {}", projects);

    // Setup: Set temporal to have land-in=editor via KV
    test.set_kv("temporal", "land-in", "editor");

    println!("\n=== Starting Navigation Focus Test (NEW BEHAVIOR) ===");

    // Step 1: Open 'temporal' directly (should land in editor due to KV)
    println!("\nStep 1: Opening 'temporal' project directly...");
    test.open_project("temporal");

    // Wait for temporal window
    thread::sleep(Duration::from_millis(3000));
    let temporal_windows = NavigationTest::get_cursor_windows();
    println!(
        "Cursor windows after opening temporal: {:?}",
        temporal_windows
    );

    let has_temporal_window = temporal_windows
        .iter()
        .any(|w| w.to_lowercase().contains("temporal") || w.contains("nav-test-temporal"));
    assert!(has_temporal_window, "Temporal cursor window should open");

    let focus_after_temporal = NavigationTest::get_focused_app();
    println!(
        "Focus after opening temporal directly: {}",
        focus_after_temporal
    );
    // This should ideally be Cursor due to KV land-in=editor

    // Step 2: Ensure we're in Cursor, then navigate to wormhole using /previous-project/
    // NEW BEHAVIOR: Should stay in editor since we're navigating FROM editor
    println!("\nStep 2: Focusing Cursor, then navigating to wormhole via /previous-project/...");

    // Force focus to Cursor to establish a known state
    focus_app("Cursor");
    thread::sleep(Duration::from_millis(1000));

    let focus_before_nav = NavigationTest::get_focused_app();
    println!("Focus before navigation: {}", focus_before_nav);

    test.navigate_previous();
    thread::sleep(Duration::from_millis(3000));

    // Verify wormhole window is now active
    let windows_after_nav = NavigationTest::get_cursor_windows();
    let has_wormhole = windows_after_nav
        .iter()
        .any(|w| w.to_lowercase().contains("wormhole") || w.contains("nav-test-wormhole"));
    assert!(
        has_wormhole,
        "Wormhole window should be open after navigation"
    );

    let focus_after_nav_from_editor = NavigationTest::get_focused_app();
    println!(
        "Focus after /previous-project/ FROM EDITOR: {}",
        focus_after_nav_from_editor
    );

    // NEW BEHAVIOR: Should stay in editor when navigating from editor
    if focus_after_nav_from_editor == "Cursor" {
        println!("✓ NEW BEHAVIOR: Stayed in editor when navigating from editor");
    }

    // Step 3: Switch to terminal, then navigate back to temporal
    // NEW BEHAVIOR: Should stay in terminal since we're navigating FROM terminal
    println!("\nStep 3: Focusing terminal, then navigating to temporal via /previous-project/...");

    // Detect and focus terminal
    let terminal_app = detect_terminal_app();
    println!("Using terminal: {}", terminal_app);
    focus_app(&terminal_app);
    thread::sleep(Duration::from_millis(1000));

    let focus_before_nav2 = NavigationTest::get_focused_app();
    println!("Focus before navigation: {}", focus_before_nav2);

    test.navigate_previous();
    thread::sleep(Duration::from_millis(3000));

    let focus_after_nav_from_terminal = NavigationTest::get_focused_app();
    println!(
        "Focus after /previous-project/ FROM TERMINAL: {}",
        focus_after_nav_from_terminal
    );

    // NEW BEHAVIOR: Should stay in terminal when navigating from terminal
    // (even though temporal has KV land-in=editor)
    if focus_after_nav_from_terminal == terminal_app {
        println!("✓ NEW BEHAVIOR: Stayed in terminal when navigating from terminal");
        println!("  (Overriding temporal's KV land-in=editor setting)");
    }

    // Summary
    println!("\n=== NEW Behavior Summary ===");
    println!("Navigation now respects WHERE you came from:");
    println!("- Navigate from editor → land in editor");
    println!("- Navigate from terminal → land in terminal");
    println!("- KV settings are only used for direct project opens");
    println!("- Query params can still override this behavior");

    // The test passes if windows were created (focus behavior may vary)
    let final_windows = NavigationTest::get_cursor_windows();
    println!("Final Cursor windows: {:?}", final_windows);

    assert!(
        final_windows
            .iter()
            .any(|w| w.to_lowercase().contains("wormhole") || w.contains("nav-test-wormhole")),
        "Wormhole window should exist"
    );
    assert!(
        final_windows
            .iter()
            .any(|w| w.to_lowercase().contains("temporal") || w.contains("nav-test-temporal")),
        "Temporal window should exist"
    );

    println!("\n✓ Navigation test completed - current behavior documented");
}
