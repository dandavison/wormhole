mod harness;

#[test]
fn test_close_cursor_window() {
    let test = harness::WormholeTest::new(8950);

    let proj = format!("{}close-test", harness::TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    std::fs::create_dir_all(&dir).unwrap();

    // Create and switch to project using unified /project/ endpoint
    test.hs_get(&format!("/project/{}?name={}", dir, proj))
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
