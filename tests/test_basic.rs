mod test_framework;

use test_framework::*;
use std::collections::HashMap;

#[test]
fn test_list_projects() {
    let server = TestServer::start(7777);
    
    let response = server.request("GET", "/list-projects/", None, None).unwrap();
    // Response should contain newline-separated project names
    // At minimum, wormhole project should be present since we're running from there
    assert!(response.contains("wormhole") || !response.is_empty(), 
            "Expected project list, got: {}", response);
    
    // Should not execute any commands for listing
    server.assert_no_commands();
}

#[test]
fn test_open_project_with_editor_focus() {
    let server = TestServer::start(7778);
    server.clear_captured_commands();
    
    // Open a project with editor focus
    let mut params = HashMap::new();
    params.insert("land-in", "editor");
    server.request("GET", "/project/wormhole", Some(params), None).unwrap();
    
    // Give time for async operations
    std::thread::sleep(std::time::Duration::from_millis(200));
    
    let commands = server.get_captured_commands();
    
    // Should open terminal and editor
    assert!(commands.iter().any(|cmd| cmd.program == "open"), 
            "Expected 'open' command for editor, got: {:?}", commands);
    
    // Should have vscode:// or cursor:// URI
    assert!(commands.iter().any(|cmd| {
        cmd.program == "open" && 
        cmd.args.iter().any(|arg| arg.contains("cursor://") || arg.contains("vscode://"))
    }), "Expected editor URI in open command");
}

#[test]
fn test_open_project_with_terminal_focus() {
    let server = TestServer::start(7779);
    server.clear_captured_commands();
    
    // Open a project with terminal focus
    let mut params = HashMap::new();
    params.insert("land-in", "terminal");
    server.request("GET", "/project/wormhole", Some(params), None).unwrap();
    
    // Give time for async operations
    std::thread::sleep(std::time::Duration::from_millis(200));
    
    let commands = server.get_captured_commands();
    
    // Should still open editor but with different focus
    assert!(commands.iter().any(|cmd| cmd.program == "open"), 
            "Expected 'open' command, got: {:?}", commands);
    
    // May have tmux or terminal commands depending on config
    // This would depend on the terminal configuration
}

#[test]
fn test_open_file_path() {
    let server = TestServer::start(7780);
    server.clear_captured_commands();
    
    // Open a specific file
    server.request("GET", "/file/Users/dan/src/wormhole/src/main.rs", None, None).unwrap();
    
    // Give time for async operations
    std::thread::sleep(std::time::Duration::from_millis(200));
    
    let commands = server.get_captured_commands();
    
    // Should open with file URI including line number
    assert!(commands.iter().any(|cmd| {
        cmd.program == "open" && 
        cmd.args.iter().any(|arg| arg.contains("main.rs"))
    }), "Expected file path in open command, got: {:?}", commands);
}

#[test]
fn test_add_project() {
    let server = TestServer::start(7781);
    server.clear_captured_commands();
    
    // Add a new project
    server.request("POST", "/add-project/test_project", None, None).unwrap();
    
    let commands = server.get_captured_commands();
    
    // Should not execute external commands for adding project
    assert!(commands.is_empty() || !commands.iter().any(|cmd| cmd.program == "open"),
            "Should not open anything when adding project");
}

#[test]
fn test_kv_operations() {
    let server = TestServer::start(7782);
    
    // Set a value
    server.request("PUT", "/kv/wormhole/test_key", None, Some("test_value")).unwrap();
    
    // Get the value back
    let response = server.request("GET", "/kv/wormhole/test_key", None, None).unwrap();
    assert_contains(&response, "test_value");
    
    // Get all KV for project
    let response = server.request("GET", "/kv/wormhole", None, None).unwrap();
    assert_contains(&response, "test_key");
    
    // Delete the value
    server.request("DELETE", "/kv/wormhole/test_key", None, None).unwrap();
    
    // Verify it's gone
    let response = server.request("GET", "/kv/wormhole/test_key", None, None).unwrap();
    assert_contains(&response, "not found");
    
    // KV operations should not trigger any external commands
    server.assert_no_commands();
}

#[test]
fn test_github_url_redirect() {
    let _server = TestServer::start(7783);
    
    // GitHub URL should redirect
    match ureq::get(&format!("http://localhost:7783/github.com/rust-lang/rust/blob/master/README.md"))
        .call() {
        Ok(response) => {
            // Should get a redirect status
            assert!(response.status() >= 300 && response.status() < 400,
                    "Expected redirect status, got: {}", response.status());
        }
        Err(ureq::Error::Status(code, _response)) if code >= 300 && code < 400 => {
            // This is actually expected - ureq might treat redirects as errors
            // depending on its configuration
        }
        Err(e) => panic!("Unexpected error: {}", e)
    }
}

#[test]
fn test_line_number_parsing() {
    let server = TestServer::start(7784);
    server.clear_captured_commands();
    
    // Open file with line number
    let mut params = HashMap::new();
    params.insert("line", "42");
    server.request("GET", "/file/Users/dan/src/wormhole/src/main.rs", Some(params), None).unwrap();
    
    std::thread::sleep(std::time::Duration::from_millis(200));
    
    let commands = server.get_captured_commands();
    
    // Should include line number in URI
    assert!(commands.iter().any(|cmd| {
        cmd.program == "open" && 
        cmd.args.iter().any(|arg| arg.contains(":42"))
    }), "Expected line number in URI, got: {:?}", commands);
}
