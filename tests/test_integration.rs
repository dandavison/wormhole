mod harness;
use harness::Focus::*;
use harness::TEST_PREFIX;
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
fn test_project_list_sorted() {
    let test = harness::WormholeTest::new(8944);

    let proj_b = format!("{}sort-beta", TEST_PREFIX);
    let proj_a = format!("{}sort-alpha", TEST_PREFIX);
    let dir_b = format!("/tmp/{}", proj_b);
    let dir_a = format!("/tmp/{}", proj_a);
    let task_b1 = format!("{}SORT-B1", TEST_PREFIX);
    let task_a1 = format!("{}SORT-A1", TEST_PREFIX);

    // Create projects in reverse order
    let _ = std::fs::remove_dir_all(&dir_b);
    let _ = std::fs::remove_dir_all(&dir_a);

    for (dir, proj) in [(&dir_b, &proj_b), (&dir_a, &proj_a)] {
        std::fs::create_dir_all(dir).unwrap();
        Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "--allow-empty", "-m", "initial"])
            .current_dir(dir)
            .output()
            .unwrap();
        test.create_project(dir, proj);
    }

    // Create tasks - task_id is now the branch name
    test.create_task(&task_b1, &proj_b);
    test.create_task(&task_a1, &proj_a);

    // Open all in reverse alphabetical order using store_key format for tasks
    let task_b1_key = test.task_store_key(&task_b1, &proj_b);
    let task_a1_key = test.task_store_key(&task_a1, &proj_a);
    test.hs_get(&format!("/project/switch/{}", task_b1_key))
        .unwrap();
    test.hs_get(&format!("/project/switch/{}", proj_b)).unwrap();
    test.hs_get(&format!("/project/switch/{}", task_a1_key))
        .unwrap();
    test.hs_get(&format!("/project/switch/{}", proj_a)).unwrap();

    // Get project list via curl (Hammerspoon can timeout with many rapid calls)
    let output = Command::new("curl")
        .args(["-s", "http://127.0.0.1:8944/project/list"])
        .output()
        .expect("curl failed");
    let list_json = String::from_utf8_lossy(&output.stdout);
    let list: Value = serde_json::from_str(&list_json).expect("invalid JSON from /project/list");
    let current = list["current"].as_array().expect("missing 'current' array");

    // With new model: projects have just "name", tasks have "name" and "branch"
    // Extract identifiers: for projects just name, for tasks name:branch
    let identifiers: Vec<String> = current
        .iter()
        .filter_map(|e| {
            let name = e["name"].as_str()?;
            if !name.starts_with(TEST_PREFIX) {
                return None;
            }
            if let Some(branch) = e["branch"].as_str() {
                Some(format!("{}:{}", name, branch))
            } else {
                Some(name.to_string())
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

    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_id, &home_proj);

    // Task is now identified by (repo, branch). The store_key is "repo:branch".
    let store_key = test.task_store_key(&task_id, &home_proj);

    // Switch to task so it appears in project list (already switched in create_task)
    // Window name is now the store_key for tasks
    test.assert_focus(Editor(&store_key));

    // Verify task is in project list (check name == repo AND branch == task_id)
    let list_json = test.hs_get("/project/list").unwrap();
    let list: Value = serde_json::from_str(&list_json).unwrap();
    let current = list["current"].as_array().unwrap();
    assert!(
        current.iter().any(|e| {
            e["name"].as_str() == Some(home_proj.as_str())
                && e["branch"].as_str() == Some(task_id.as_str())
        }),
        "Task should be in list before close, got: {:?}",
        current
    );

    // Close the task using store_key format
    test.hs_post(&format!("/project/close/{}", store_key))
        .unwrap();

    // Wait for window to close (window name is store_key)
    assert!(
        test.wait_until(|| !test.window_exists(&store_key), 5),
        "Task window should be closed"
    );

    // Verify task is NOT in project list
    let list_json = test.hs_get("/project/list").unwrap();
    let list: Value = serde_json::from_str(&list_json).unwrap();
    let current = list["current"].as_array().unwrap();
    assert!(
        !current.iter().any(|e| {
            e["name"].as_str() == Some(home_proj.as_str())
                && e["branch"].as_str() == Some(task_id.as_str())
        }),
        "Task should NOT be in list after close, got: {:?}",
        current
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
    if editor_is_none() {
        return; // Pin relies on detecting focused GUI app
    }
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

    // Store keys for tasks
    let task_1_key = test.task_store_key(&task_1, &home_proj);
    let task_2_key = test.task_store_key(&task_2, &home_proj);

    let task_1_dir = format!("{}/.git/wormhole/worktrees/{}", home_dir, task_1);
    let task_2_dir = format!("{}/.git/wormhole/worktrees/{}", home_dir, task_2);

    // Switch to home project
    test.hs_get(&format!("/project/switch/{}", home_proj))
        .unwrap();
    test.assert_focus(Editor(&home_proj));
    test.assert_tmux_cwd(&home_dir);

    // Switch to task 1 using store_key
    test.hs_get(&format!("/project/switch/{}", task_1_key))
        .unwrap();
    test.assert_focus(Editor(&task_1_key));
    test.assert_tmux_cwd(&task_1_dir);

    // Switch to task 2 using store_key
    test.hs_get(&format!("/project/switch/{}", task_2_key))
        .unwrap();
    test.assert_focus(Editor(&task_2_key));
    test.assert_tmux_cwd(&task_2_dir);

    // Switch back to home project
    test.hs_get(&format!("/project/switch/{}", home_proj))
        .unwrap();
    test.assert_focus(Editor(&home_proj));
    test.assert_tmux_cwd(&home_dir);

    // Switch to task 1 again
    test.hs_get(&format!("/project/switch/{}", task_1_key))
        .unwrap();
    test.assert_focus(Editor(&task_1_key));
    test.assert_tmux_cwd(&task_1_dir);
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

    // Clean up
    let _ = std::fs::remove_dir_all(&parent_dir);
    let _ = std::fs::remove_dir_all(&child_src);

    // Create source repo (will become submodule)
    std::fs::create_dir_all(&child_src).unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(&child_src)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(&child_src)
        .output()
        .unwrap();

    // Create parent repo
    std::fs::create_dir_all(&parent_dir).unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(&parent_dir)
        .output()
        .unwrap();
    Command::new("git")
        .args(["commit", "--allow-empty", "-m", "initial"])
        .current_dir(&parent_dir)
        .output()
        .unwrap();

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
    test.hs_get(&format!("/project/switch/{}", submodule_name))
        .unwrap();
    test.assert_focus(Editor(&submodule_name));
    test.assert_tmux_cwd(&submodule_dir);

    // Switch to task using store_key format
    let store_key = test.task_store_key(&task_id, &submodule_name);
    test.hs_get(&format!("/project/switch/{}", store_key))
        .unwrap();
    // Window name is now the store_key for tasks
    test.assert_focus(Editor(&store_key));
    test.assert_tmux_cwd(&task_dir);
}

#[test]
fn test_task_home_project_not_self() {
    let test = harness::WormholeTest::new(8940);

    let home_proj = format!("{}home-proj", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_id = format!("{}TASK-SELF", TEST_PREFIX);

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

    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_id, &home_proj);

    // Switch to task so it's in the open projects list
    let store_key = test.task_store_key(&task_id, &home_proj);
    test.hs_get(&format!("/project/switch/{}", store_key))
        .unwrap();
    // Window name is now the store_key for tasks
    test.assert_focus(Editor(&store_key));

    // Get project list and verify task has correct name and branch
    let list_json = test.hs_get("/project/list").unwrap();
    let list: Value = serde_json::from_str(&list_json).unwrap();
    let current = list["current"].as_array().unwrap();

    // Task has name=repo and branch=task_id
    let task_entry = current
        .iter()
        .find(|e| {
            e["name"].as_str() == Some(home_proj.as_str())
                && e["branch"].as_str() == Some(task_id.as_str())
        })
        .expect("Task should be in current list with correct name and branch");

    // Verify the name field is the repo, not the task_id
    assert_eq!(
        task_entry["name"].as_str().unwrap(),
        &home_proj,
        "Task's name should be the repo '{}', not the branch",
        home_proj
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

    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_id, &home_proj);

    // Task store key for the new model
    let store_key = test.task_store_key(&task_id, &home_proj);

    // Switch to home project first
    test.hs_get(&format!("/project/switch/{}", home_proj))
        .unwrap();
    test.assert_focus(Editor(&home_proj));

    // Switch to task using store_key
    test.hs_get(&format!("/project/switch/{}", store_key))
        .unwrap();
    // Window name is the store_key for tasks
    test.assert_focus(Editor(&store_key));

    // Verify both are in the list
    let list_json = test.hs_get("/project/list").unwrap();
    let list: Value = serde_json::from_str(&list_json).unwrap();
    let current = list["current"].as_array().unwrap();

    // Project entry has just name, task entry has name and branch
    let has_project = current
        .iter()
        .any(|e| e["name"].as_str() == Some(home_proj.as_str()) && e["branch"].is_null());
    let has_task = current.iter().any(|e| {
        e["name"].as_str() == Some(home_proj.as_str())
            && e["branch"].as_str() == Some(task_id.as_str())
    });
    assert!(has_project, "Home project should be in list");
    assert!(has_task, "Task should be in list");

    // Toggle back via previous - should go to home project
    test.hs_get("/project/previous").unwrap();
    test.assert_focus(Editor(&home_proj));

    // Toggle forward via next - should go to task
    test.hs_get("/project/next").unwrap();
    test.assert_focus(Editor(&store_key));
}

#[test]
fn test_project_status() {
    let test = harness::WormholeTest::new(8938);

    let proj = format!("{}status-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);

    std::fs::create_dir_all(format!("{}/.task", dir)).unwrap();
    std::fs::write(format!("{}/.task/plan.md", dir), "# Plan").unwrap();

    test.create_project(&dir, &proj);
    test.assert_focus(Editor(&proj));

    // Get info by name
    let status = test.hs_get(&format!("/project/show/{}", proj)).unwrap();
    assert!(status.contains(&proj), "Status should contain project name");

    // Get current project info
    let status = test.hs_get("/project/show").unwrap();
    assert!(
        status.contains(&proj),
        "Current status should contain project name"
    );

    // Get JSON format
    let status = test
        .hs_get(&format!("/project/show/{}?format=json", proj))
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

    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_id, &home_proj);

    // Store key is now "repo:branch"
    let store_key = test.task_store_key(&task_id, &home_proj);

    // Directly set land-in=terminal for the task using store_key
    test.hs_put(&format!("/kv/{}/land-in", store_key), "terminal")
        .unwrap();

    // Switch to home project first (so we're not on the task)
    test.hs_get(&format!("/project/switch/{}", home_proj))
        .unwrap();
    test.assert_focus(Editor(&home_proj));

    // Switch to task - should respect land-in=terminal
    test.hs_get(&format!("/project/switch/{}", store_key))
        .unwrap();
    test.assert_focus(Terminal(&store_key));
}

#[test]
fn test_task_list_reflects_tmux_window_state() {
    // Regression test: The project list should only include tasks whose tmux windows exist.
    // Bug: The filter was `terminal_windows.contains(&p.repo_name) || p.is_task()` which
    // incorrectly included ALL tasks regardless of whether their tmux window was open.
    // Fix: Changed to `terminal_windows.contains(&p.store_key().to_string())` to properly
    // check for window existence using the full store key (repo:branch).
    let test = harness::WormholeTest::new(8945);

    let home_proj = format!("{}list-home", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_1 = format!("{}LIST-T1", TEST_PREFIX);
    let task_2 = format!("{}LIST-T2", TEST_PREFIX);

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

    // Create home project and two tasks
    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_1, &home_proj);
    test.create_task(&task_2, &home_proj);

    let task_1_key = test.task_store_key(&task_1, &home_proj);
    let task_2_key = test.task_store_key(&task_2, &home_proj);

    // Switch to both tasks to ensure they're in the ring and have tmux windows
    test.hs_get(&format!("/project/switch/{}", task_1_key))
        .unwrap();
    test.hs_get(&format!("/project/switch/{}", task_2_key))
        .unwrap();

    // Verify both tasks are in the project list
    let list_json = test.hs_get("/project/list").unwrap();
    let list: Value = serde_json::from_str(&list_json).unwrap();
    let current = list["current"].as_array().unwrap();

    let has_task_1 = current.iter().any(|e| {
        e["name"].as_str() == Some(home_proj.as_str())
            && e["branch"].as_str() == Some(task_1.as_str())
    });
    let has_task_2 = current.iter().any(|e| {
        e["name"].as_str() == Some(home_proj.as_str())
            && e["branch"].as_str() == Some(task_2.as_str())
    });
    assert!(has_task_1, "Task 1 should be in list initially");
    assert!(has_task_2, "Task 2 should be in list initially");

    // Kill task 1's tmux window directly (bypassing wormhole's close_project)
    // This leaves task 1 in the ring but without a tmux window
    test.kill_tmux_window(&task_1_key);

    // Wait for window to be gone
    assert!(
        test.wait_until(|| !test.tmux_window_exists(&task_1_key), 5),
        "Task 1 tmux window should be closed"
    );

    // Verify task 1 is NOT in the project list (window closed)
    // but task 2 IS still in the list (window still open)
    let list_json = test.hs_get("/project/list").unwrap();
    let list: Value = serde_json::from_str(&list_json).unwrap();
    let current = list["current"].as_array().unwrap();

    let has_task_1_after = current.iter().any(|e| {
        e["name"].as_str() == Some(home_proj.as_str())
            && e["branch"].as_str() == Some(task_1.as_str())
    });
    let has_task_2_after = current.iter().any(|e| {
        e["name"].as_str() == Some(home_proj.as_str())
            && e["branch"].as_str() == Some(task_2.as_str())
    });

    assert!(
        !has_task_1_after,
        "Task 1 should NOT be in list after tmux window closed, got: {:?}",
        current
    );
    assert!(
        has_task_2_after,
        "Task 2 should still be in list (window still open)"
    );
}

#[test]
fn test_neighbors_returns_branch_for_tasks() {
    // Regression test: The /project/neighbors endpoint must return `branch` for tasks
    // so that clients (like Hammerspoon) can distinguish tasks from projects and
    // correctly identify the current item in the ring.
    // Bug: Hammerspoon checked `item.home` to detect tasks, but the new data model
    // uses `item.branch`. Also, `isCurrent` checked just `item.name` which couldn't
    // distinguish between tasks from the same repo.
    // Fix: Hammerspoon now checks `item.branch` and uses `name:branch` as unique key.
    let test = harness::WormholeTest::new(8946);

    let home_proj = format!("{}neighbors-home", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_1 = format!("{}NBR-T1", TEST_PREFIX);
    let task_2 = format!("{}NBR-T2", TEST_PREFIX);

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

    // Create home project and two tasks from the same repo
    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_1, &home_proj);
    test.create_task(&task_2, &home_proj);

    let task_1_key = test.task_store_key(&task_1, &home_proj);
    let task_2_key = test.task_store_key(&task_2, &home_proj);

    // Switch to both tasks to add them to the ring
    test.hs_get(&format!("/project/switch/{}", task_1_key))
        .unwrap();
    test.hs_get(&format!("/project/switch/{}", task_2_key))
        .unwrap();

    // Get neighbors endpoint
    let neighbors_json = test.hs_get("/project/neighbors").unwrap();
    let neighbors: Value = serde_json::from_str(&neighbors_json).unwrap();
    let ring = neighbors["ring"]
        .as_array()
        .expect("ring should be an array");

    // Find the two tasks in the ring
    let task_1_entry = ring.iter().find(|e| {
        e["name"].as_str() == Some(home_proj.as_str())
            && e["branch"].as_str() == Some(task_1.as_str())
    });
    let task_2_entry = ring.iter().find(|e| {
        e["name"].as_str() == Some(home_proj.as_str())
            && e["branch"].as_str() == Some(task_2.as_str())
    });

    // Verify tasks have `branch` field (not `home`)
    assert!(
        task_1_entry.is_some(),
        "Task 1 should be in ring with name={} and branch={}, got: {:?}",
        home_proj,
        task_1,
        ring
    );
    assert!(
        task_2_entry.is_some(),
        "Task 2 should be in ring with name={} and branch={}, got: {:?}",
        home_proj,
        task_2,
        ring
    );

    // Verify that tasks do NOT have `home` field (old model)
    assert!(
        task_1_entry.unwrap().get("home").is_none(),
        "Tasks should not have 'home' field in new model"
    );
    assert!(
        task_2_entry.unwrap().get("home").is_none(),
        "Tasks should not have 'home' field in new model"
    );

    // Verify that regular project doesn't have branch
    let project_entry = ring
        .iter()
        .find(|e| e["name"].as_str() == Some(home_proj.as_str()) && e["branch"].is_null());
    assert!(
        project_entry.is_some(),
        "Regular project should be in ring without branch field"
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
    let _ = std::fs::remove_dir_all(&home_dir);
    std::fs::create_dir_all(&home_dir).unwrap();

    // Initialize git repo
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
    test.hs_post("/project/refresh").unwrap();

    // Give a moment for refresh to complete
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Get project list
    let response = test.hs_get("/project/list").unwrap();
    let data: Value = serde_json::from_str(&response).unwrap();
    let current = data["current"].as_array().expect("current should be array");

    // The task should appear in the list even though it has no terminal window
    let task_store_key = format!("{}:{}", home_proj, task_branch);
    let task_entry = current.iter().find(|e| {
        e["name"].as_str() == Some(home_proj.as_str())
            && e["branch"].as_str() == Some(task_branch.as_str())
    });

    assert!(
        task_entry.is_some(),
        "Task '{}' should appear in project list even without terminal window. Got: {:?}",
        task_store_key,
        current
    );
}

#[test]
fn test_switch_creates_task_from_colon_syntax() {
    // `w project switch repo:branch` should create the task if it doesn't exist
    use std::process::Command;

    let test = harness::WormholeTest::new(8952);

    let home_proj = format!("{}colon-create", TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_branch = format!("{}new-task", TEST_PREFIX);

    // Clean up and create home directory
    let _ = std::fs::remove_dir_all(&home_dir);
    std::fs::create_dir_all(&home_dir).unwrap();

    // Initialize git repo
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

    // Register home project
    test.create_project(&home_dir, &home_proj);

    // Use colon syntax to create a NEW task (not --home-project/--branch)
    let task_key = format!("{}:{}", home_proj, task_branch);
    let response = test
        .hs_get(&format!("/project/switch/{}?sync=1", task_key))
        .unwrap();
    assert!(
        response.contains("ok") || response.is_empty(),
        "Task creation via colon syntax failed: {}",
        response
    );

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
    test.hs_post("/project/refresh").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));

    let response = test.hs_get("/project/list").unwrap();
    let data: Value = serde_json::from_str(&response).unwrap();
    let current = data["current"].as_array().expect("current should be array");

    let task_entry = current.iter().find(|e| {
        e["name"].as_str() == Some(home_proj.as_str())
            && e["branch"].as_str() == Some(task_branch.as_str())
    });

    assert!(
        task_entry.is_some(),
        "Task '{}' should be created via colon syntax. Got: {:?}",
        task_key,
        current
    );
}
