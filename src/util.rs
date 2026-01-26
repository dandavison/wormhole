use core::{panic, str};
use std::{
    env,
    ffi::OsStr,
    fmt::{Debug, Display},
    path::Path,
    process::{Command, Output},
};

use crate::ps;

pub fn debug() -> bool {
    env::var("WORMHOLE_DEBUG").is_ok()
}

pub fn warn(msg: &str) {
    let msg = format!("WARNING: {}", msg);
    desktop_notification(&msg);
    eprintln!("{}", msg);
}

pub fn error(msg: &str) {
    let msg = format!("ERROR: {}", msg);
    desktop_notification(&msg);
    eprintln!("{}", msg)
}

pub fn panic(msg: &str) -> ! {
    let msg = format!("PANIC: {}", msg);
    desktop_notification(&msg);
    panic!("{}", msg)
}

pub fn desktop_notification(msg: &str) {
    Command::new("terminal-notifier")
        .args(["-message", msg, "-title", "wormhole"])
        .spawn()
        .unwrap_or_else(|err| panic(&format!("failed to spawn terminal-notifier: {err}")));
}

pub fn execute_command<S, I, P>(program: S, args: I, current_dir: P) -> String
where
    S: AsRef<OsStr>,
    I: IntoIterator<Item = S>,
    P: AsRef<Path>,
    S: Copy,
    S: Display,
    I: Debug,
    P: Debug,
{
    if debug() {
        ps!("execute_command({program}, {args:?}, {current_dir:?})");
    }
    let output = Command::new(program)
        .args(args)
        .current_dir(current_dir)
        .output()
        .unwrap_or_else(|_| panic(&format!("failed to execute {program}")));
    get_stdout(program, output)
}

pub fn get_stdout<S>(program: S, output: Output) -> String
where
    S: AsRef<OsStr>,
    S: Display,
{
    let stdout = str::from_utf8(&output.stdout)
        .unwrap_or_else(|err| panic(&format!("failed to parse stdout from {program}: {err}")))
        .trim_end()
        .to_string();
    if !output.stderr.is_empty() {
        let stderr = str::from_utf8(&output.stderr)
            .unwrap_or_else(|err| panic(&format!("failed to parse stderr from {program}: {err}")));
        panic(&format!(
            "program {program} produced output on stderr: {stderr}"
        ));
    }
    stdout
}

pub fn format_columns(rows: &[Vec<&str>]) -> Vec<String> {
    if rows.is_empty() {
        return vec![];
    }
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let widths: Vec<usize> = (0..num_cols)
        .map(|col| {
            rows.iter()
                .filter_map(|r| r.get(col))
                .map(|s| s.len())
                .max()
                .unwrap_or(0)
        })
        .collect();

    rows.iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(i, s)| {
                    if i < row.len() - 1 {
                        format!("{:width$}", s, width = widths[i])
                    } else {
                        s.to_string()
                    }
                })
                .collect::<Vec<_>>()
                .join("  ")
        })
        .collect()
}
