use serde::Deserialize;
use std::ffi::OsStr;
use std::process::Command;
use std::{str, thread};

use crate::project::Project;
use crate::terminal::write_wormhole_env_vars;
use crate::util::{info, panic, warn};

#[allow(dead_code)]
#[derive(Deserialize)]
struct PaneSize {
    rows: u32,
    cols: u32,
    pixel_width: u32,
    pixel_height: u32,
    dpi: u32,
}

#[allow(dead_code)]
#[derive(Deserialize)]
struct Pane {
    window_id: u32,
    tab_id: u32,
    pane_id: u32,
    workspace: String,
    title: String,
    cwd: String,
    size: PaneSize,
    cursor_x: u32,
    cursor_y: u32,
    cursor_shape: String,
    cursor_visibility: String,
    left_col: u32,
    top_row: u32,
    tab_title: String,
    window_title: String,
    is_active: bool,
    is_zoomed: bool,
    tty_name: Option<String>,
}

pub fn open(project: &Project) -> Result<(), String> {
    info(&format!("wezterm::open({project:?})"));
    let pane = Pane::get_first_by_tab_title(&project.name)
        .unwrap_or_else(|| new_tab(&project.name, &project.path.to_str().unwrap()));
    wezterm(["cli", "activate-tab", "--tab-id", &pane.tab_id.to_string()]);
    let project = project.clone();
    thread::spawn(move || write_wormhole_env_vars(&project));
    Ok(())
}

fn new_tab(title: &str, cwd: &str) -> Pane {
    let pane_id: u32 = wezterm(["cli", "spawn", "--cwd", cwd])
        .parse()
        .unwrap_or_else(|_| panic("failed to parse `wezterm cli spawn` output"));
    let pane = Pane::get_by_id(pane_id).unwrap_or_else(|| {
        panic(&format!(
            "wezterm pane returned by spawn not found: {pane_id}"
        ))
    });
    wezterm([
        "cli",
        "set-tab-title",
        "--pane-id",
        &pane_id.to_string(),
        title,
    ]);
    pane
}

fn list_panes() -> Vec<Pane> {
    let output = Command::new("wezterm")
        .args(&["cli", "list", "--format", "json"])
        .output()
        .unwrap_or_else(|err| panic(&format!("Failed to execute wezterm command: {err}")));

    let json_output = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(&json_output)
        .unwrap_or_else(|err| (panic(&format!("Failed to parse JSON: {err}\n{json_output}"))))
}

impl Pane {
    fn get_by_id(pane_id: u32) -> Option<Pane> {
        for p in list_panes() {
            if p.pane_id == pane_id {
                return Some(p);
            }
        }
        None
    }

    fn get_first_by_tab_title(title: &str) -> Option<Pane> {
        for p in list_panes() {
            eprintln!("looking for __{title}__, see __{}__", p.tab_title);
            if p.tab_title == title {
                return Some(p);
            }
        }
        warn(&format!("did not find pane with tab title: {title}"));
        None
    }
}

pub fn wezterm<I, S>(args: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new("wezterm")
        .args(args)
        .output()
        .unwrap_or_else(|_| panic("failed to execute wezterm"));
    let stdout = str::from_utf8(&output.stdout)
        .unwrap_or_else(|_| panic("failed to parse stdout"))
        .trim_end()
        .to_string();
    assert!(output.stderr.is_empty());
    stdout
}
