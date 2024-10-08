use core::{panic, str};
use std::{
    ffi::OsStr,
    fmt::{Debug, Display},
    path::{Path, PathBuf},
    process::Command,
};

use crate::ps;

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

pub fn expand_user(path: &str) -> String {
    path.replacen("~", &home_dir().to_str().unwrap(), 1)
}

pub fn contract_user(path: &str) -> String {
    path.replacen(&home_dir().to_str().unwrap(), "~", 1)
}

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| panic("Cannot determine home directory"))
}

pub fn desktop_notification(msg: &str) {
    execute_command(
        "terminal-notifier",
        ["-message", msg, "-title", "wormhole"],
        "/tmp",
    );
}

pub fn execute_command<I, S, P>(program: S, args: I, current_dir: P) -> String
where
    I: IntoIterator<Item = S>,
    I: Debug,
    P: Debug,
    S: AsRef<OsStr>,
    S: Copy,
    S: Display,
    P: AsRef<Path>,
{
    ps!("execute_command({program}, {args:?}, {current_dir:?})");
    let output = Command::new(program)
        .args(args)
        .current_dir(current_dir)
        .output()
        .unwrap_or_else(|_| panic(&format!("failed to execute {program}")));
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
