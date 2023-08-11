pub fn info(msg: &str) {
    println!("    {}", msg)
}

pub fn warn(msg: &str) {
    eprintln!("    WARNING: {}", msg)
}

pub fn error(msg: &str) {
    eprintln!("    ERROR: {}", msg)
}
