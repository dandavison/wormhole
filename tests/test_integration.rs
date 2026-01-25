mod harness;
use harness::Focus::*;
use harness::TEST_PREFIX;
use std::process::Command;

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

    test.create_project(&dir_a, &proj_a);
    test.create_project(&dir_b, &proj_b);

    // Initially, editor gains focus.
    test.hs_get(&format!("/project/switch/{}", proj_a)).unwrap();
    test.assert_focus(Editor(&proj_a));
    test.assert_tmux_cwd(&dir_a);

    // Switching stays with editor.
    test.hs_get(&format!("/project/switch/{}", proj_b)).unwrap();
    test.assert_focus(Editor(&proj_b));
    test.assert_tmux_cwd(&dir_b);

    // Now focus the terminal.
    test.focus_terminal();

    // Switching now stays with terminal.
    test.hs_get(&format!("/project/switch/{}", proj_a)).unwrap();
    test.assert_focus(Terminal(&proj_a));
    test.assert_tmux_cwd(&dir_a);

    // land-in=editor overrides: even though we're in terminal, we land in editor
    test.hs_get(&format!("/project/switch/{}?land-in=editor", proj_b))
        .unwrap();
    test.assert_focus(Editor(&proj_b));

    // land-in=terminal overrides: even though we're now in editor, we land in terminal
    test.hs_get(&format!("/project/switch/{}?land-in=terminal", proj_a))
        .unwrap();
    test.assert_focus(Terminal(&proj_a));

    // land-in is also respected from project kv store.
    test.hs_put(&format!("/kv/{}/land-in", proj_b), "editor")
        .unwrap();
    test.hs_get(&format!("/project/switch/{}", proj_b)).unwrap();
    test.assert_focus(Editor(&proj_b));
}

#[test]
fn test_previous_project_and_next_project() {
    let test = harness::WormholeTest::new(8936);

    let proj_a = format!("{}proj-a", TEST_PREFIX);
    let proj_b = format!("{}proj-b", TEST_PREFIX);
    let dir_a = format!("/tmp/{}", proj_a);
    let dir_b = format!("/tmp/{}", proj_b);

    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    test.create_project(&dir_a, &proj_a);
    test.create_project(&dir_b, &proj_b);

    // Start in (a, editor)
    test.hs_get(&format!("/project/switch/{}", proj_a)).unwrap();
    test.assert_focus(Editor(&proj_a));

    // Transition to (b, editor)
    test.hs_get(&format!("/project/switch/{}", proj_b)).unwrap();
    test.assert_focus(Editor(&proj_b));

    for _ in 0..2 {
        // Previous should transition to (a, editor)
        test.hs_get("/project/previous").unwrap();
        test.assert_focus(Editor(&proj_a));

        // Next should transition to (b, editor)
        test.hs_get("/project/next").unwrap();
        test.assert_focus(Editor(&proj_b));
    }

    // Transition to (b, terminal)
    test.focus_terminal();
    test.assert_focus(Terminal(&proj_b));

    // Set land-in in kv to check that previous disregards it
    test.hs_put(&format!("/kv/{}/land-in", proj_a), "terminal")
        .unwrap();

    // Previous should transition to (a, editor)
    test.hs_get("/project/previous").unwrap();
    test.assert_focus(Editor(&proj_a));
}

#[test]
fn test_close_project() {
    let test = harness::WormholeTest::new(8933);

    let proj = format!("{}close-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    std::fs::create_dir_all(&dir).unwrap();

    test.create_project(&dir, &proj);
    test.hs_get(&format!("/project/switch/{}", proj)).unwrap();
    test.assert_focus(Editor(&proj));

    test.hs_post(&format!("/project/close/{}", proj)).unwrap();

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

    test.create_project(&dir, &proj);

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

    test.create_project(&dir, &proj);
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

    test.create_project(&dir, &proj);

    // Go to project in editor
    test.hs_get(&format!("/project/switch/{}", proj)).unwrap();
    test.assert_focus(Editor(&proj));

    // Pin while in editor - should set land-in=editor
    test.hs_post("/project/pin").unwrap();
    assert!(
        test.wait_for_kv(&proj, "land-in", "editor", 10),
        "Expected land-in=editor after pinning in editor"
    );

    // Focus terminal and pin again
    test.focus_terminal();
    test.assert_focus(Terminal(&proj));

    test.hs_post("/project/pin").unwrap();
    assert!(
        test.wait_for_kv(&proj, "land-in", "terminal", 5),
        "Expected land-in=terminal after pinning in terminal"
    );
}

#[test]
fn test_task_switching() {
    let test = harness::WormholeTest::new(8937);

    let home_proj = format!("{}task-home", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_1 = format!("{}TASK-1", TEST_PREFIX);
    let task_2 = format!("{}TASK-2", TEST_PREFIX);

    // Create home project as a git repo
    let _ = std::fs::remove_dir_all(&home_dir);
    std::fs::create_dir_all(&home_dir).unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(&home_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(&home_dir)
        .output()
        .unwrap();

    // Register home project with wormhole
    test.create_project(&home_dir, &home_proj);

    // Create two tasks based on the home project
    test.create_task(&task_1, &home_proj);
    test.create_task(&task_2, &home_proj);

    let task_1_dir = format!("{}/.git/wormhole/worktrees/{}", home_dir, task_1);
    let task_2_dir = format!("{}/.git/wormhole/worktrees/{}", home_dir, task_2);

    // Switch to home project
    test.hs_get(&format!("/project/switch/{}", home_proj)).unwrap();
    test.assert_focus(Editor(&home_proj));
    test.assert_tmux_cwd(&home_dir);

    // Switch to task 1
    test.hs_get(&format!("/project/switch/{}", task_1)).unwrap();
    test.assert_focus(Editor(&task_1));
    test.assert_tmux_cwd(&task_1_dir);

    // Switch to task 2
    test.hs_get(&format!("/project/switch/{}", task_2)).unwrap();
    test.assert_focus(Editor(&task_2));
    test.assert_tmux_cwd(&task_2_dir);

    // Switch back to home project
    test.hs_get(&format!("/project/switch/{}", home_proj)).unwrap();
    test.assert_focus(Editor(&home_proj));
    test.assert_tmux_cwd(&home_dir);

    // Switch to task 1 again
    test.hs_get(&format!("/project/switch/{}", task_1)).unwrap();
    test.assert_focus(Editor(&task_1));
    test.assert_tmux_cwd(&task_1_dir);
}

#[test]
fn test_project_status() {
    let test = harness::WormholeTest::new(8938);

    let proj = format!("{}status-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(format!("{}/plan.md", dir), "# Plan").unwrap();

    test.create_project(&dir, &proj);
    test.hs_get(&format!("/project/switch/{}", proj)).unwrap();

    // Get status by name
    let status = test.hs_get(&format!("/project/status/{}", proj)).unwrap();
    assert!(status.contains(&proj), "Status should contain project name");
    assert!(status.contains("plan.md"), "Status should mention plan.md");

    // Get current project status
    let status = test.hs_get("/project/status").unwrap();
    assert!(status.contains(&proj), "Current status should contain project name");

    // Get JSON format
    let status = test
        .hs_get(&format!("/project/status/{}?format=json", proj))
        .unwrap();
    assert!(status.contains("\"name\""), "JSON should have name field");
    assert!(
        status.contains("\"plan_exists\": true"),
        "JSON should show plan_exists true, got: {}",
        status
    );
}
