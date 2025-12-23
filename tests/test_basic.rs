mod test_framework;

use test_framework::*;

#[test]
fn test_list_projects() {
    let server = TestServer::start(8877);

    let response = server.request("GET", "/list-projects/", None, None).unwrap();
    // In test mode, the project list will be empty since tmux commands aren't executed
    // Just verify the endpoint responds with a string (empty or with project names)
    assert!(response.is_empty() || response.lines().all(|line| !line.is_empty()),
            "Expected valid project list response, got: {}", response);

    // List projects should only query tmux, not open anything
    let commands = server.get_captured_commands();
    assert!(commands.iter().all(|cmd| cmd.program == "tmux"),
            "Expected only tmux commands, got: {:?}", commands);
}

#[test]
fn test_add_project() {
    let server = TestServer::start(8878);
    server.clear_captured_commands();

    // Add a new project
    server.request("POST", "/add-project/test_project", None, None).unwrap();

    let commands = server.get_captured_commands();

    // Should not execute external commands for adding project
    assert!(commands.is_empty() || !commands.iter().any(|cmd| cmd.program == "open"),
            "Should not open anything when adding project");
}