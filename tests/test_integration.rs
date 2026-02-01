mod harness;
use harness::{init_git_repo, Focus::*, TEST_PREFIX};
use serde_json::Value;
use std::process::Command;

fn editor_is_none() -> bool {
    std::env::var("WORMHOLE_EDITOR").ok().as_deref() == Some("none")
}

#[test]
fn test_open_project() {
    // open-project preserves application by default, but respects land-in (from query parameter and
    // from kv).
    let test = harness::WormholeTest::new(8932);

    let proj_a = format!("{}proj-a", TEST_PREFIX);
    let proj_b = format!("{}proj-b", TEST_PREFIX);
    let dir_a = format!("/tmp/{}", proj_a);
    let dir_b = format!("/tmp/{}", proj_b);

    init_git_repo(&dir_a);
    init_git_repo(&dir_b);

    test.create_project(&dir_a, &proj_a);
    test.create_project(&dir_b, &proj_b);

    // Initially, editor gains focus.
    test.cli(&format!("wormhole open {}", proj_a)).unwrap();
    test.assert_focus(Editor(&proj_a));
    test.assert_tmux_cwd(&dir_a);

    // Switching stays with editor.
    test.cli(&format!("wormhole open {}", proj_b)).unwrap();
    test.assert_focus(Editor(&proj_b));
    test.assert_tmux_cwd(&dir_b);

    // Now focus the terminal.
    test.focus_terminal();

    // Switching now stays with terminal.
    test.cli(&format!("wormhole open {}", proj_a)).unwrap();
    test.assert_focus(Terminal(&proj_a));
    test.assert_tmux_cwd(&dir_a);

    // land-in=editor overrides: even though we're in terminal, we land in editor
    test.cli(&format!("wormhole open {} --land-in editor", proj_b))
        .unwrap();
    test.assert_focus(Editor(&proj_b));

    // land-in=terminal overrides: even though we're now in editor, we land in terminal
    test.cli(&format!("wormhole open {} --land-in terminal", proj_a))
        .unwrap();
    test.assert_focus(Terminal(&proj_a));

    // land-in is also respected from project kv store.
    test.cli(&format!("wormhole kv set {} land-in editor", proj_b))
        .unwrap();
    test.cli(&format!("wormhole open {}", proj_b)).unwrap();
    test.assert_focus(Editor(&proj_b));
}

