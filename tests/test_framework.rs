use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct CapturedCommand {
    pub program: String,
    pub args: Vec<String>,
    #[allow(dead_code)]
    pub cwd: String,
    #[allow(dead_code)]
    pub mode: String,
}

pub struct TestServer {
    child: Option<Child>,
    #[allow(dead_code)]
    port: u16,
    capture_file: String,
}

impl TestServer {
    pub fn start(port: u16) -> Self {
        let capture_file = format!("/tmp/wormhole_test_capture_{}.json", port);

        // Clean up any previous capture file
        let _ = fs::remove_file(&capture_file);

        // Start wormhole with test mode enabled
        let child = Command::new("./target/debug/wormhole")
            .env("WORMHOLE_TEST_MODE", &capture_file)
            .env("WORMHOLE_PORT", port.to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to start test wormhole server");

        // Wait for server to be ready
        thread::sleep(Duration::from_millis(500));

        // Verify server is responding
        for _ in 0..10 {
            if Self::health_check(port).is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        TestServer {
            child: Some(child),
            port,
            capture_file,
        }
    }

    #[allow(dead_code)]
    pub fn start_with_tmux(port: u16, tmux_socket: &str) -> Self {
        let capture_file = format!("/tmp/wormhole_test_capture_{}.json", port);

        // Clean up any previous capture file
        let _ = fs::remove_file(&capture_file);

        // Start wormhole with test mode and tmux socket
        let child = Command::new("./target/debug/wormhole")
            .env("WORMHOLE_TEST_MODE", &capture_file)
            .env("WORMHOLE_PORT", port.to_string())
            .env("TMUX", format!("{},1,1", tmux_socket))  // Set TMUX env var to use test socket
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Failed to start test wormhole server");

        // Wait for server to be ready
        thread::sleep(Duration::from_millis(500));

        // Verify server is responding
        for _ in 0..10 {
            if Self::health_check(port).is_ok() {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        TestServer {
            child: Some(child),
            port,
            capture_file,
        }
    }

    fn health_check(port: u16) -> Result<(), Box<dyn std::error::Error>> {
        let response = ureq::get(&format!("http://localhost:{}/list-projects/", port))
            .timeout(Duration::from_secs(1))
            .call()?;

        if response.status() == 200 {
            Ok(())
        } else {
            Err("Server not healthy".into())
        }
    }

    pub fn request(&self, method: &str, path: &str, params: Option<HashMap<&str, &str>>, body: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
        let mut url = format!("http://localhost:{}{}", self.port, path);

        if let Some(params) = params {
            let query: Vec<String> = params.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect();
            if !query.is_empty() {
                url.push_str("?");
                url.push_str(&query.join("&"));
            }
        }

        let request = match method {
            "GET" => ureq::get(&url),
            "POST" => ureq::post(&url),
            "PUT" => ureq::put(&url),
            "DELETE" => ureq::delete(&url),
            _ => panic!("Unsupported method: {}", method),
        };

        let response = if let Some(body) = body {
            request.send_string(body)?
        } else {
            request.call()?
        };

        Ok(response.into_string()?)
    }

    pub fn get_captured_commands(&self) -> Vec<CapturedCommand> {
        // Clear any existing commands first
        self.clear_captured_commands();

        // Give a moment for any pending writes
        thread::sleep(Duration::from_millis(50));

        match fs::read_to_string(&self.capture_file) {
            Ok(content) => {
                content.lines()
                    .filter(|line| !line.is_empty())
                    .filter_map(|line| serde_json::from_str::<CapturedCommand>(line).ok())
                    .collect()
            }
            Err(_) => Vec::new(),
        }
    }

    pub fn clear_captured_commands(&self) {
        let _ = fs::write(&self.capture_file, "");
    }

    #[allow(dead_code)]
    pub fn assert_command_contains(&self, program: &str, arg_substring: &str) -> bool {
        let commands = self.get_captured_commands();
        commands.iter().any(|cmd| {
            cmd.program == program &&
            cmd.args.iter().any(|arg| arg.contains(arg_substring))
        })
    }

    #[allow(dead_code)]
    pub fn assert_no_commands(&self) {
        let commands = self.get_captured_commands();
        assert!(commands.is_empty(), "Expected no commands, but found: {:?}", commands);
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        // Stop the server
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }

        // Clean up capture file
        let _ = fs::remove_file(&self.capture_file);
    }
}

// Test assertion helpers
#[allow(dead_code)]
pub fn assert_contains(haystack: &str, needle: &str) {
    assert!(
        haystack.contains(needle),
        "Expected '{}' to contain '{}'",
        haystack,
        needle
    );
}

#[allow(dead_code)]
pub fn assert_command<'a>(commands: &'a [CapturedCommand], program: &str) -> Option<&'a CapturedCommand> {
    commands.iter().find(|cmd| cmd.program == program)
}

#[allow(dead_code)]
pub fn assert_command_with_arg<'a>(commands: &'a [CapturedCommand], program: &str, arg: &str) -> Option<&'a CapturedCommand> {
    commands.iter().find(|cmd| {
        cmd.program == program && cmd.args.iter().any(|a| a.contains(arg))
    })
}
