use std::process::Command;
use std::str;

use crate::project_path::ProjectPath;

pub fn open(path: ProjectPath) -> Result<bool, String> {
    let mut uri = format!(
        "vscode-insiders://file/{}",
        path.absolute_path().to_str().unwrap()
    );
    if let Some(line) = path.line {
        uri.push_str(&format!(":{}", line));
    }
    focus_workspace(&path.project.name)?;
    eprintln!("vscode::open({uri})");
    if let Ok(_) = Command::new("open").arg(&uri).output() {
        Ok(true)
    } else {
        Err(format!("Failed to open URI: {}", uri))
    }
}

fn focus_workspace(workspace: &str) -> Result<bool, String> {
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
        .expect("Failed to execute command");

    let stdout = str::from_utf8(&output.stdout).unwrap();
    eprintln!("{}", stdout);
    Ok(true)
}
