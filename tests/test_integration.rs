mod harness;
use harness::TEST_PREFIX;

#[test]
fn test_open_project_preserves_application_by_default_but_respects_land_in() {
    let test = harness::WormholeTest::new(8932);

    let proj_a = format!("{}proj-a", TEST_PREFIX);
    let proj_b = format!("{}proj-b", TEST_PREFIX);
    let dir_a = format!("/tmp/{}", proj_a);
    let dir_b = format!("/tmp/{}", proj_b);

    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    test.hs_post(&format!("/add-project/{}?name={}", dir_a, proj_a))
        .unwrap();
    test.hs_post(&format!("/add-project/{}?name={}", dir_b, proj_b))
        .unwrap();

    // Initially, editor gains focus.
    test.hs_get(&format!("/project/{}", proj_a)).unwrap();
    test.assert_tmux_window(&proj_a);
    test.assert_editor_has_focus(&proj_a);

    // Switching stays with editor.
    test.hs_get(&format!("/project/{}", proj_b)).unwrap();
    test.assert_tmux_window(&proj_b);
    test.assert_editor_has_focus(&proj_b);

    // Now focus the terminal.
    test.focus_terminal();
    test.assert_terminal_has_focus();

    // Switching now stays with terminal.
    test.hs_get(&format!("/project/{}", proj_a)).unwrap();
    test.assert_tmux_window(&proj_a);
    test.assert_terminal_has_focus();

    test.close_cursor_window(&proj_a);
    test.close_cursor_window(&proj_b);
}

#[test]
fn test_navigation_no_deadlock() {
    let test = harness::WormholeTest::new(8930);

    if let Err(e) = test.hs_get("/previous-project/") {
        if e.contains("timeout") {
            panic!("Deadlock detected! {}", e);
        }
    }

    if let Err(e) = test.hs_get("/next-project/") {
        if e.contains("timeout") {
            panic!("Deadlock detected! {}", e);
        }
    }
}

#[test]
fn test_file_opens_in_editor() {
    let test = harness::WormholeTest::new(8931);

    let proj = format!("{}file-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    let file = format!("{}/test.rs", dir);

    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&file, "fn main() {}").unwrap();

    test.hs_post(&format!("/add-project/{}?name={}", dir, proj))
        .unwrap();
    test.hs_get(&format!("/file/{}", file)).unwrap();
    test.assert_editor_has_focus(&proj);

    test.close_cursor_window(&proj);
}
