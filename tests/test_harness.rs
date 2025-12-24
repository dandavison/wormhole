mod harness;

#[test]
fn test_close_cursor_window() {
    let test = harness::WormholeTest::new(8950);

    let proj = format!("{}close-test", harness::TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    std::fs::create_dir_all(&dir).ok();

    test.hs_post(&format!("/add-project/{}?name={}", dir, proj))
        .unwrap();
    test.hs_get(&format!("/project/{}", proj)).unwrap();

    assert!(
        test.wait_for_window_containing(&proj, 5),
        "Window should exist after opening project"
    );

    test.close_cursor_window(&proj);

    std::thread::sleep(std::time::Duration::from_millis(500));

    assert!(!test.window_exists(&proj), "Window should be closed");
}
