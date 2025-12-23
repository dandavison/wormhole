use std::process::Command;
use std::thread;
use std::time::Duration;

/// Integration test helper that manages a tmux session for testing
struct TmuxTestSession {
    session_name: String,
    socket_name: String, // Using -L flag instead of -S for better isolation
}

impl TmuxTestSession {
    fn new(name: &str) -> Self {
        let session_name = format!("wormhole-test-{}", name);
        let socket_name = format!("wormhole-test-socket-{}", name);

        // Kill any existing test server on this socket
        let _ = Command::new("tmux")
            .args(&["-L", &socket_name, "kill-server"])
            .output();

        // Get the current directory for wormhole
        let current_dir = std::env::current_dir().expect("Failed to get current directory");

        // Create a new tmux session in the wormhole directory
        let output = Command::new("tmux")
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

        if !output.status.success() {
            panic!(
                "Failed to start tmux session: {:?}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        // Give tmux a moment to initialize
        thread::sleep(Duration::from_millis(100));

        TmuxTestSession {
            session_name,
            socket_name,
        }
    }

    fn tmux_cmd(&self, args: &[&str]) -> String {
        let mut cmd_args = vec!["-L", &self.socket_name];
        cmd_args.extend_from_slice(args);

        let output = Command::new("tmux")
            .args(&cmd_args)
            .output()
            .expect("Failed to execute tmux command");

        String::from_utf8_lossy(&output.stdout).to_string()
    }

    fn create_window(&self, name: &str, dir: &str) {
        self.tmux_cmd(&["new-window", "-n", name, "-c", dir]);
    }

    fn list_windows(&self) -> Vec<String> {
        self.tmux_cmd(&["list-windows", "-F", "#W"])
            .lines()
            .map(|s| s.to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    fn capture_pane(&self, window_name: Option<&str>) -> String {
        let target = if let Some(name) = window_name {
            format!("{}:{}", self.session_name, name)
        } else {
            self.session_name.clone()
        };

        self.tmux_cmd(&["capture-pane", "-t", &target, "-p"])
    }

    fn send_keys(&self, keys: &str, window_name: Option<&str>) {
        let target = if let Some(name) = window_name {
            format!("{}:{}", self.session_name, name)
        } else {
            self.session_name.clone()
        };

        // Handle special keys like Enter and C-c
        if keys == "Enter" {
            self.tmux_cmd(&["send-keys", "-t", &target, "Enter"]);
        } else if keys == "C-c" {
            self.tmux_cmd(&["send-keys", "-t", &target, "C-c"]);
        } else {
            self.tmux_cmd(&["send-keys", "-t", &target, keys]);
        }
    }

    #[allow(dead_code)]
    fn socket_path(&self) -> String {
        // Return path that would be used with -S flag
        // This is for TMUX environment variable
        format!("/tmp/tmux-1000/{}", self.socket_name)
    }
}

impl Drop for TmuxTestSession {
    fn drop(&mut self) {
        // Clean up the tmux server entirely
        let _ = Command::new("tmux")
            .args(&["-L", &self.socket_name, "kill-server"])
            .output();
    }
}

#[test]
fn test_wormhole_in_tmux_session() {
    // Create an isolated tmux session for testing
    let tmux = TmuxTestSession::new("wormhole-test");

    // Create test project directories
    std::fs::create_dir_all("/tmp/test-project-a")
        .expect("Failed to create test project directory");
    std::fs::create_dir_all("/tmp/test-project-b")
        .expect("Failed to create test project directory");

    // Start wormhole inside the tmux session
    // The TMUX environment variable is automatically set correctly inside the session
    let wormhole_path = std::env::current_dir()
        .unwrap()
        .join("target/debug/wormhole");

    if !wormhole_path.exists() {
        panic!(
            "Wormhole binary not found at {:?}. Run 'cargo build' first.",
            wormhole_path
        );
    }

    // Start wormhole with test mode to use TestEditor instead of real editor
    // We still get real tmux but avoid spawning actual editor windows
    tmux.send_keys(
        "WORMHOLE_PORT=8885 WORMHOLE_TEST_MODE=/tmp/test_capture_8885.json ./target/debug/wormhole",
        None,
    );
    tmux.send_keys("Enter", None);

    // Wait for wormhole to start - check if it's actually running
    thread::sleep(Duration::from_millis(1000));

    // Verify wormhole started by checking if we can connect
    let mut wormhole_ready = false;
    for _ in 0..10 {
        if ureq::get("http://localhost:8885/list-projects/")
            .timeout(std::time::Duration::from_millis(500))
            .call()
            .is_ok()
        {
            wormhole_ready = true;
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }

    if !wormhole_ready {
        let output = tmux.capture_pane(None);
        panic!("Wormhole failed to start. Output:\n{}", output);
    }

    // Verify wormhole is running by checking the pane
    let pane_content = tmux.capture_pane(None);
    println!("Wormhole output:\n{}", pane_content);

    // Make HTTP requests to wormhole
    // Note: add-project expects absolute path, optionally with ?name=xxx
    println!("Adding project proj-a...");
    let add_result =
        ureq::post("http://localhost:8885/add-project//tmp/test-project-a?name=proj-a").call();
    match add_result {
        Ok(response) => {
            let body = response.into_string().unwrap_or_default();
            println!("Successfully added proj-a: {}", body);
        }
        Err(e) => {
            let output = tmux.capture_pane(None);
            println!("Wormhole output:\n{}", output);
            tmux.send_keys("C-c", None);
            panic!("Failed to add project: {}", e);
        }
    }

    // List projects to verify it was added
    println!("Listing projects...");
    let list_result = ureq::get("http://localhost:8885/list-projects/").call();
    match list_result {
        Ok(response) => {
            let body = response.into_string().unwrap_or_default();
            println!("Current projects:\n{}", body);
        }
        Err(e) => {
            println!("Failed to list projects: {}", e);
        }
    }

    // Open the project - this should create a new tmux window
    println!("Opening project proj-a...");
    let open_result = ureq::get("http://localhost:8885/open-project/proj-a").call();
    match open_result {
        Ok(response) => {
            let body = response.into_string().unwrap_or_default();
            println!("Opened proj-a: {}", body);
        }
        Err(e) => {
            println!("Failed to open project: {}", e);
        }
    }

    // Give tmux more time to create the window
    thread::sleep(Duration::from_millis(2000));

    // Check that a new window was created
    let windows = tmux.list_windows();
    println!("Windows after opening project: {:?}", windows);

    // Also capture what wormhole is outputting
    let wormhole_output = tmux.capture_pane(None);
    println!("Wormhole pane content:\n{}", wormhole_output);

    // List all tmux sessions and windows for debugging
    let all_sessions = tmux.tmux_cmd(&["list-sessions", "-F", "#S"]);
    println!("All sessions: {}", all_sessions);

    let all_windows = tmux.tmux_cmd(&["list-windows", "-a", "-F", "#S:#W"]);
    println!("All windows across sessions: {}", all_windows);

    assert!(
        windows.iter().any(|w| w.contains("proj-a")),
        "Expected proj-a window to be created. Windows found: {:?}",
        windows
    );

    // Add and open another project
    let _ = ureq::post("http://localhost:8885/add-project//tmp/test-project-b?name=proj-b").call();
    let _ = ureq::get("http://localhost:8885/open-project/proj-b").call();
    thread::sleep(Duration::from_millis(500));

    let windows = tmux.list_windows();
    assert!(
        windows.iter().any(|w| w.contains("proj-b")),
        "Expected proj-b window to be created"
    );

    // Clean up: send Ctrl-C to stop wormhole
    tmux.send_keys("C-c", None);
    thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_wormhole_project_navigation() {
    // Test that wormhole properly navigates between projects in tmux
    let tmux = TmuxTestSession::new("nav-test");

    // Start wormhole with test mode to use TestEditor instead of real editor
    tmux.send_keys(
        "WORMHOLE_PORT=8886 WORMHOLE_TEST_MODE=/tmp/test_capture_8886.json ./target/debug/wormhole",
        None,
    );
    tmux.send_keys("Enter", None);

    // Wait for wormhole to be ready
    let mut wormhole_ready = false;
    for _ in 0..10 {
        if ureq::get("http://localhost:8886/list-projects/")
            .timeout(std::time::Duration::from_millis(500))
            .call()
            .is_ok()
        {
            wormhole_ready = true;
            break;
        }
        thread::sleep(Duration::from_millis(500));
    }

    if !wormhole_ready {
        let output = tmux.capture_pane(None);
        panic!("Wormhole failed to start. Output:\n{}", output);
    }

    // Create project directories
    std::fs::create_dir_all("/tmp/alpha").ok();
    std::fs::create_dir_all("/tmp/beta").ok();
    std::fs::create_dir_all("/tmp/gamma").ok();

    // Add multiple projects
    let _ = ureq::post("http://localhost:8886/add-project//tmp/alpha").call();
    let _ = ureq::post("http://localhost:8886/add-project//tmp/beta").call();
    let _ = ureq::post("http://localhost:8886/add-project//tmp/gamma").call();

    // Open projects in sequence
    let _ = ureq::get("http://localhost:8886/open-project/alpha").call();
    thread::sleep(Duration::from_millis(300));

    let _ = ureq::get("http://localhost:8886/open-project/beta").call();
    thread::sleep(Duration::from_millis(300));

    let _ = ureq::get("http://localhost:8886/open-project/gamma").call();
    thread::sleep(Duration::from_millis(300));

    // Verify all windows exist
    let windows = tmux.list_windows();
    println!("All windows: {:?}", windows);
    assert!(
        windows.iter().any(|w| w.contains("alpha")),
        "alpha window should exist"
    );
    assert!(
        windows.iter().any(|w| w.contains("beta")),
        "beta window should exist"
    );
    assert!(
        windows.iter().any(|w| w.contains("gamma")),
        "gamma window should exist"
    );

    // Test navigation endpoints
    let _ = ureq::get("http://localhost:8886/previous-project/").call();
    thread::sleep(Duration::from_millis(200));

    let _ = ureq::get("http://localhost:8886/next-project/").call();
    thread::sleep(Duration::from_millis(200));

    // Close a project
    let _ = ureq::post("http://localhost:8886/close-project/beta").call();
    thread::sleep(Duration::from_millis(300));

    let windows_after_close = tmux.list_windows();
    assert!(
        !windows_after_close.iter().any(|w| w.contains("beta")),
        "beta window should be closed"
    );

    // Clean up
    tmux.send_keys("C-c", None);
    thread::sleep(Duration::from_millis(100));
}

#[test]
fn test_tmux_capture_pane() {
    // Demonstrate tmux capture-pane functionality for future test development
    let tmux = TmuxTestSession::new("capture");

    // Send some text to the pane
    tmux.send_keys("echo 'Hello from tmux test'", None);
    tmux.send_keys("Enter", None);

    // Wait for command to execute
    thread::sleep(Duration::from_millis(100));

    // Capture the pane content
    let content = tmux.capture_pane(None);
    println!("Captured pane content:\n{}", content);

    // Verify we can capture output
    assert!(
        content.contains("Hello from tmux test"),
        "Should capture echoed text"
    );

    // Create a new window and test capture there
    tmux.create_window("test-window", "/tmp");
    thread::sleep(Duration::from_millis(100)); // Let window initialize

    tmux.send_keys("pwd", Some("test-window"));
    tmux.send_keys("Enter", Some("test-window"));

    thread::sleep(Duration::from_millis(200)); // Give more time for command to execute

    let window_content = tmux.capture_pane(Some("test-window"));
    println!("Window pane content:\n{}", window_content);

    // Just verify we can capture something from the window
    assert!(
        !window_content.is_empty(),
        "Should capture content from test-window"
    );
}
