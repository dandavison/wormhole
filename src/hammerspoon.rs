use std::process::Command;
use std::str;

pub fn focus_vscode_workspace(workspace: &str) -> Result<bool, String> {
    let hammerspoon = format!(
        r#"
    print('Searching for window matching "{}"')
    local function is_vscode_with_workspace(window)
        if string.find(window:application():title(), 'Code', 1, true) then
            return string.find(window:title(), '{}', 1, true)
        end
    end

    for _, window in pairs(hs.window.allWindows()) do
        if is_vscode_with_workspace(window) then
            print('Found matching window: ' .. window:title())
            window:focus()
            break
        end
    end
    "#,
        workspace, workspace
    );

    let output = Command::new("hs")
        .arg("-c")
        .arg(&hammerspoon)
        .output()
        .expect("Failed to execute command")
        .stdout;

    let stdout = str::from_utf8(&output).unwrap();
    eprintln!("{}", stdout);
    Ok(true)
}
