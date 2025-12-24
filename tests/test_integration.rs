use core::panic;
use std::thread;
use std::time::Duration;

mod harness;
use harness::TEST_PREFIX;

#[test]
fn test_open_project_preserves_application() {
    let test = harness::WormholeTest::new(8932);

    let proj_a = format!("{}proj-a", TEST_PREFIX);
    let proj_b = format!("{}proj-b", TEST_PREFIX);
    let dir_a = format!("/tmp/{}", proj_a);
    let dir_b = format!("/tmp/{}", proj_b);

    std::fs::create_dir_all(&dir_a).ok();
    std::fs::create_dir_all(&dir_b).ok();

    test.hs_post(&format!("/add-project/{}?name={}", dir_a, proj_a))
        .unwrap();
    test.hs_post(&format!("/add-project/{}?name={}", dir_b, proj_b))
        .unwrap();

    test.hs_get(&format!("/project/{}", proj_a)).unwrap();
    test.assert_editor_has_focus();

    test.hs_get(&format!("/project/{}", proj_b)).unwrap();
    test.assert_editor_has_focus();
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
    let test = harness::WormholeTest::new(8931);

    let proj = format!("{}file-proj", TEST_PREFIX);
    let dir = format!("/tmp/{}", proj);
    let file = format!("{}/test.rs", dir);

    std::fs::create_dir_all(&dir).ok();
    std::fs::write(&file, "fn main() {}").ok();

    test.hs_post(&format!("/add-project/{}?name={}", dir, proj))
        .unwrap();

    test.hs_get(&format!("/file/{}", file)).unwrap();

    thread::sleep(Duration::from_secs(2));
}

#[test]
fn z_cleanup() {
    let test = harness::WormholeTest::new(8999);
    test.close_cursor_window(TEST_PREFIX);
    thread::sleep(Duration::from_millis(500));
}
