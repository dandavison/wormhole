mod harness;
use harness::Focus::*;
use harness::TEST_PREFIX;

#[test]
fn test_open_project() {
    // open-project preserves application by default, but respects land-in (from query parameter and
    // from kv).
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
    test.assert_focus(Editor(&proj_a));

    // Switching stays with editor.
    test.hs_get(&format!("/project/{}", proj_b)).unwrap();
    test.assert_focus(Editor(&proj_b));

    // Now focus the terminal.
    test.focus_terminal();

    // Switching now stays with terminal.
    test.hs_get(&format!("/project/{}", proj_a)).unwrap();
    test.assert_focus(Terminal(&proj_a));

    // land-in=editor overrides: even though we're in terminal, we land in editor
    test.hs_get(&format!("/project/{}?land-in=editor", proj_b))
        .unwrap();
    test.assert_focus(Editor(&proj_b));

    // land-in=terminal overrides: even though we're now in editor, we land in terminal
    test.hs_get(&format!("/project/{}?land-in=terminal", proj_a))
        .unwrap();
    test.assert_focus(Terminal(&proj_a));

    // land-in is also respected from project kv store.
    test.hs_put(&format!("/kv/{}/land-in", proj_b), "editor")
        .unwrap();
    test.hs_get(&format!("/project/{}", proj_b)).unwrap();
    test.assert_focus(Editor(&proj_b));
}

#[test]
fn test_previous_project_and_next_project() {
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

    // Start in (a, editor)
    test.hs_get(&format!("/project/{}", proj_a)).unwrap();
    test.assert_focus(Editor(&proj_a));

    // Transition to (b, editor)
    test.hs_get(&format!("/project/{}", proj_b)).unwrap();
    test.assert_focus(Editor(&proj_b));

    for _ in 0..2 {
        // Previous should transition to (a, editor)
        test.hs_get("/previous-project/").unwrap();
        test.assert_focus(Editor(&proj_a));

        // Next should transition to (b, editor)
        test.hs_get("/next-project/").unwrap();
        test.assert_focus(Editor(&proj_b));
    }

    // Transition to (b, terminal)
    test.focus_terminal();
    test.assert_focus(Terminal(&proj_b));

    // Set land-in in kv to check that previous disregards it
    test.hs_put(&format!("/kv/{}/land-in", proj_a), "terminal")
        .unwrap();

    // Previous should transition to (a, editor)
    test.hs_get("/previous-project/").unwrap();
    test.assert_focus(Editor(&proj_a));
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
    test.assert_focus(Editor(&proj));
}

#[test]
fn test_close_project() {
    let test = harness::WormholeTest::new(8933);

    let proj = format!("{}close-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    std::fs::create_dir_all(&dir).unwrap();

    test.hs_post(&format!("/add-project/{}?name={}", dir, proj))
        .unwrap();

    test.hs_get(&format!("/project/{}", proj)).unwrap();
    test.assert_focus(Editor(&proj));

    test.hs_post(&format!("/close-project/{}", proj)).unwrap();

    assert!(
        test.wait_until(|| !test.window_exists(&proj), 5),
        "Editor window should be closed"
    );
}
