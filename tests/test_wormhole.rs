use std::process::Command;
use std::thread;
use std::time::Duration;

/// Helper to create an isolated tmux session for testing
struct WormholeTmuxTest {
    session_name: String,
    socket_name: String,
    port: u16,
}

impl WormholeTmuxTest {
    fn new(name: &str, port: u16) -> Self {
        let session_name = format!("wormhole-test-{}", name);
        let socket_name = format!("wormhole-test-socket-{}", name);

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

        let test = WormholeTmuxTest {
            session_name,
            socket_name,
            port,
        };

        // Start wormhole inside the tmux session
        test.start_wormhole();
        test.wait_for_wormhole();

        test
    }

    fn start_wormhole(&self) {
        // Start wormhole with integration test mode
        // This ensures real tmux but no editor windows
        let cmd = format!(
            "WORMHOLE_PORT={} WORMHOLE_INTEGRATION_TEST_MODE=1 ./target/debug/wormhole",
            self.port
        );
        self.send_keys(&cmd);
        self.send_keys("Enter");
    }

    fn wait_for_wormhole(&self) {
        // Poll until wormhole is ready
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

    fn send_keys(&self, keys: &str) {
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

    fn capture_pane(&self) -> String {
        let output = Command::new("tmux")
            .args(&[
                "-L",
                &self.socket_name,
                "capture-pane",
                "-t",
                &self.session_name,
                "-p",
            ])
            .output()
            .expect("Failed to capture pane");

        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn list_windows(&self) -> Vec<String> {
        let output = Command::new("tmux")
            .args(&["-L", &self.socket_name, "list-windows", "-F", "#W"])
            .output()
            .expect("Failed to list windows");

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect()
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

    fn open_project(&self, name: &str) {
        let url = format!("http://localhost:{}/open-project/{}", self.port, name);
        ureq::get(&url)
            .call()
            .expect(&format!("Failed to open project {}", name));
    }

    fn close_project(&self, name: &str) {
        let url = format!("http://localhost:{}/close-project/{}", self.port, name);
        ureq::post(&url)
            .call()
            .expect(&format!("Failed to close project {}", name));
    }

    fn navigate(&self, direction: &str) {
        let url = format!("http://localhost:{}/{}/", self.port, direction);
        ureq::get(&url)
            .call()
            .expect(&format!("Failed to navigate {}", direction));
    }
}

impl Drop for WormholeTmuxTest {
    fn drop(&mut self) {
        // Clean up tmux session
        let _ = Command::new("tmux")
            .args(&["-L", &self.socket_name, "kill-server"])
            .output();
    }
}

// ============================================================================
// Clean Integration Test Example
// ============================================================================

#[test]
fn test_complete_project_workflow() {
    // Setup: Create test directories
    std::fs::create_dir_all("/tmp/project-alpha").ok();
    std::fs::create_dir_all("/tmp/project-beta").ok();
    std::fs::create_dir_all("/tmp/project-gamma").ok();

    // Start wormhole in a tmux session
    let test = WormholeTmuxTest::new("workflow", 8890);

    // Add projects
    test.add_project("alpha", "/tmp/project-alpha");
    test.add_project("beta", "/tmp/project-beta");
    test.add_project("gamma", "/tmp/project-gamma");

    // Open first project - should create tmux window
    test.open_project("alpha");
    thread::sleep(Duration::from_millis(500));

    let windows = test.list_windows();
    assert!(
        windows.contains(&"alpha".to_string()),
        "Alpha window should exist after opening"
    );

    // Open second project
    test.open_project("beta");
    thread::sleep(Duration::from_millis(500));

    let windows = test.list_windows();
    assert!(
        windows.contains(&"beta".to_string()),
        "Beta window should exist"
    );

    // Navigate between projects
    test.navigate("previous-project");
    thread::sleep(Duration::from_millis(200));

    test.navigate("next-project");
    thread::sleep(Duration::from_millis(200));

    // Close a project - window should disappear
    test.close_project("alpha");
    thread::sleep(Duration::from_millis(500));

    let windows = test.list_windows();
    assert!(
        !windows.contains(&"alpha".to_string()),
        "Alpha window should be gone after closing"
    );
    assert!(
        windows.contains(&"beta".to_string()),
        "Beta window should still exist"
    );

    // Verify wormhole is still running and responsive
    assert!(
        test.is_wormhole_ready(),
        "Wormhole should still be responsive"
    );

    // Check captured output for any errors
    let output = test.capture_pane();
    assert!(!output.contains("ERROR"), "No errors in wormhole output");
    assert!(!output.contains("panic"), "No panics in wormhole output");
}

#[test]
fn test_tmux_window_focus() {
    let test = WormholeTmuxTest::new("focus", 8891);

    test.add_project("focus-test", "/tmp/focus-test");

    // Open with terminal focus
    let url = format!("http://localhost:8891/open-project/focus-test?land-in=terminal");
    ureq::get(&url)
        .call()
        .expect("Failed to open with terminal focus");
    thread::sleep(Duration::from_millis(500));

    // Verify window was created
    let windows = test.list_windows();
    assert!(windows.contains(&"focus-test".to_string()));

    // In a real test, we could verify focus behavior by checking
    // the active window, but that requires more tmux introspection
}
