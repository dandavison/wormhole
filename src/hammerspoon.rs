use std::process::Command;
use std::str;

use crate::editor::Editor;
use crate::project::Project;
use crate::util::info;
use crate::WindowAction;

impl WindowAction {
    fn lua(&self) -> &'static str {
        match self {
            WindowAction::Focus => "focus",
            WindowAction::Raise => "raise",
        }
    }
}

pub fn select_editor_workspace(
    editor: Editor,
    project: &Project,
    action: WindowAction,
) -> Result<(), String> {
    hammerspoon(&format!(
        r#"
    print('Searching for application "{}" window matching "{}"')
    local function is_requested_workspace(window)
        print(window:application():title() .. ' -- ' window:title())
        if window:application():title() == {} then
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
    Ok(())
}

pub fn focus_alacritty() {
    hammerspoon(&format!(
        r#"
        hs.application.launchOrFocus("/Applications/Alacritty.app")
    "#,
    ));
}

fn hammerspoon(lua: &str) {
    let output = Command::new("hs")
        .arg("-c")
        .arg(lua)
        .output()
        .expect("Failed to execute hammerspoon")
        .stdout;

    for line in str::from_utf8(&output).unwrap().split_terminator("\n") {
        info(line);
    }
}
