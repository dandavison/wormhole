use crate::handlers::doctor::{
    CloseWindowsResult, ConformResult, EditorWindowsReport, PersistedDataReport,
};

use super::util::Client;

pub(super) fn doctor_persisted_data(client: &Client, output: &str) -> Result<(), String> {
    let response = client.get("/doctor/persisted-data")?;
    if output == "json" {
        println!("{}", response);
    } else {
        let report: PersistedDataReport =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        println!("{}", report.render_terminal());
    }
    Ok(())
}

pub(super) fn doctor_list_editor_windows(client: &Client, output: &str) -> Result<(), String> {
    let response = client.get("/doctor/editor-windows")?;
    if output == "json" {
        println!("{}", response);
    } else {
        let report: EditorWindowsReport =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        println!("{}", report.render_terminal());
    }
    Ok(())
}

pub(super) fn doctor_close_editor_windows(
    client: &Client,
    keys: Vec<String>,
    stranded: bool,
    dry_run: bool,
    output: &str,
) -> Result<(), String> {
    if keys.is_empty() && !stranded {
        return Err("Specify project keys or --stranded".into());
    }
    let body = serde_json::json!({ "keys": keys, "stranded": stranded, "dry_run": dry_run });
    let response = client.post_json("/doctor/close-editor-windows", &body)?;
    if output == "json" {
        println!("{}", response);
    } else {
        let result: CloseWindowsResult =
            serde_json::from_str(&response).map_err(|e| e.to_string())?;
        println!("{}", result.render_terminal());
    }
    Ok(())
}

pub(super) fn doctor_conform(client: &Client, dry_run: bool, output: &str) -> Result<(), String> {
    let path = if dry_run {
        "/doctor/conform?dry-run=true"
    } else {
        "/doctor/conform"
    };
    let response = client.post(path)?;
    if output == "json" {
        println!("{}", response);
    } else {
        let result: ConformResult = serde_json::from_str(&response).map_err(|e| e.to_string())?;
        println!("{}", result.render_terminal());
    }
    Ok(())
}
