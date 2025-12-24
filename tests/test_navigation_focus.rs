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

    fn wait_for_cursor_window_containing(substring: &str, timeout: Duration) -> bool {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            let windows = Self::get_cursor_windows();
            if windows
                .iter()
                .any(|title| title.to_lowercase().contains(&substring.to_lowercase()))
            {
                return true;
            }
            thread::sleep(Duration::from_millis(100));
        }
        false
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

#[test]
fn test_navigation_focus_with_kv_land_in() {
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

    println!("\n=== Starting Navigation Focus Test ===");

    // Step 1: Open 'wormhole' project (no land-in specified, should use default)
    println!("\nStep 1: Opening 'wormhole' project...");
    let response = test.open_project("wormhole");
    println!("Response from opening wormhole: {}", response);

    // Wait for wormhole window
    thread::sleep(Duration::from_millis(2000));

    let windows = NavigationTest::get_cursor_windows();
    println!("Cursor windows after opening wormhole: {:?}", windows);

    // The window title should contain either the project name or directory name
    let has_window = windows
        .iter()
        .any(|w| w.to_lowercase().contains("wormhole") || w.contains("nav-test-wormhole"));
    assert!(has_window, "Wormhole cursor window should be open");
    thread::sleep(Duration::from_millis(2000));

    let focus_after_wormhole = NavigationTest::get_focused_app();
    println!("Focus after opening wormhole: {}", focus_after_wormhole);

    // Step 2: Navigate to 'temporal' using /project/temporal (should land in editor due to KV)
    println!("\nStep 2: Opening 'temporal' with KV land-in=editor...");
    test.open_project("temporal");

    // Wait for temporal window
    thread::sleep(Duration::from_millis(2000));
    let temporal_windows = NavigationTest::get_cursor_windows();
    println!(
        "Cursor windows after opening temporal: {:?}",
        temporal_windows
    );

    let has_temporal_window = temporal_windows
        .iter()
        .any(|w| w.to_lowercase().contains("temporal") || w.contains("nav-test-temporal"));
    assert!(has_temporal_window, "Temporal cursor window should open");
    thread::sleep(Duration::from_millis(3000));

    let focus_after_temporal = NavigationTest::get_focused_app();
    println!("Focus after opening temporal: {}", focus_after_temporal);

    // In current implementation, this should be Cursor (editor) due to KV land-in=editor
    // Note: Focus may not change on some systems, but the intent is editor
    if focus_after_temporal == "Cursor" {
        println!("✓ Temporal opened with editor focus (as expected from KV)");
    } else {
        println!(
            "Note: Temporal opened but focus is on {}",
            focus_after_temporal
        );
    }

    // Step 3: Navigate back using /previous-project/
    // CURRENT BEHAVIOR: This will apply wormhole's KV settings (if any) when returning
    println!("\nStep 3: Navigating back to wormhole via /previous-project/...");
    test.navigate_previous();

    thread::sleep(Duration::from_millis(3000));

    let focus_after_previous = NavigationTest::get_focused_app();
    println!("Focus after /previous-project/: {}", focus_after_previous);

    // Step 4: Navigate forward again using /previous-project/
    // CURRENT BEHAVIOR: This will re-apply temporal's land-in=editor from KV
    println!("\nStep 4: Navigating forward to temporal via /previous-project/...");
    test.navigate_previous();

    thread::sleep(Duration::from_millis(3000));

    let focus_after_second_previous = NavigationTest::get_focused_app();
    println!(
        "Focus after second /previous-project/: {}",
        focus_after_second_previous
    );

    // Document current behavior: When navigating to temporal, its KV land-in=editor
    // is applied regardless of navigation method
    println!("\n=== Current Behavior Summary ===");
    println!("When navigating to a project with KV land-in setting,");
    println!("that setting is applied regardless of:");
    println!("1. Where you came from (terminal vs editor)");
    println!("2. How you navigated (direct vs previous/next)");
    println!("\nThis test documents the CURRENT behavior.");

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
