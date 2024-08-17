use std::process::Command;
use std::str;

use crate::editor::Editor;
use crate::project::Project;
use crate::util::{error, info, panic, warn};
use crate::wormhole::{Application, WindowAction};

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

pub fn select_editor_workspace(editor: Editor, project: &Project, action: &WindowAction) -> bool {
    info(&format!(
        "hammerspoon::select_editor_workspace({editor:?}, {project:?} {action:?})"
    ));
    let success_marker = "Found matching window";
    let stdout = hammerspoon(&format!(
        r#"
    print('Searching for application "{}" window matching "{}"')

    function string:endswith(s)
        return self:sub(-#s) == s
    end

    local function is_requested_workspace(window)
        if window:application():title() == '{}' then
            print('window:title() = '  .. window:title())
            return window:title():endswith('{}') or window:title():endswith('{} (Workspace)')
        end
    end

    for _, window in pairs(hs.window.allWindows()) do
        if is_requested_workspace(window) then
            print('Found matching application: ' .. window:application():title())
            print('{}: ' .. window:title())
            window:{}()
            break
        end
    end
    "#,
        editor.application_name(),
        project.name,
        editor.application_name(),
        project.name,
        project.name,
        success_marker,
        action.lua(),
    ));

    let mut success = false;
    for line in str::from_utf8(&stdout).unwrap().split_terminator("\n") {
        info(line);
        success |= line.contains(success_marker);
    }
    success
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
        .unwrap_or_else(|_| panic("Failed to execute hammerspoon"));
    for line in str::from_utf8(&output.stderr)
        .unwrap()
        .split_terminator("\n")
    {
        error(line);
    }
    output.stdout
}
