mod harness;

use harness::init_git_repo;
use serde_json::Value;

/// Tests for server JSON responses.
/// These use the existing test harness which handles tmux setup.
/// Run with: make integration-test-headless

#[test]
fn test_project_list_json_structure() {
    // Use unique port to avoid conflicts with other tests
    let test = harness::WormholeTest::new(18910);

    let response = test.http_get("/project/list").unwrap();
    let json: Value = serde_json::from_str(&response).expect("Should be valid JSON");

    // Should have "current" and "available" arrays
    assert!(json["current"].is_array(), "Should have 'current' array");
    assert!(
        json["available"].is_array(),
        "Should have 'available' array"
    );
}

#[test]
fn test_project_show_returns_json() {
    let test = harness::WormholeTest::new(18911);

    // Create a test project
    let proj = format!("{}json-test", harness::TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    init_git_repo(&dir);
    test.create_project(&dir, &proj);

    let response = test.http_get(&format!("/project/show/{}", proj)).unwrap();
    let json: Value = serde_json::from_str(&response).expect("Should be valid JSON");

    // Should have expected fields
    assert!(json["name"].is_string(), "Should have 'name' field");
    assert!(json["path"].is_string(), "Should have 'path' field");
}

#[test]
fn test_task_remove_deletes_kv_file() {
    let test = harness::WormholeTest::new(18912);

    let home_proj = format!("{}kv-cleanup", harness::TEST_PREFIX);
    let home_dir = format!("/tmp/{}", home_proj);
    let task_branch = format!("{}cleanup-task", harness::TEST_PREFIX);

    init_git_repo(&home_dir);

    // Create project and task
    test.create_project(&home_dir, &home_proj);
    test.create_task(&task_branch, &home_proj);

    let store_key = test.task_store_key(&task_branch, &home_proj);

    // Set a KV value on the task
    test.http_put(&format!("/kv/{}/test_key", store_key), "test_value")
        .unwrap();

    // Verify KV file exists
    let kv_file = format!(
        "{}/.git/wormhole/kv/{}_{}.json",
        home_dir, home_proj, task_branch
    );
    assert!(
        std::path::Path::new(&kv_file).exists(),
        "KV file should exist before removal: {}",
        kv_file
    );

    // Remove the task
    test.http_post(&format!("/project/remove/{}", store_key))
        .unwrap();

    // Wait for removal to complete
    std::thread::sleep(std::time::Duration::from_millis(500));

    // KV file should be deleted
    assert!(
        !std::path::Path::new(&kv_file).exists(),
        "KV file should be deleted after task removal: {}",
        kv_file
    );
}
