use std::path::PathBuf;

pub fn info(msg: &str) {
    println!("    {}", msg)
}

pub fn warn(msg: &str) {
    eprintln!("    WARNING: {}", msg)
}

pub fn error(msg: &str) {
    eprintln!("    ERROR: {}", msg)
}

pub fn expand_user(path: &str) -> String {
    path.replacen("~", &home_dir().to_str().unwrap(), 1)
}

pub fn contract_user(path: &str) -> String {
    path.replacen(&home_dir().to_str().unwrap(), "~", 1)
}

pub fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| panic!("Cannot determine home directory"))
}
