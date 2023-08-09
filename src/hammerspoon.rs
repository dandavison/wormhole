use std::process::{Command, Output};
use std::str;

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
    let found = "found";
    if let Ok(output) = hammerspoon(&format!(
        r#"
    local function is_vscode_with_workspace(window)
        if string.find(window:application():title(), 'Code', 1, true) then
            return string.find(window:title(), '{}', 1, true)
        end
    end

    for _, window in pairs(hs.window.allWindows()) do
        if is_vscode_with_workspace(window) then
            window:{}()
            print("{}")
        end
    end
    "#,
        workspace,
        action.lua(),
        found,
    )) {
        Ok(str::from_utf8(&output.stdout)
            .map(|s| s.contains(found))
            .unwrap_or(true))
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
