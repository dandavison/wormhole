use core::panic;
use std::thread;
use std::time::Duration;

mod harness;

#[test]
fn test_open_project_preserves_application() {
    let test = harness::WormholeTest::new(8932);

    std::fs::create_dir_all("/tmp/test-proj-a").ok();
    std::fs::create_dir_all("/tmp/test-proj-b").ok();

    test.hs_post("/add-project//tmp/test-proj-a?name=proj-a")
        .unwrap_or_else(|_| panic!("Failed to add proj-a"));
    test.hs_post("/add-project//tmp/test-proj-b?name=proj-b")
        .unwrap_or_else(|_| panic!("Failed to add proj-b"));

    test.hs_get("/project/proj-a")
        .unwrap_or_else(|_| panic!("Failed to open proj-a"));

    assert!(
        test.wait_for_app_focus("Cursor", 5),
        "Expected Cursor to have focus after opening proj-a"
    );

    test.hs_get("/project/proj-b")
        .unwrap_or_else(|_| panic!("Failed to open proj-b"));

    assert!(
        test.wait_for_app_focus("Cursor", 5),
        "Expected Cursor to have focus after switching to proj-b"
    );
}

#[test]
fn test_navigation_no_deadlock() {
    let test = harness::WormholeTest::new(8930);

    match test.hs_get("/previous-project/") {
        Err(e) if e.contains("timeout") => panic!("Deadlock detected! {}", e),
        _ => {}
    }

    match test.hs_get("/next-project/") {
        Err(e) if e.contains("timeout") => panic!("Deadlock detected! {}", e),
        _ => {}
    }
}

#[test]
fn test_file_opens_in_editor() {
    std::fs::create_dir_all("/tmp/test-file-proj").ok();
    std::fs::write("/tmp/test-file-proj/test.rs", "fn main() {}").ok();

    let test = harness::WormholeTest::new(8931);

    test.hs_post("/add-project//tmp/test-file-proj?name=file-proj")
        .unwrap_or_else(|_| panic!("Failed to add project"));

    test.hs_get("/file//tmp/test-file-proj/test.rs")
        .unwrap_or_else(|_| panic!("Failed to open file"));

    thread::sleep(Duration::from_secs(2));
}
