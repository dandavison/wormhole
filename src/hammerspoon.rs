use std::process::Command;
use std::str;

use crate::editor::Editor;
use crate::project::Project;
use crate::util::{error, info, warn};
use crate::{Application, WindowAction};

impl WindowAction {
    fn lua(&self) -> &'static str {
        match self {
            WindowAction::Focus => "focus",
            WindowAction::Raise => "raise",
        }
    }
}

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

pub fn select_editor_workspace(
    editor: Editor,
    project: &Project,
    action: &WindowAction,
) -> Result<(), String> {
    info(&format!(
        "hammerspoon::select_editor_workspace({editor:?}, {project:?} {action:?})"
    ));
    let stdout = hammerspoon(&format!(
        r#"
    print('Searching for application "{}" window matching "{}"')
    local function is_requested_workspace(window)
        if window:application():title() == '{}' then
            print('window:title() = '  .. window:title())
            return string.find(window:title(), '{}', 1, true)
        end
    end

    for _, window in pairs(hs.window.allWindows()) do
        if is_requested_workspace(window) then
            print('Found matching application: ' .. window:application():title())
            print('Found matching window: ' .. window:title())
            window:{}()
            break
        end
    end
    "#,
        editor.application_name(),
        project.name,
        editor.application_name(),
        project.name,
        action.lua(),
    ));

    for line in str::from_utf8(&stdout).unwrap().split_terminator("\n") {
        info(line);
    }
    Ok(())
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
