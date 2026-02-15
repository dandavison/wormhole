mod harness;

use serde_json::Value;

/// Tests for the /batch HTTP API.
/// Run with: make integration-test-headless

#[test]
fn test_batch_create_and_poll() {
    let test = harness::WormholeTest::new(18920);

    let body = serde_json::json!({
        "command": ["echo", "hello"],
        "runs": [
            { "key": "a", "dir": "/tmp" },
            { "key": "b", "dir": "/tmp" },
        ]
    });
    let response = test
        .http_post_json("/batch", &body.to_string())
        .unwrap();
    let batch: Value = serde_json::from_str(&response).unwrap();

    assert!(batch["id"].is_string(), "batch should have an id");
    assert_eq!(batch["total"].as_u64(), Some(2));
    assert_eq!(batch["command"], serde_json::json!(["echo", "hello"]));

    // Poll until done
    let id = batch["id"].as_str().unwrap();
    let done = test.wait_until(
        || {
            let resp = test.http_get(&format!("/batch/{}", id)).unwrap();
            let b: Value = serde_json::from_str(&resp).unwrap();
            b["done"].as_bool() == Some(true)
        },
        10,
    );
    assert!(done, "batch should complete");

    let resp = test.http_get(&format!("/batch/{}", id)).unwrap();
    let batch: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(batch["completed"].as_u64(), Some(2));

    // All runs should have succeeded with stdout
    for run in batch["runs"].as_array().unwrap() {
        assert_eq!(run["status"].as_str(), Some("succeeded"));
        assert_eq!(run["exit_code"].as_u64(), Some(0));
        assert_eq!(run["stdout"].as_str().unwrap().trim(), "hello");
    }
}

#[test]
fn test_batch_list() {
    let test = harness::WormholeTest::new(18921);

    // Create a batch
    let body = serde_json::json!({
        "command": ["true"],
        "runs": [{ "key": "x", "dir": "/tmp" }]
    });
    test.http_post_json("/batch", &body.to_string()).unwrap();

    // List batches
    let resp = test.http_get("/batch").unwrap();
    let list: Value = serde_json::from_str(&resp).unwrap();
    assert!(list["batches"].is_array());
    assert!(
        !list["batches"].as_array().unwrap().is_empty(),
        "should have at least one batch"
    );
}

#[test]
fn test_batch_failed_command_shows_stderr() {
    let test = harness::WormholeTest::new(18922);

    let body = serde_json::json!({
        "command": ["nonexistent_cmd_xyz"],
        "runs": [{ "key": "proj", "dir": "/tmp" }]
    });
    let response = test
        .http_post_json("/batch", &body.to_string())
        .unwrap();
    let batch: Value = serde_json::from_str(&response).unwrap();
    let id = batch["id"].as_str().unwrap();

    let done = test.wait_until(
        || {
            let resp = test.http_get(&format!("/batch/{}", id)).unwrap();
            let b: Value = serde_json::from_str(&resp).unwrap();
            b["done"].as_bool() == Some(true)
        },
        10,
    );
    assert!(done, "batch should complete");

    let resp = test.http_get(&format!("/batch/{}", id)).unwrap();
    let batch: Value = serde_json::from_str(&resp).unwrap();
    let run = &batch["runs"][0];
    assert_eq!(run["status"].as_str(), Some("failed"));
    let stderr = run["stderr"].as_str().unwrap_or("");
    assert!(
        !stderr.is_empty(),
        "failed run should have non-empty stderr explaining the error"
    );
}

#[test]
fn test_batch_shell_features() {
    let test = harness::WormholeTest::new(18923);

    // Single-string command with pipe
    let body = serde_json::json!({
        "command": ["echo hello | tr a-z A-Z"],
        "runs": [{ "key": "proj", "dir": "/tmp" }]
    });
    let response = test
        .http_post_json("/batch", &body.to_string())
        .unwrap();
    let batch: Value = serde_json::from_str(&response).unwrap();
    let id = batch["id"].as_str().unwrap();

    let done = test.wait_until(
        || {
            let resp = test.http_get(&format!("/batch/{}", id)).unwrap();
            let b: Value = serde_json::from_str(&resp).unwrap();
            b["done"].as_bool() == Some(true)
        },
        10,
    );
    assert!(done, "batch should complete");

    let resp = test.http_get(&format!("/batch/{}", id)).unwrap();
    let batch: Value = serde_json::from_str(&resp).unwrap();
    let run = &batch["runs"][0];
    assert_eq!(run["status"].as_str(), Some("succeeded"));
    assert_eq!(run["stdout"].as_str().unwrap().trim(), "HELLO");
}

#[test]
fn test_batch_cancel() {
    let test = harness::WormholeTest::new(18924);

    let body = serde_json::json!({
        "command": ["sleep", "999"],
        "runs": [{ "key": "a", "dir": "/tmp" }]
    });
    let response = test
        .http_post_json("/batch", &body.to_string())
        .unwrap();
    let batch: Value = serde_json::from_str(&response).unwrap();
    let id = batch["id"].as_str().unwrap();

    // Wait for it to start running
    let started = test.wait_until(
        || {
            let resp = test.http_get(&format!("/batch/{}", id)).unwrap();
            let b: Value = serde_json::from_str(&resp).unwrap();
            b["runs"][0]["status"].as_str() == Some("running")
        },
        5,
    );
    assert!(started, "run should start");

    // Cancel
    let resp = test
        .http_post(&format!("/batch/{}/cancel", id))
        .unwrap();
    let _batch: Value = serde_json::from_str(&resp).unwrap();

    // Wait for completion
    let done = test.wait_until(
        || {
            let resp = test.http_get(&format!("/batch/{}", id)).unwrap();
            let b: Value = serde_json::from_str(&resp).unwrap();
            b["done"].as_bool() == Some(true)
        },
        10,
    );
    assert!(done, "batch should complete after cancel");

    let resp = test.http_get(&format!("/batch/{}", id)).unwrap();
    let batch: Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(
        batch["runs"][0]["status"].as_str(),
        Some("cancelled"),
        "run should be cancelled"
    );
}

#[test]
fn test_batch_long_poll() {
    let test = harness::WormholeTest::new(18925);

    let body = serde_json::json!({
        "command": ["echo", "done"],
        "runs": [{ "key": "a", "dir": "/tmp" }]
    });
    let response = test
        .http_post_json("/batch", &body.to_string())
        .unwrap();
    let batch: Value = serde_json::from_str(&response).unwrap();
    let id = batch["id"].as_str().unwrap();

    // Wait for completion, then query with completed=0 - should return immediately
    let done = test.wait_until(
        || {
            let resp = test.http_get(&format!("/batch/{}", id)).unwrap();
            let b: Value = serde_json::from_str(&resp).unwrap();
            b["done"].as_bool() == Some(true)
        },
        10,
    );
    assert!(done);

    let start = std::time::Instant::now();
    let (body, _) = test
        .http_get_with_header(
            &format!("/batch/{}?completed=0", id),
            "Prefer: wait=5",
        )
        .unwrap();
    let elapsed = start.elapsed();

    let batch: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(batch["done"].as_bool(), Some(true));
    assert!(
        elapsed.as_secs() < 2,
        "should return immediately when already ahead, took {:?}",
        elapsed
    );
}
