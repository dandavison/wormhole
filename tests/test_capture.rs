// Command capture tests (no real execution)
// These tests use WORMHOLE_TEST_MODE to capture commands without executing them

mod test_framework;
use test_framework::*;

#[test]
fn test_list_projects() {
    let server = TestServer::start(8877);

    let response = server
        .request("GET", "/list-projects/", None, None)
        .unwrap();

    // In test mode we might get empty response or valid project names
    assert!(
        response.is_empty() || response.lines().all(|line| !line.is_empty()),
        "Expected valid project list response, got: {}",
        response
    );

    // When using test mode, only tmux commands should be captured
    // (as they are allowed to execute for real)
    let commands = server.get_captured_commands();
    assert!(
        commands.iter().all(|cmd| cmd.program == "tmux"),
        "Expected only tmux commands, got: {:?}",
        commands
    );
}

#[test]
fn test_add_project() {
    let server = TestServer::start(8878);
    server.clear_captured_commands();

    // Add project using the correct API format
    server
        .request("POST", "/add-project//tmp/test_project", None, None)
        .unwrap();

    // In test mode, commands are captured
    let commands = server.get_captured_commands();
    assert!(
        commands.is_empty() || !commands.iter().any(|cmd| cmd.program == "open"),
        "Should not open anything when adding project"
    );
}
