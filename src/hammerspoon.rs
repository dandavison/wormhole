use std::str;

use crate::command;
use crate::util::{error, warn};
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
            if app_title == config::terminal().application_name() {
                Application::Terminal
            } else if app_title == config::editor().application_name() {
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
    let result = command::execute_with_stderr("hs", ["-c", lua]);

    // Log any errors from stderr
    if !result.stderr.is_empty() {
        for line in str::from_utf8(&result.stderr)
            .unwrap_or("")
            .split_terminator("\n")
        {
            if !line.is_empty() {
                error(line);
            }
        }
    }

    result.stdout
}
