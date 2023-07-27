use std::str;
use std::{collections::HashMap, process::Command};

use regex::Regex;

pub fn open_file(path: &str, repo_paths: &HashMap<String, String>) -> Result<bool, &'static str> {
    if path.starts_with("/file/") {
        let path = path.strip_prefix("/file/").unwrap_or("");
        if let Some(repo) = repo_paths.get(path) {
            focus_vscode_workspace(&repo)?;
        }
        open_in_vscode(path, None)?;
        Ok(true)
    } else {
        println!("Not a file URL: {}", path);
        Ok(false)
    }
}

pub fn open_github_url(
    url: &str,
    repo_paths: &HashMap<String, String>,
) -> Result<bool, &'static str> {
    let re = Regex::new(r"/([^/]+)/([^/]+)/blob/([^/]+)/([^?]*)(?:\?line=(\d+))?").unwrap();
    if let Some(captures) = re.captures(url) {
        println!("Handling as github URL");
        let path = captures.get(4).unwrap().as_str();
        let line = captures
            .get(5)
            .map(|m| m.as_str().parse::<usize>().unwrap());
        let repo = captures.get(2).unwrap().as_str();
        println!("path: {} line: {:?} repo: {}", path, line, repo);
        let repo_path = repo_paths.get(repo).unwrap();
        focus_vscode_workspace(repo)?;
        open_in_vscode(&format!("{}/{}", repo_path, path), line)?;
        Ok(true)
    } else {
        println!("Not a github URL: {}", url);
        Ok(false)
    }
}

fn open_in_vscode(path: &str, line: Option<usize>) -> Result<bool, &'static str> {
    let mut uri = format!("vscode-insiders://file/{}", path);
    if let Some(line) = line {
        uri.push_str(&format!(":{}", line));
    }
    let _ = Command::new("open")
        .arg(format!("vscode-insiders://file/{}", path))
        .output()
        .unwrap_or_else(|_| panic!("Failed to open URI: {}", uri));
    Ok(true)
}

fn focus_vscode_workspace(workspace: &str) -> Result<bool, &'static str> {
    let lua_code = format!(
        r#"
    print('Searching for window matching "{}"')
    local function is_vscode_with_workspace(window)
        if string.find(window:application():title(), 'Code', 1, true) then
            print(window:title())
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
        .arg(&lua_code)
        .output()
        .expect("Failed to execute command");

    let stdout = str::from_utf8(&output.stdout).unwrap();
    eprintln!("{}", stdout);
    Ok(true)
}
