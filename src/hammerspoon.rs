use std::process::{Command, Output};
use std::str;

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

pub fn select_vscode_workspace(workspace: &str, action: &WindowAction) -> Result<bool, String> {
    let found_it = "found-it";
    if let Ok(output) = hammerspoon(&format!(
        r#"
    local function is_requested_vscode_workspace(window)
        if string.find(window:application():title(), 'Code', 1, true) then
            print(window:title())
            return string.find(window:title(), '{}', 1, true)
        end
    end

    print("Searching for window: {}")
    for _, window in pairs(hs.window.allWindows()) do
        if is_requested_vscode_workspace(window) then
            window:{}()
            print("{}")
            break
        end
    end
    "#,
        workspace,
        workspace,
        action.lua(),
        found_it,
    )) {
        let mut found = false;
        for line in str::from_utf8(&output.stdout)
            .unwrap()
            .split_terminator("\n")
        {
            info(line);
            if line.contains(found_it) {
                found = true
            }
        }
        Ok(found)
    } else {
        Err("Hammerspoon command failed".into())
    }
}

pub fn focus_alacritty() {
    hammerspoon(&format!(
        r#"
        hs.application.launchOrFocus("/Applications/Alacritty.app")
    "#,
    ))
    .expect("Hammerspoon command must succeed");
}

fn hammerspoon(lua: &str) -> std::io::Result<Output> {
    Command::new("hs").arg("-c").arg(lua).output()
}
