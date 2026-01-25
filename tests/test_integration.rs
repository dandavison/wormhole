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

    // Create projects using /project/switch/ endpoint (upsert behavior)
    // Small delay between calls since project opening is async and uses Hammerspoon
    test.hs_get(&format!("/project/switch/{}?name={}", dir_a, proj_a))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.hs_get(&format!("/project/switch/{}?name={}", dir_b, proj_b))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Initially, editor gains focus.
    test.hs_get(&format!("/project/switch/{}", proj_a)).unwrap();
    test.assert_focus(Editor(&proj_a));

    // Switching stays with editor.
    test.hs_get(&format!("/project/switch/{}", proj_b)).unwrap();
    test.assert_focus(Editor(&proj_b));

    // Now focus the terminal.
    test.focus_terminal();

    // Switching now stays with terminal.
    test.hs_get(&format!("/project/switch/{}", proj_a)).unwrap();
    test.assert_focus(Terminal(&proj_a));

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
    let test = harness::WormholeTest::new(8932);

    let proj_a = format!("{}proj-a", TEST_PREFIX);
    let proj_b = format!("{}proj-b", TEST_PREFIX);
    let dir_a = format!("/tmp/{}", proj_a);
    let dir_b = format!("/tmp/{}", proj_b);

    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();

    // Create projects using /project/switch/ endpoint
    // Small delay between calls since project opening is async and uses Hammerspoon
    test.hs_get(&format!("/project/switch/{}?name={}", dir_a, proj_a))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.hs_get(&format!("/project/switch/{}?name={}", dir_b, proj_b))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

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

    // Create project using /project/switch/ endpoint
    test.hs_get(&format!("/project/switch/{}?name={}", dir, proj))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

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

    // Create project using /project/switch/ endpoint
    test.hs_get(&format!("/project/switch/{}?name={}", dir, proj))
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

    // Create project using /project/switch/ endpoint
    test.hs_get(&format!("/project/switch/{}?name={}", dir, proj))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.hs_get(&format!("/file/{}", file)).unwrap();
    test.assert_focus(Editor(&proj));
}

#[test]
fn test_pin() {
    // Test that /pin sets the land-in KV based on current application.
    // The actual effect of land-in on navigation is tested in test_open_project.
    let test = harness::WormholeTest::new(8935);

    let proj = format!("{}pin-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    std::fs::create_dir_all(&dir).unwrap();

    // Create project using /project/switch/ endpoint
    test.hs_get(&format!("/project/switch/{}?name={}", dir, proj))
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Go to project in editor
    test.hs_get(&format!("/project/switch/{}", proj)).unwrap();
    test.assert_focus(Editor(&proj));

    // Pin while in editor - should set land-in=editor
    test.hs_post("/pin").unwrap();
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

    test.hs_post("/pin").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Verify KV was updated
    let kv = test.hs_get(&format!("/kv/{}/land-in", proj)).unwrap();
    assert_eq!(
        kv, "terminal",
        "Expected land-in=terminal after pinning in terminal"
    );
}

#[test]
fn test_task_switch_changes_terminal_cwd() {
    let repo_name = format!("{}task-repo", TEST_PREFIX);
    let repo_dir = format!("/private/tmp/{}", repo_name);
    let worktrees_dir = format!("{}/.tmp/worktrees", repo_dir);
    let task_a = format!("{}task-a", TEST_PREFIX);
    let task_b = format!("{}task-b", TEST_PREFIX);
    let task_a_dir = format!("{}/{}", worktrees_dir, task_a);
    let task_b_dir = format!("{}/{}", worktrees_dir, task_b);

    let _ = std::fs::remove_dir_all(&repo_dir);
    std::fs::create_dir_all(&repo_dir).unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    std::fs::write(format!("{}/README.md", repo_dir), "# Test").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();

    std::fs::create_dir_all(&worktrees_dir).unwrap();
    Command::new("git")
        .args(["worktree", "add", "-b", &task_a, &task_a_dir, "HEAD"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["worktree", "add", "-b", &task_b, &task_b_dir, "HEAD"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();

    let wormhole_path = "/private/tmp";
    let test = harness::WormholeTest::with_wormhole_path(8936, Some(wormhole_path));

    test.hs_get(&format!("/task/switch/{}", task_a)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.assert_tmux_window(&task_a);
    test.assert_tmux_cwd(&task_a_dir);

    test.hs_get(&format!("/task/switch/{}", task_b)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.assert_tmux_window(&task_b);
    test.assert_tmux_cwd(&task_b_dir);

    test.hs_get(&format!("/task/switch/{}", task_a)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.assert_tmux_window(&task_a);
    test.assert_tmux_cwd(&task_a_dir);
}

#[test]
fn test_task_switch_restores_cwd_after_manual_cd() {
    let repo_name = format!("{}task-cd-repo", TEST_PREFIX);
    let repo_dir = format!("/private/tmp/{}", repo_name);
    let worktrees_dir = format!("{}/.tmp/worktrees", repo_dir);
    let task_a = format!("{}task-cd-a", TEST_PREFIX);
    let task_b = format!("{}task-cd-b", TEST_PREFIX);
    let task_a_dir = format!("{}/{}", worktrees_dir, task_a);
    let task_b_dir = format!("{}/{}", worktrees_dir, task_b);

    let _ = std::fs::remove_dir_all(&repo_dir);
    std::fs::create_dir_all(&repo_dir).unwrap();

    Command::new("git")
        .args(["init"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["config", "user.name", "Test"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    std::fs::write(format!("{}/README.md", repo_dir), "# Test").unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "Initial commit"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();

    std::fs::create_dir_all(&worktrees_dir).unwrap();
    Command::new("git")
        .args(["worktree", "add", "-b", &task_a, &task_a_dir, "HEAD"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["worktree", "add", "-b", &task_b, &task_b_dir, "HEAD"])
        .current_dir(&repo_dir)
        .output()
        .unwrap();

    let wormhole_path = "/private/tmp";
    let test = harness::WormholeTest::with_wormhole_path(8937, Some(wormhole_path));

    test.hs_get(&format!("/task/switch/{}", task_a)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.assert_tmux_window(&task_a);
    test.assert_tmux_cwd(&task_a_dir);

    test.tmux_send_keys(&format!("cd {}", task_b_dir));
    std::thread::sleep(std::time::Duration::from_millis(300));
    test.assert_tmux_cwd(&task_b_dir);

    test.hs_get(&format!("/task/switch/{}", task_b)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.assert_tmux_window(&task_b);
    test.assert_tmux_cwd(&task_b_dir);

    test.hs_get(&format!("/task/switch/{}", task_a)).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    test.assert_tmux_window(&task_a);
    test.assert_tmux_cwd(&task_a_dir);
}
