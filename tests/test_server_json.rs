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

#[test]
fn test_poll_current_json_structure() {
    let test = harness::WormholeTest::new(18913);

    // Poll with short timeout
    let (body, headers) = test
        .http_get_with_header("/project/current/poll", "Prefer: wait=1")
        .unwrap();
    let json: Value = serde_json::from_str(&body).expect("Should be valid JSON");

    // Should have expected fields
    assert!(
        json.get("current").is_some(),
        "Should have 'current' field, got: {}",
        body
    );
    assert!(
        json["changed"].is_boolean(),
        "Should have 'changed' boolean field"
    );

    // Should have Preference-Applied header (case-insensitive)
    assert!(
        headers
            .to_lowercase()
            .contains("preference-applied: wait=1"),
        "Should have Preference-Applied header, got: {}",
        headers
    );
}

#[test]
fn test_poll_current_immediate_on_mismatch() {
    let test = harness::WormholeTest::new(18914);

    // Create a project so there's a current project
    let proj = format!("{}poll-mismatch", harness::TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    init_git_repo(&dir);
    test.create_project(&dir, &proj);

    // Poll with a wrong current value - should return immediately
    let start = std::time::Instant::now();
    let (body, _) = test
        .http_get_with_header(
            "/project/current/poll?current=nonexistent",
            "Prefer: wait=30",
        )
        .unwrap();
    let elapsed = start.elapsed();

    let json: Value = serde_json::from_str(&body).expect("Should be valid JSON");

    // Should return immediately (well under 30s timeout)
    assert!(
        elapsed.as_secs() < 2,
        "Should return immediately on mismatch, took {:?}",
        elapsed
    );

    // Should indicate change
    assert_eq!(
        json["changed"].as_bool(),
        Some(true),
        "Should report changed=true on mismatch"
    );

    // Current should be the actual current project
    assert!(
        json["current"].as_str().is_some(),
        "Should return current project"
    );
}

#[test]
fn test_poll_current_timeout_no_change() {
    let test = harness::WormholeTest::new(18915);

    // Create a project
    let proj = format!("{}poll-timeout", harness::TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    init_git_repo(&dir);
    test.create_project(&dir, &proj);

    // Poll with the correct current value and short timeout
    let start = std::time::Instant::now();
    let (body, _) = test
        .http_get_with_header(
            &format!("/project/current/poll?current={}", proj),
            "Prefer: wait=1",
        )
        .unwrap();
    let elapsed = start.elapsed();

    let json: Value = serde_json::from_str(&body).expect("Should be valid JSON");

    // Should take approximately the timeout duration
    assert!(
        elapsed.as_millis() >= 800,
        "Should wait near timeout duration, took {:?}",
        elapsed
    );

    // Should indicate no change
    assert_eq!(
        json["changed"].as_bool(),
        Some(false),
        "Should report changed=false on timeout"
    );

    // Current should still be the same
    assert_eq!(
        json["current"].as_str(),
        Some(proj.as_str()),
        "Current should remain unchanged"
    );
}

#[test]
fn test_poll_current_empty_string_treated_as_none() {
    let test = harness::WormholeTest::new(18919);

    // First, get current state without any current param
    let (body1, _) = test
        .http_get_with_header("/project/current/poll", "Prefer: wait=1")
        .unwrap();
    let json1: Value = serde_json::from_str(&body1).expect("Should be valid JSON");

    // Now poll with empty current= param (which JS sends as encodeURIComponent(''))
    // This should be treated identically to no param at all
    let (body2, _) = test
        .http_get_with_header("/project/current/poll?current=", "Prefer: wait=1")
        .unwrap();
    let json2: Value = serde_json::from_str(&body2).expect("Should be valid JSON");

    // Both should return the same result - empty string should be treated as None
    assert_eq!(
        json1["current"], json2["current"],
        "Empty current= should behave same as no current param"
    );
    assert_eq!(
        json1["changed"], json2["changed"],
        "Empty current= should behave same as no current param"
    );
}
