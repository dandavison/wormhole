mod test_framework;
use test_framework::*;

/// Test that wormhole issues the correct tmux commands when managing projects
#[test]
fn test_wormhole_tmux_commands() {
    // Start wormhole in test mode (captures commands instead of executing)
    let server = TestServer::start(8881);

    // Wait a moment for startup to complete and initial tmux queries
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Get commands that were executed during startup
    let startup_commands = server.get_captured_commands();
    let tmux_startup: Vec<_> = startup_commands
        .iter()
        .filter(|cmd| cmd.program == "tmux")
        .collect();

    println!("Startup tmux commands: {:?}", tmux_startup);

    // Should have queried tmux during startup for existing projects
    assert!(
        !tmux_startup.is_empty(),
        "Should have executed tmux commands during startup"
    );

    // Should query for both panes and windows
    assert!(
        tmux_startup
            .iter()
            .any(|cmd| cmd.args.iter().any(|arg| arg == "list-panes")),
        "Should list tmux panes"
    );

    assert!(
        tmux_startup
            .iter()
            .any(|cmd| cmd.args.iter().any(|arg| arg == "list-windows")),
        "Should list tmux windows"
    );
}

#[test]
fn test_project_open_with_editor_commands() {
    // Start wormhole in test mode with TestEditor
    let server = TestServer::start(8882);

    // Add a project
    server
        .request("POST", "/add-project/myproject:/tmp/myproject", None, None)
        .unwrap();

    // Clear captured commands
    server.clear_captured_commands();

    // Open the project
    server
        .request("GET", "/open-project/myproject", None, None)
        .unwrap();

    // Wait for async operations
    std::thread::sleep(std::time::Duration::from_millis(200));

    // Get captured commands
    let commands = server.get_captured_commands();

    // Should have test-editor commands (because WORMHOLE_TEST_MODE sets TestEditor)
    let editor_commands: Vec<_> = commands
        .iter()
        .filter(|cmd| cmd.program == "test-editor")
        .collect();

    assert!(!editor_commands.is_empty(), "Should have editor commands");

    // Should open workspace
    assert!(
        editor_commands
            .iter()
            .any(|cmd| cmd.args.contains(&"open-workspace".to_string())),
        "Should open workspace with editor"
    );

    // Should have tmux commands for terminal
    let tmux_commands: Vec<_> = commands
        .iter()
        .filter(|cmd| cmd.program == "tmux")
        .collect();

    println!("Tmux commands: {:?}", tmux_commands);

    // Should interact with tmux to manage windows
    assert!(
        !tmux_commands.is_empty(),
        "Should have tmux commands for terminal management"
    );
}

#[test]
fn test_close_project_commands() {
    let server = TestServer::start(8883);

    // Add a project
    server
        .request("POST", "/add-project/closetest:/tmp/closetest", None, None)
        .unwrap();

    // Clear commands
    server.clear_captured_commands();

    // Close the project
    server
        .request("POST", "/close-project/closetest", None, None)
        .unwrap();

    // Get captured commands
    let commands = server.get_captured_commands();

    // Should have tmux kill commands
    let tmux_commands: Vec<_> = commands
        .iter()
        .filter(|cmd| cmd.program == "tmux")
        .collect();

    assert!(
        tmux_commands
            .iter()
            .any(|cmd| cmd.args.iter().any(|arg| arg.contains("kill"))),
        "Should have tmux kill command when closing project"
    );
}
