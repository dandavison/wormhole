use std::{path::PathBuf, process::Command};

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
