use crate::handlers::doctor::{ConformResult, MigrateResult, PersistedDataReport};

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

pub(super) fn doctor_migrate_worktrees(client: &Client) -> Result<(), String> {
    let response = client.post("/doctor/migrate-worktrees")?;
    let result: MigrateResult = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    println!("{}", result.render_terminal());
    Ok(())
}
