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
            print(focusedWindow:application():title())
        end
    "#,
    ))
    .map(str::trim)
    {
        Ok(app_title) => {
            if app_title == config::TERMINAL.application_name() {
                Application::Terminal
            } else if app_title == config::EDITOR.application_name() {
                Application::Editor
            } else {
                Application::Other
            }
        }
        Err(err) => {
            warn(&format!("current_application() ERROR: {err}"));
            Application::Other
        }
    }
}

pub fn launch_or_focus(application_name: &str) {
    ps!("Focusing {}", application_name);
    hammerspoon(&format!(
        r#"
        hs.application.launchOrFocus("/Applications/{application_name}.app")
    "#,
    ));
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
