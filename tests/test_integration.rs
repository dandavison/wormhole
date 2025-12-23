use std::process::Command;
use std::thread;
use std::time::Duration;

/// Integration test helper that manages a tmux session for testing
struct TmuxTestSession {
    session_name: String,
    socket_name: String,  // Using -L flag instead of -S for better isolation
}

impl TmuxTestSession {
    fn new(name: &str) -> Self {
        let session_name = format!("wormhole-test-{}", name);
        let socket_name = format!("wormhole-test-socket-{}", name);

        // Kill any existing test server on this socket
        let _ = Command::new("tmux")
            .args(&["-L", &socket_name, "kill-server"])
            .output();

        // Create a new tmux session
        let output = Command::new("tmux")
            .args(&["-L", &socket_name, "new-session", "-d", "-s", &session_name, "-c", "/tmp"])
            .output()
            .expect("Failed to create tmux session");

        if !output.status.success() {
            panic!("Failed to start tmux session: {:?}", String::from_utf8_lossy(&output.stderr));
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

        self.tmux_cmd(&["send-keys", "-t", &target, keys]);
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
fn test_real_tmux_project_discovery() {
    // Create a test tmux session with some windows
    let tmux = TmuxTestSession::new("discovery");

    // Create some test project directories
    let _ = std::fs::create_dir_all("/tmp/test-project-1");
    let _ = std::fs::create_dir_all("/tmp/test-project-2");

    // Create tmux windows for these projects
    tmux.create_window("project1", "/tmp/test-project-1");
    tmux.create_window("project2", "/tmp/test-project-2");

    // Verify tmux windows were created
    let windows = tmux.list_windows();
    assert!(windows.contains(&"project1".to_string()), "Expected project1 window");
    assert!(windows.contains(&"project2".to_string()), "Expected project2 window");

    // Capture pane content to verify tmux is working
    let pane_content = tmux.capture_pane(Some("project1"));
    println!("Pane content for project1: {:?}", pane_content);

    // Note: Testing wormhole's tmux integration would require running wormhole
    // within the test tmux session, which is complex. For now, we verify
    // the tmux test infrastructure works.
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
    assert!(content.contains("Hello from tmux test"), "Should capture echoed text");

    // Create a new window and test capture there
    tmux.create_window("test-window", "/tmp");
    thread::sleep(Duration::from_millis(100));  // Let window initialize

    tmux.send_keys("pwd", Some("test-window"));
    tmux.send_keys("Enter", Some("test-window"));

    thread::sleep(Duration::from_millis(200));  // Give more time for command to execute

    let window_content = tmux.capture_pane(Some("test-window"));
    println!("Window pane content:\n{}", window_content);

    // Just verify we can capture something from the window
    assert!(!window_content.is_empty(), "Should capture content from test-window");
}
