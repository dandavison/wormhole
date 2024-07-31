use std::process::Command;
use std::str;

use crate::editor::Editor;
use crate::util::{error, warn};
use crate::wormhole::Application;

pub fn current_application() -> Application {
    let mut app = Application::Other;
    match str::from_utf8(&hammerspoon(
        r#"
        local focusedWindow = hs.window.focusedWindow()
        if focusedWindow then
            print(focusedWindow:application():title())
        end
    "#,
    )) {
        Ok(app_title) => {
            let app_title = app_title.trim();
            if app_title == "Alacritty" {
                app = Application::Terminal;
            } else {
                for editor in [
                    Editor::VSCodeInsiders,
                    Editor::VSCode,
                    Editor::PyCharm,
                    Editor::IntelliJ,
                ] {
                    if app_title == editor.application_name() {
                        app = Application::Editor;
                    }
                }
            }
        }
        Err(err) => warn(&format!("current_application() ERROR: {err}")),
    }
    app
}

pub fn launch_or_focus(application_name: &str) {
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
        .expect("Failed to execute hammerspoon");
    for line in str::from_utf8(&output.stderr)
        .unwrap()
        .split_terminator("\n")
    {
        error(line);
    }
    output.stdout
}
