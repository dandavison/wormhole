use std::{
    ffi::OsStr,
    fmt::Display,
    path::{Path, PathBuf},
    process::Command,
    str,
};

pub fn info(msg: &str) {
    println!("    {}", msg)
}

pub fn warn(msg: &str) {
    let msg = format!("    WARNING: {}", msg);
    notify(&msg);
    eprintln!("{}", msg);
}

pub fn error(msg: &str) {
    let msg = format!("    ERROR: {}", msg);
    notify(&msg);
    eprintln!("{}", msg)
}

pub fn panic(msg: &str) -> ! {
    let msg = format!("    PANIC: {}", msg);
    notify(&msg);
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

pub fn notify(msg: &str) {
    Command::new("terminal-notifier")
        .args(&["-message", msg, "-title", "wormhole"])
        .output()
        .unwrap_or_else(|_| panic!("failed to execute terminal-notifier"));
}

pub fn execute_command<I, S, P>(program: S, args: I, current_dir: P) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
    S: Display,
    P: AsRef<Path>,
{
    let output = Command::new(program)
        .args(args)
        .current_dir(current_dir)
        .output()
        .unwrap_or_else(|_| panic(&format!("failed to execute")));
    let stdout = str::from_utf8(&output.stdout)
        .unwrap_or_else(|_| panic("failed to parse stdout"))
        .trim_end()
        .to_string();
    assert!(output.stderr.is_empty());
    stdout
}
