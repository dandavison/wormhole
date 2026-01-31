mod harness;

use harness::init_git_repo;

#[test]
fn test_close_cursor_window() {
    if std::env::var("WORMHOLE_EDITOR").ok().as_deref() == Some("none") {
        return; // Skip GUI-only test in headless mode
    }

    let test = harness::WormholeTest::new(8950);

    let proj = format!("{}close-test", harness::TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    init_git_repo(&dir);

    // Create and switch to project using unified /project/switch/ endpoint
    test.http_get(&format!("/project/switch/{}?name={}&sync=true", dir, proj))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(
        test.wait_for_window_containing(&proj, 5),
        "Window should exist after opening project"
    );

    test.close_cursor_window(&proj);

    assert!(
        test.wait_until(|| !test.window_exists(&proj), 10),
        "Window should be closed"
    );
}
