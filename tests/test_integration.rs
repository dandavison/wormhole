mod harness;
use harness::Focus::*;
use harness::{init_git_repo, TEST_PREFIX};
use std::path::PathBuf;

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

    // Create projects using unified /project/ endpoint (upsert behavior)
    // Small delay between calls since project opening is async and uses Hammerspoon
    test.hs_get(&format!("/project/{}?name={}", dir_a, proj_a))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.hs_get(&format!("/project/{}?name={}", dir_b, proj_b))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

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

    // Create projects using unified /project/ endpoint
    // Small delay between calls since project opening is async and uses Hammerspoon
    test.hs_get(&format!("/project/{}?name={}", dir_a, proj_a))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.hs_get(&format!("/project/{}?name={}", dir_b, proj_b))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

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
fn test_close_project() {
    let test = harness::WormholeTest::new(8933);

    let proj = format!("{}close-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    std::fs::create_dir_all(&dir).unwrap();

    // Create project using unified /project/ endpoint
    test.hs_get(&format!("/project/{}?name={}", dir, proj))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    test.hs_get(&format!("/project/{}", proj)).unwrap();
    test.assert_focus(Editor(&proj));

    test.hs_post(&format!("/close-project/{}", proj)).unwrap();

    assert!(
        test.wait_until(|| !test.window_exists(&proj), 5),
        "Editor window should be closed"
    );
}

#[test]
fn test_open_github_url() {
    let test = harness::WormholeTest::new(8934);

    let proj = format!("{}github-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    let file = format!("{}/src/main.rs", dir);

    std::fs::create_dir_all(format!("{}/src", dir)).unwrap();
    std::fs::write(&file, "fn main() {}").unwrap();

    // Create project using unified /project/ endpoint
    test.hs_get(&format!("/project/{}?name={}", dir, proj))
        .unwrap();

    // GitHub URL format: /<owner>/<repo>/blob/<branch>/<path>
    // The repo name should match the project name
    test.hs_get(&format!("/owner/{}/blob/main/src/main.rs", proj))
        .unwrap();
    test.assert_focus(Editor(&proj));
}

#[test]
fn test_open_file() {
    let test = harness::WormholeTest::new(8931);

    let proj = format!("{}file-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    let file = format!("{}/test.rs", dir);

    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(&file, "fn main() {}").unwrap();

    // Create project using unified /project/ endpoint
    test.hs_get(&format!("/project/{}?name={}", dir, proj))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.hs_get(&format!("/file/{}", file)).unwrap();
    test.assert_focus(Editor(&proj));
}

#[test]
fn test_pin() {
    // Test that /pin/ sets the land-in KV based on current application.
    // The actual effect of land-in on navigation is tested in test_open_project.
    let test = harness::WormholeTest::new(8935);

    let proj = format!("{}pin-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    std::fs::create_dir_all(&dir).unwrap();

    // Create project using unified /project/ endpoint
    test.hs_get(&format!("/project/{}?name={}", dir, proj))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Go to project in editor
    test.hs_get(&format!("/project/{}", proj)).unwrap();
    test.assert_focus(Editor(&proj));

    // Pin while in editor - should set land-in=editor
    test.hs_post("/pin/").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Verify KV was set
    let kv = test.hs_get(&format!("/kv/{}/land-in", proj)).unwrap();
    assert_eq!(
        kv, "editor",
        "Expected land-in=editor after pinning in editor"
    );

    // Focus terminal and pin again
    test.focus_terminal();
    test.assert_focus(Terminal(&proj));

    test.hs_post("/pin/").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Verify KV was updated
    let kv = test.hs_get(&format!("/kv/{}/land-in", proj)).unwrap();
    assert_eq!(
        kv, "terminal",
        "Expected land-in=terminal after pinning in terminal"
    );
}

#[test]
fn test_task_creation_and_switching() {
    let test = harness::WormholeTest::new(8936);

    let home_repo = format!("{}task-home", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_repo);
    let task_a = format!("{}TASK-001", TEST_PREFIX);
    let task_b = format!("{}TASK-002", TEST_PREFIX);

    std::fs::create_dir_all(&home_dir).unwrap();
    init_git_repo(&PathBuf::from(&home_dir));

    test.hs_get(&format!("/project/{}?name={}", home_dir, home_repo))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    test.hs_get(&format!("/task/{}?home={}", task_a, home_repo))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1000));
    test.assert_focus(Editor(&task_a));

    let worktree_a = format!("{}/.git/wormhole/worktrees/{}", home_dir, task_a);
    assert!(
        PathBuf::from(&worktree_a).exists(),
        "Worktree for task_a should exist at {}",
        worktree_a
    );

    test.hs_get(&format!("/task/{}?home={}", task_b, home_repo))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(1000));
    test.assert_focus(Editor(&task_b));

    let worktree_b = format!("{}/.git/wormhole/worktrees/{}", home_dir, task_b);
    assert!(
        PathBuf::from(&worktree_b).exists(),
        "Worktree for task_b should exist at {}",
        worktree_b
    );

    // Switch to existing task (no home param needed)
    test.hs_get(&format!("/task/{}", task_a)).unwrap();
    test.assert_focus(Editor(&task_a));

    // Explicit land-in=editor works
    test.hs_get(&format!("/task/{}?land-in=editor", task_b))
        .unwrap();
    test.assert_focus(Editor(&task_b));

    // Explicit land-in=terminal works
    test.hs_get(&format!("/task/{}?land-in=terminal", task_a))
        .unwrap();
    test.assert_focus(Terminal(&task_a));

    // Switching tasks with explicit land-in preserves as expected
    test.hs_get(&format!("/task/{}?land-in=terminal", task_b))
        .unwrap();
    test.assert_focus(Terminal(&task_b));
}

#[test]
fn test_task_tmux_pwd() {
    let test = harness::WormholeTest::new(8937);

    let home_repo = format!("{}task-pwd-home", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_repo);
    let task_id = format!("{}PWD-001", TEST_PREFIX);

    std::fs::create_dir_all(&home_dir).unwrap();
    init_git_repo(&PathBuf::from(&home_dir));

    test.hs_get(&format!("/project/{}?name={}", home_dir, home_repo))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    test.hs_get(&format!("/task/{}?home={}", task_id, home_repo))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2000));

    test.assert_focus(Editor(&task_id));

    let expected_pwd = format!("/private{}/.git/wormhole/worktrees/{}", home_dir, task_id);
    test.assert_tmux_pwd(&task_id, &expected_pwd);
}

#[test]
fn test_project_tmux_pwd() {
    let test = harness::WormholeTest::new(8938);

    let proj = format!("{}pwd-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    std::fs::create_dir_all(&dir).unwrap();

    test.hs_get(&format!("/project/{}?name={}", dir, proj))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    test.hs_get(&format!("/project/{}", proj)).unwrap();
    test.assert_focus(Editor(&proj));

    let expected_dir = format!("/private{}", dir);
    test.assert_tmux_pwd(&proj, &expected_dir);
}