#[test]
fn test_open_fallback_without_subcommand() {
    // `wormhole <target>` should behave the same as `wormhole open <target>`
    let test = harness::WormholeTest::new(8960);

    let proj = format!("{}fallback", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    init_git_repo(&dir);
    test.create_project(&dir, &proj);

    // `wormhole <project-name>` without explicit `open` subcommand
    test.cli(&format!("wormhole {}", proj)).unwrap();
    test.assert_focus(Editor(&proj));
    test.assert_tmux_cwd(&dir);

    // Also works with path
    test.cli(&format!("wormhole {}", dir)).unwrap();
    test.assert_focus(Editor(&proj));
}

#[test]
fn test_previous_project_and_next_project() {
    let test = harness::WormholeTest::new(8936);

    let proj_a = format!("{}proj-a", TEST_PREFIX);
    let proj_b = format!("{}proj-b", TEST_PREFIX);
    let dir_a = format!("/tmp/{}", proj_a);
    let dir_b = format!("/tmp/{}", proj_b);

    init_git_repo(&dir_a);
    init_git_repo(&dir_b);

    test.create_project(&dir_a, &proj_a);
    test.create_project(&dir_b, &proj_b);

    // Start in (a, editor)
    test.cli(&format!("wormhole open {}", proj_a)).unwrap();
    test.assert_focus(Editor(&proj_a));

    // Transition to (b, editor)
    test.cli(&format!("wormhole open {}", proj_b)).unwrap();
    test.assert_focus(Editor(&proj_b));

    for _ in 0..2 {
        // Previous should transition to (a, editor)
        test.cli("wormhole project previous").unwrap();
        test.assert_focus(Editor(&proj_a));

        // Next should transition to (b, editor)
        test.cli("wormhole project next").unwrap();
        test.assert_focus(Editor(&proj_b));
    }

    // Transition to (b, terminal)
    test.focus_terminal();
    test.assert_focus(Terminal(&proj_b));

    // Set land-in in kv to check that previous disregards it
    test.cli(&format!("wormhole kv set {} land-in terminal", proj_a))
        .unwrap();

    // Previous should transition to (a, editor)
    test.cli("wormhole project previous").unwrap();
    test.assert_focus(Editor(&proj_a));
}

#[test]
fn test_close_project() {
    let test = harness::WormholeTest::new(8933);

    let proj = format!("{}close-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    init_git_repo(&dir);

    test.create_project(&dir, &proj);
    test.cli(&format!("wormhole open {}", proj)).unwrap();
    test.assert_focus(Editor(&proj));

    test.cli(&format!("wormhole project close {}", proj))
        .unwrap();

    assert!(
        test.wait_until(|| !test.window_exists(&proj), 5),
        "Editor window should be closed"
    );
}

#[test]
fn test_project_list_sorted() {
    let test = harness::WormholeTest::new(8944);

    let proj_b = format!("{}sort-beta", TEST_PREFIX);
    let proj_a = format!("{}sort-alpha", TEST_PREFIX);
    let dir_b = format!("/tmp/{}", proj_b);
    let dir_a = format!("/tmp/{}", proj_a);
    let task_b1 = format!("{}SORT-B1", TEST_PREFIX);
    let task_a1 = format!("{}SORT-A1", TEST_PREFIX);

    // Create projects in reverse order
    for (dir, proj) in [(&dir_b, &proj_b), (&dir_a, &proj_a)] {
        init_git_repo(dir);
        test.create_project(dir, proj);
    }

    // Create tasks - task_id is now the branch name
    test.create_task(&task_b1, &proj_b);
    test.create_task(&task_a1, &proj_a);

    // Open all in reverse alphabetical order using store_key format for tasks
    let task_b1_key = test.task_store_key(&task_b1, &proj_b);
    let task_a1_key = test.task_store_key(&task_a1, &proj_a);
    test.cli(&format!("wormhole open {}", task_b1_key)).unwrap();
    test.cli(&format!("wormhole open {}", proj_b)).unwrap();
    test.cli(&format!("wormhole open {}", task_a1_key)).unwrap();
    test.cli(&format!("wormhole open {}", proj_a)).unwrap();

    // Get project list via curl (Hammerspoon can timeout with many rapid calls)
    let output = Command::new("curl")
        .args(["-s", "http://127.0.0.1:8944/project/list"])
        .output()
        .expect("curl failed");
    let list_json = String::from_utf8_lossy(&output.stdout);
    let list: Value = serde_json::from_str(&list_json).expect("invalid JSON from /project/list");
    let current = list["current"].as_array().expect("missing 'current' array");

    // Extract project keys, filtering to test projects only
    let identifiers: Vec<String> = current
        .iter()
        .filter_map(|e| {
            let project_key = e["project_key"].as_str()?;
            if project_key.starts_with(TEST_PREFIX) {
                Some(project_key.to_string())
            } else {
                None
            }
        })
        .collect();

    // Projects without branch first (alphabetically), then tasks (by name, then branch)
    let expected: Vec<String> = vec![
        proj_a.clone(),
        proj_b.clone(),
        task_a1_key.clone(),
        task_b1_key.clone(),
    ];
    assert_eq!(
        identifiers, expected,
        "Expected sorted order: projects first alphabetically, then tasks by (name, branch)"
    );
}

#[test]
fn test_close_task_removes_from_list() {
    let test = harness::WormholeTest::new(8943);

    let home_proj = format!("{}close-home", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_id = format!("{}CLOSE-TASK", TEST_PREFIX);

    init_git_repo(&home_dir);

    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_id, &home_proj);

    // Task is now identified by (repo, branch). The store_key is "repo:branch".
    let store_key = test.task_store_key(&task_id, &home_proj);

    // Switch to task so it appears in project list (already switched in create_task)
    // Cursor window title shows the folder name (branch), not the store_key
    test.assert_focus(Editor(&task_id));

    // Verify task is in project list (check name == repo AND branch == task_id)
    assert!(
        test.task_in_list(&home_proj, &task_id),
        "Task should be in list before close"
    );

    // Close the task using store_key format
    test.cli(&format!("wormhole project close '{}'", store_key))
        .unwrap();

    // Wait for window to close (window name is store_key)
    assert!(
        test.wait_until(|| !test.window_exists(&store_key), 5),
        "Task window should be closed"
    );

    // Verify task is NOT in project list
    assert!(
        !test.task_in_list(&home_proj, &task_id),
        "Task should NOT be in list after close"
    );
}

#[test]
fn test_open_github_url() {
    let test = harness::WormholeTest::new(8934);

    let proj = format!("{}github-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    let file = format!("{}/src/main.rs", dir);

    init_git_repo(&dir);
    std::fs::create_dir_all(format!("{}/src", dir)).unwrap();
    std::fs::write(&file, "fn main() {}").unwrap();

    test.create_project(&dir, &proj);

    // GitHub URL format: /<owner>/<repo>/blob/<branch>/<path>
    // The repo name should match the project name
    test.http_get(&format!("/owner/{}/blob/main/src/main.rs", proj))
        .unwrap();
    test.assert_focus(Editor(&proj));
}

#[test]
fn test_open_file() {
    let test = harness::WormholeTest::new(8931);

    let proj = format!("{}file-proj2", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    let file = format!("{}/test.rs", dir);

    init_git_repo(&dir);
    std::fs::write(&file, "fn main() {}").unwrap();

    test.create_project(&dir, &proj);
    test.cli(&format!("wormhole open {}", file)).unwrap();
    test.assert_focus(Editor(&proj));
}

#[test]
fn test_open_file_with_line_number() {
    let test = harness::WormholeTest::new(8955);

    let proj = format!("{}file-line", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    let file = format!("{}/test.rs", dir);

    init_git_repo(&dir);
    std::fs::write(&file, "line1\nline2\nline3\n").unwrap();

    test.create_project(&dir, &proj);
    // Open file with line number using CLI path:line syntax
    test.cli(&format!("wormhole open {}:3", file)).unwrap();
    test.assert_focus(Editor(&proj));
}

#[test]
fn test_pin() {
    // Test that /pin/ sets the land-in KV based on current application.
    // The actual effect of land-in on navigation is tested in test_open_project.
    if editor_is_none() {
        return; // Pin relies on detecting focused GUI app
    }
    let test = harness::WormholeTest::new(8935);

    let proj = format!("{}pin-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    init_git_repo(&dir);

    test.create_project(&dir, &proj);

    // Go to project in editor
    test.cli(&format!("wormhole open {}", proj)).unwrap();
    test.assert_focus(Editor(&proj));

    // Pin while in editor - should set land-in=editor
    test.cli("wormhole project pin").unwrap();
    assert!(
        test.wait_for_kv(&proj, "land-in", "editor", 10),
        "Expected land-in=editor after pinning in editor"
    );

    // Wait for the pin alert animation to finish (0.5s) before changing focus
    std::thread::sleep(std::time::Duration::from_millis(600));

    // Focus terminal and pin again
    test.focus_terminal();
    test.assert_focus(Terminal(&proj));

    test.cli("wormhole project pin").unwrap();
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

    init_git_repo(&home_dir);

    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_1, &home_proj);
    test.create_task(&task_2, &home_proj);

    let task_1_key = test.task_store_key(&task_1, &home_proj);
    let task_2_key = test.task_store_key(&task_2, &home_proj);
    let task_1_dir = format!("{}/.git/wormhole/worktrees/{}", home_dir, task_1);
    let task_2_dir = format!("{}/.git/wormhole/worktrees/{}", home_dir, task_2);

    // Table-driven: (switch_key, window_title, expected_cwd)
    let cases = [
        (&home_proj, &home_proj, &home_dir),
        (&task_1_key, &task_1, &task_1_dir),
        (&task_2_key, &task_2, &task_2_dir),
        (&home_proj, &home_proj, &home_dir),
        (&task_1_key, &task_1, &task_1_dir),
    ];

    for (switch_key, window_title, expected_cwd) in cases {
        test.cli(&format!("wormhole open '{}'", switch_key))
            .unwrap();
        test.assert_focus(Editor(window_title));
        test.assert_tmux_cwd(expected_cwd);
    }
}

#[test]
fn test_task_in_submodule() {
    use std::process::Command;

    let test = harness::WormholeTest::new(8939);

    let parent_name = format!("{}submod-parent", TEST_PREFIX);
    let parent_dir = format!("/tmp/{}", parent_name);
    let child_src = format!("/tmp/{}submod-src", TEST_PREFIX);
    // Use a name matching the submodule directory for window title matching
    let submodule_name = format!("{}submod-child", TEST_PREFIX);
    let submodule_dirname = submodule_name.clone();
    let submodule_dir = format!("{}/{}", parent_dir, submodule_dirname);
    let task_id = format!("{}SUB-TASK", TEST_PREFIX);

    // Create source repo (will become submodule)
    init_git_repo(&child_src);

    // Create parent repo
    init_git_repo(&parent_dir);

    // Add as submodule with name matching our project name
    Command::new("git")
        .args(["submodule", "add", &child_src, &submodule_dirname])
        .current_dir(&parent_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add submodule"])
        .current_dir(&parent_dir)
        .output()
        .unwrap();

    // Register submodule as a wormhole project
    test.create_project(&submodule_dir, &submodule_name);

    // Create a task in the submodule
    test.create_task(&task_id, &submodule_name);

    // Worktree should be in parent's .git/modules/<submodule>/wormhole/worktrees/
    let task_dir = format!(
        "{}/.git/modules/{}/wormhole/worktrees/{}",
        parent_dir, submodule_dirname, task_id
    );

    // Switch between submodule project and task
    test.cli(&format!("wormhole open {}", submodule_name))
        .unwrap();
    test.assert_focus(Editor(&submodule_name));
    test.assert_tmux_cwd(&submodule_dir);

    // Switch to task using store_key format
    // Cursor window title shows the folder name (branch), not the store_key
    let store_key = test.task_store_key(&task_id, &submodule_name);
    test.cli(&format!("wormhole open '{}'", store_key)).unwrap();
    test.assert_focus(Editor(&task_id));
    test.assert_tmux_cwd(&task_dir);
}

#[test]
fn test_task_home_project_not_self() {
    let test = harness::WormholeTest::new(8940);

    let home_proj = format!("{}home-proj", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_id = format!("{}TASK-SELF", TEST_PREFIX);

    init_git_repo(&home_dir);

    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_id, &home_proj);

    // Switch to task so it's in the open projects list
    let store_key = test.task_store_key(&task_id, &home_proj);
    test.cli(&format!("wormhole open '{}'", store_key)).unwrap();
    // Cursor window title shows the folder name (branch), not the store_key
    test.assert_focus(Editor(&task_id));

    // Verify task appears with name=repo and branch=task_id
    assert!(
        test.task_in_list(&home_proj, &task_id),
        "Task should be in current list with name={} and branch={}",
        home_proj,
        task_id
    );
}

#[test]
fn test_task_switching_updates_ring_order() {
    // Verify that switching to a task updates the ring for navigation.
    // The project list is sorted, but previous/next commands use the ring order.
    let test = harness::WormholeTest::new(8941);

    let home_proj = format!("{}ring-home", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_id = format!("{}RING-TASK", TEST_PREFIX);

    init_git_repo(&home_dir);

    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_id, &home_proj);

    // Task store key for the new model
    let store_key = test.task_store_key(&task_id, &home_proj);

    // Switch to home project first
    test.cli(&format!("wormhole open {}", home_proj)).unwrap();
    test.assert_focus(Editor(&home_proj));

    // Switch to task using store_key
    // Cursor window title shows the folder name (branch), not the store_key
    test.cli(&format!("wormhole open '{}'", store_key)).unwrap();
    test.assert_focus(Editor(&task_id));

    // Verify both are in the list
    assert!(
        test.project_in_list(&home_proj),
        "Home project should be in list"
    );
    assert!(
        test.task_in_list(&home_proj, &task_id),
        "Task should be in list"
    );

    // Toggle back via previous - should go to home project
    test.cli("wormhole project previous").unwrap();
    test.assert_focus(Editor(&home_proj));

    // Toggle forward via next - should go to task
    test.cli("wormhole project next").unwrap();
    test.assert_focus(Editor(&task_id));
}

#[test]
fn test_project_status() {
    let test = harness::WormholeTest::new(8938);

    let proj = format!("{}status-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    init_git_repo(&dir);
    std::fs::create_dir_all(format!("{}/.task", dir)).unwrap();
    std::fs::write(format!("{}/.task/plan.md", dir), "# Plan").unwrap();

    test.create_project(&dir, &proj);
    test.assert_focus(Editor(&proj));

    // Get info by name
    let status = test
        .cli(&format!("wormhole project show {}", proj))
        .unwrap();
    assert!(status.contains(&proj), "Status should contain project name");

    // Get current project info (via HTTP - CLI uses cwd which isn't the test project)
    let status = test.http_get("/project/show").unwrap();
    assert!(
        status.contains(&proj),
        "Current status should contain project name"
    );

    // Get JSON format
    let status = test
        .cli(&format!("wormhole project show {} -o json", proj))
        .unwrap();
    assert!(status.contains("\"name\""), "JSON should have name field");
    assert!(
        status.contains("\"plan_exists\": true"),
        "JSON should show plan_exists true, got: {}",
        status
    );
}

#[test]
fn test_task_respects_land_in_kv() {
    // Tests that switching to a task respects the land-in KV value.
    // This is a regression test: open_task() was not reading the KV.
    let test = harness::WormholeTest::new(8942);

    let home_proj = format!("{}kv-task-home", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_id = format!("{}KV-TASK", TEST_PREFIX);

    init_git_repo(&home_dir);

    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_id, &home_proj);

    // Store key is now "repo:branch"
    let store_key = test.task_store_key(&task_id, &home_proj);

    // Directly set land-in=terminal for the task using store_key
    test.cli(&format!("wormhole kv set '{}' land-in terminal", store_key))
        .unwrap();

    // Switch to home project first (so we're not on the task)
    test.cli(&format!("wormhole open {}", home_proj)).unwrap();
    test.assert_focus(Editor(&home_proj));

    // Switch to task - should respect land-in=terminal
    test.cli(&format!("wormhole open '{}'", store_key)).unwrap();
    test.assert_focus(Terminal(&store_key));
}

#[test]
fn test_tasks_persist_after_tmux_window_closed() {
    // Tasks should remain in project list even after their tmux window is closed.
    // This allows users to see all their tasks after restarting tmux or wormhole.
    let test = harness::WormholeTest::new(8945);

    let home_proj = format!("{}list-home", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_1 = format!("{}LIST-T1", TEST_PREFIX);
    let task_2 = format!("{}LIST-T2", TEST_PREFIX);

    init_git_repo(&home_dir);

    // Create home project and two tasks
    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_1, &home_proj);
    test.create_task(&task_2, &home_proj);

    let task_1_key = test.task_store_key(&task_1, &home_proj);
    let task_2_key = test.task_store_key(&task_2, &home_proj);

    // Switch to both tasks to ensure they're in the ring and have tmux windows
    test.cli(&format!("wormhole open '{}'", task_1_key))
        .unwrap();
    test.cli(&format!("wormhole open '{}'", task_2_key))
        .unwrap();

    // Verify both tasks are in the project list
    assert!(
        test.task_in_list(&home_proj, &task_1),
        "Task 1 should be in list initially"
    );
    assert!(
        test.task_in_list(&home_proj, &task_2),
        "Task 2 should be in list initially"
    );

    // Kill task 1's tmux window directly (bypassing wormhole's close_project)
    test.kill_tmux_window(&task_1_key);

    // Wait for window to be gone
    assert!(
        test.wait_until(|| !test.tmux_window_exists(&task_1_key), 5),
        "Task 1 tmux window should be closed"
    );

    // Both tasks should STILL be in the list (tasks persist regardless of tmux windows)
    assert!(
        test.task_in_list(&home_proj, &task_1),
        "Task 1 should STILL be in list after tmux window closed"
    );
    assert!(
        test.task_in_list(&home_proj, &task_2),
        "Task 2 should still be in list"
    );
}

#[test]
fn test_neighbors_returns_project_key() {
    // Verify /project/neighbors returns project_key for each item in the ring.
    // Tasks have project_key containing ':', regular projects don't.
    let test = harness::WormholeTest::new(8946);

    let home_proj = format!("{}neighbors-home", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_1 = format!("{}NBR-T1", TEST_PREFIX);
    let task_2 = format!("{}NBR-T2", TEST_PREFIX);

    init_git_repo(&home_dir);

    // Create home project and two tasks from the same repo
    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_1, &home_proj);
    test.create_task(&task_2, &home_proj);

    let task_1_key = test.task_store_key(&task_1, &home_proj);
    let task_2_key = test.task_store_key(&task_2, &home_proj);

    // Switch to both tasks to add them to the ring
    test.cli(&format!("wormhole open '{}'", task_1_key)).unwrap();
    test.cli(&format!("wormhole open '{}'", task_2_key)).unwrap();

    // Get neighbors endpoint
    let neighbors_json = test.http_get("/project/neighbors").unwrap();
    let neighbors: Value = serde_json::from_str(&neighbors_json).unwrap();
    let ring = neighbors["ring"]
        .as_array()
        .expect("ring should be an array");

    // Collect all project_keys
    let keys: Vec<&str> = ring
        .iter()
        .filter_map(|e| e["project_key"].as_str())
        .collect();

    // Verify tasks are in ring with colon-separated keys
    assert!(
        keys.contains(&task_1_key.as_str()),
        "Task 1 key '{}' should be in ring, got: {:?}",
        task_1_key,
        keys
    );
    assert!(
        keys.contains(&task_2_key.as_str()),
        "Task 2 key '{}' should be in ring, got: {:?}",
        task_2_key,
        keys
    );

    // Verify regular project is in ring without colon
    assert!(
        keys.contains(&home_proj.as_str()),
        "Project '{}' should be in ring, got: {:?}",
        home_proj,
        keys
    );
}

#[test]
fn test_tasks_appear_without_terminal_windows() {
    // Tasks should appear in project list even without active terminal windows.
    // This test creates worktrees directly with git (not via create_task which opens terminals),
    // then verifies they appear in project list after refresh.
    use std::process::Command;

    let test = harness::WormholeTest::new(8951);

    let home_proj = format!("{}tasks-no-term", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_branch = format!("{}task-branch", TEST_PREFIX);

    // Clean up and create home directory
    init_git_repo(&home_dir);

    // Register home project (this creates a terminal window for the project)
    test.create_project(&home_dir, &home_proj);

    // Create worktree directory and worktree using git directly
    // (NOT via create_task, so no terminal window is created for the task)
    let worktrees_dir = format!("{}/.git/wormhole/worktrees", home_dir);
    std::fs::create_dir_all(&worktrees_dir).unwrap();
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            &task_branch,
            &format!("{}/{}", worktrees_dir, task_branch),
        ])
        .current_dir(&home_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Failed to create worktree: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify the worktree was created
    let worktree_path = format!("{}/{}", worktrees_dir, task_branch);
    assert!(
        std::path::Path::new(&worktree_path).exists(),
        "Worktree directory should exist"
    );

    // Refresh to discover the new task
    test.cli("wormhole refresh").unwrap();

    // Give a moment for refresh to complete
    std::thread::sleep(std::time::Duration::from_millis(500));

    // The task should appear in the list even though it has no terminal window
    assert!(
        test.task_in_list(&home_proj, &task_branch),
        "Task '{}:{}' should appear in project list even without terminal window",
        home_proj,
        task_branch
    );
}

#[test]
fn test_switch_to_project_when_task_exists() {
    // Regression test: switching to a project by name should open the PROJECT
    // directory, not a task directory, even when both exist for the same repo.
    //
    // Bug: resolve_project() added the project correctly but then called
    // by_exact_path() to retrieve it. Since project and task share the same
    // repo_path, by_exact_path()'s find() could return the task instead.
    //
    // Fix: use by_key() to retrieve the project by its key after adding it.
    use std::process::Command;

    let test = harness::WormholeTest::new(8953);

    let home_proj = format!("{}switch-proj", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_branch = format!("{}task-exists", TEST_PREFIX);

    // Clean up and create home directory
    init_git_repo(&home_dir);

    // Register the project first (so wormhole knows about this repo)
    test.create_project(&home_dir, &home_proj);

    // Create worktree directly with git (not via create_task)
    let worktrees_dir = format!("{}/.git/wormhole/worktrees", home_dir);
    std::fs::create_dir_all(&worktrees_dir).unwrap();
    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            &task_branch,
            &format!("{}/{}", worktrees_dir, task_branch),
        ])
        .current_dir(&home_dir)
        .output()
        .unwrap();

    // Refresh to discover the task (use refresh-tasks to skip slow JIRA/GitHub cache)
    test.http_post("/project/refresh-tasks").unwrap();

    // Verify task is discovered
    assert!(
        test.task_in_list(&home_proj, &task_branch),
        "Task should be discovered after refresh"
    );

    // Switch to the task first (so it's the most recent)
    let task_key = test.task_store_key(&task_branch, &home_proj);
    test.cli(&format!("wormhole open '{}'", task_key)).unwrap();
    let task_dir = format!("{}/{}", worktrees_dir, task_branch);
    test.assert_tmux_cwd(&task_dir);

    // Now switch to the PROJECT by name (not the task)
    // The bug would cause this to stay in task dir or return wrong project
    test.cli(&format!("wormhole open {}", home_proj)).unwrap();

    // Verify we're in the PROJECT directory, not the task worktree
    test.assert_tmux_cwd(&home_dir);

    // Also verify the project (not task) is in the list
    assert!(
        test.project_in_list(&home_proj),
        "Project '{}' (without branch) should be in list after switching to it",
        home_proj
    );

    // PART 2: Test switching by absolute PATH (not name)
    // This exercises a different code path in resolve_project that also had the bug.

    // Switch to task first
    test.cli(&format!("wormhole open '{}'", task_key)).unwrap();
    test.assert_tmux_cwd(&task_dir);

    // Now switch by absolute path to the project directory
    test.cli(&format!("wormhole open {}", home_dir)).unwrap();

    // Should be in project dir, not task dir
    test.assert_tmux_cwd(&home_dir);
}

#[test]
fn test_file_opens_in_project_not_task() {
    // Regression test: /file/ endpoint should open files in the correct project.
    // When both a project and task exist for the same repo, a file in the main
    // repo directory should open in the project, not the task.
    //
    // Bug: by_path() used max_by_key with worktree_path length to pick the
    // "most specific" match. Since project and task share the same repo_path,
    // both matched. The task's worktree_path was longer, so it incorrectly won.
    //
    // Fix: Use the length of the path that actually matched the query, not
    // the theoretical working directory.
    use std::process::Command;

    let test = harness::WormholeTest::new(8954);

    let home_proj = format!("{}file-proj", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_branch = format!("{}file-task", TEST_PREFIX);

    init_git_repo(&home_dir);

    let test_file = format!("{}/src/main.rs", home_dir);
    std::fs::create_dir_all(format!("{}/src", home_dir)).unwrap();
    std::fs::write(&test_file, "fn main() {}").unwrap();

    test.create_project(&home_dir, &home_proj);

    let worktrees_dir = format!("{}/.git/wormhole/worktrees", home_dir);
    std::fs::create_dir_all(&worktrees_dir).unwrap();
    Command::new("git")
        .args(["add", "."])
        .current_dir(&home_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "-m", "add src"])
        .current_dir(&home_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            &task_branch,
            &format!("{}/{}", worktrees_dir, task_branch),
        ])
        .current_dir(&home_dir)
        .output()
        .unwrap();

    test.http_post("/project/refresh-tasks").unwrap();

    assert!(
        test.task_in_list(&home_proj, &task_branch),
        "Task should be discovered"
    );

    let task_key = test.task_store_key(&task_branch, &home_proj);
    test.cli(&format!("wormhole open '{}'", task_key)).unwrap();

    test.cli(&format!("wormhole open {}", test_file)).unwrap();

    test.assert_focus(Editor(&home_proj));
    test.assert_tmux_cwd(&home_dir);
}

#[test]
fn test_project_list_active_flag() {
    // `wormhole project list --active` should only show projects with tmux windows.
    // Tasks without terminal windows should be excluded.
    use std::process::Command;

    let test = harness::WormholeTest::new(8956);

    let home_proj = format!("{}active-home", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_with_window = format!("{}ACTIVE-WIN", TEST_PREFIX);
    let task_no_window = format!("{}ACTIVE-NOWIN", TEST_PREFIX);

    init_git_repo(&home_dir);

    // Create home project (has tmux window)
    test.create_project(&home_dir, &home_proj);

    // Create a task via wormhole (will have tmux window)
    test.create_task(&task_with_window, &home_proj);

    // Create a worktree directly with git (no tmux window)
    let worktrees_dir = format!("{}/.git/wormhole/worktrees", home_dir);
    std::fs::create_dir_all(&worktrees_dir).unwrap();
    let output = Command::new("git")
        .args([
            "worktree",
            "add",
            "-b",
            &task_no_window,
            &format!("{}/{}", worktrees_dir, task_no_window),
        ])
        .current_dir(&home_dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Failed to create worktree: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Refresh to discover the new task
    test.cli("wormhole refresh").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Verify both tasks are in the regular list
    assert!(
        test.task_in_list(&home_proj, &task_with_window),
        "Task with window should be in regular list"
    );
    assert!(
        test.task_in_list(&home_proj, &task_no_window),
        "Task without window should be in regular list"
    );

    // Get project list with --active flag
    let active_output = test
        .cli("wormhole project list --active --name-only")
        .unwrap();
    let active_lines: Vec<&str> = active_output.lines().collect();

    let task_with_window_key = test.task_store_key(&task_with_window, &home_proj);
    let task_no_window_key = test.task_store_key(&task_no_window, &home_proj);

    // Task with window should appear in --active list
    assert!(
        active_lines
            .iter()
            .any(|l| l.contains(&task_with_window_key)),
        "Task with tmux window '{}' should appear in --active list, got: {:?}",
        task_with_window_key,
        active_lines
    );

    // Task without window should NOT appear in --active list
    assert!(
        !active_lines.iter().any(|l| l.contains(&task_no_window_key)),
        "Task without tmux window '{}' should NOT appear in --active list, got: {:?}",
        task_no_window_key,
        active_lines
    );

    // Home project should appear (it has a tmux window)
    assert!(
        active_lines.iter().any(|l| *l == home_proj),
        "Home project '{}' should appear in --active list, got: {:?}",
        home_proj,
        active_lines
    );
}

#[test]
fn test_switch_creates_task_from_colon_syntax() {
    // `w project switch repo:branch` should create the task if it doesn't exist
    let test = harness::WormholeTest::new(8952);

    let home_proj = format!("{}colon-create", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_branch = format!("{}new-task", TEST_PREFIX);

    // Clean up and create home directory
    init_git_repo(&home_dir);

    // Register home project
    test.create_project(&home_dir, &home_proj);

    // Use colon syntax to create a NEW task (not --home-project/--branch)
    let task_key = format!("{}:{}", home_proj, task_branch);
    test.cli(&format!("wormhole open '{}'", task_key)).unwrap();

    // Give time for task creation
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Verify the worktree was created
    let worktree_path = format!("{}/.git/wormhole/worktrees/{}", home_dir, task_branch);
    assert!(
        std::path::Path::new(&worktree_path).exists(),
        "Worktree should be created at {}",
        worktree_path
    );

    // Refresh and verify task appears in list
    test.cli("wormhole refresh").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));

    assert!(
        test.task_in_list(&home_proj, &task_branch),
        "Task '{}' should be created via colon syntax",
        task_key
    );
}
