use std::process::Command;
use std::str;

use crate::util::{error, panic, warn};
use crate::wormhole::Application;
use crate::{config, ps};

pub fn current_application() -> Application {
    match str::from_utf8(&hammerspoon(
        r#"
        local focusedWindow = hs.window.focusedWindow()
        if focusedWindow then
            local app = focusedWindow:application()
            if app then
                print(app:title())
            end
        end
    "#,
    ))
    .map(str::trim)
    {
        Ok(app_title) => {
            if app_title == config::TERMINAL.application_name() {
                Application::Terminal
            } else {
                Application::Editor
            }
        }
        Err(err) => {
            warn(&format!("current_application() ERROR: {err}"));
            Application::Editor
        }
    }
}

pub fn launch_or_focus(application_name: &str) {
    if crate::util::debug() {
        ps!("launch_or_focus({application_name})");
    }
    hammerspoon(&format!(
        r#"
        hs.application.launchOrFocus("/Applications/{application_name}.app")
    "#,
    ));
}

pub fn close_window(application_name: &str, title_pattern: &str) {
    let lua_pattern = title_pattern.replace("-", "%-");
    hammerspoon(&format!(
        r#"
        local app = hs.application.find('{application_name}')
        if app then
            for _, w in ipairs(app:allWindows()) do
                if string.find(w:title(), "{lua_pattern}") then
                    w:close()
                end
            end
        end
        "#,
    ));
}

pub fn any_windows_exist(application_name: &str) -> bool {
    let result = hammerspoon(&format!(
        r#"
        local app = hs.application.find('{application_name}')
        if app and #app:allWindows() > 0 then
            return "true"
        end
        return "false"
        "#,
    ));
    String::from_utf8_lossy(&result).trim() == "true"
}

pub fn close_all_windows(application_name: &str) {
    hammerspoon(&format!(
        r#"
        local app = hs.application.find('{application_name}')
        if app then
            for _, w in ipairs(app:allWindows()) do
                w:close()
            end
        end
        "#,
    ));
}

pub fn close_all_windows_and_wait(application_name: &str, timeout_secs: u64) {
    use std::time::{Duration, Instant};
    use std::thread;

    let start = Instant::now();
    let timeout = Duration::from_secs(timeout_secs);
    while start.elapsed() < timeout {
        close_all_windows(application_name);
        if !any_windows_exist(application_name) {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

pub fn alert(message: &str) {
    hammerspoon(&format!(r#"hs.alert.show("{message}", 0.5)"#,));
}

fn hammerspoon(lua: &str) -> Vec<u8> {
    let output = Command::new("hs")
        .arg("-c")
        .arg(lua)
        .output()
        .unwrap_or_else(|_| panic("Failed to execute hammerspoon"));
    for line in str::from_utf8(&output.stderr)
        .unwrap()
        .split_terminator("\n")
    {
        error(line);
    }
    output.stdout
}
