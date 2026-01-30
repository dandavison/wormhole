mod harness;

use serde_json::Value;

/// Tests for server JSON responses.
/// These use the existing test harness which handles tmux setup.
/// Run with: make integration-test-headless

#[test]
fn test_project_list_json_structure() {
    // Use unique port to avoid conflicts with other tests
    let test = harness::WormholeTest::new(18910);

    let response = test.hs_get("/project/list").unwrap();
    let json: Value = serde_json::from_str(&response).expect("Should be valid JSON");

    // Should have "current" and "available" arrays
    assert!(json["current"].is_array(), "Should have 'current' array");
    assert!(json["available"].is_array(), "Should have 'available' array");
}

#[test]
fn test_project_show_returns_json() {
    let test = harness::WormholeTest::new(18911);

    // Create a test project
    let proj = format!("{}json-test", harness::TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    std::fs::create_dir_all(&dir).unwrap();
    test.create_project(&dir, &proj);

    let response = test.hs_get(&format!("/project/show/{}", proj)).unwrap();
    let json: Value = serde_json::from_str(&response).expect("Should be valid JSON");

    // Should have expected fields
    assert!(json["name"].is_string(), "Should have 'name' field");
    assert!(json["path"].is_string(), "Should have 'path' field");
}
