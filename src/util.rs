use core::{panic, str};
use std::{
    env,
    ffi::OsStr,
    fmt::{Debug, Display},
    path::Path,
    process::{Command, Output, Stdio},
    sync::mpsc,
    thread,
    time::Duration,
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
    let _ = Command::new("terminal-notifier")
        .args(["-message", msg, "-title", "wormhole"])
        .spawn()
        .map(|mut child| child.wait());
}

pub fn execute_command<S, I, P>(program: S, args: I, current_dir: P) -> Result<String, String>
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
        .map_err(|e| format!("failed to execute {program}: {e}"))?;
    get_stdout(program, output)
}

pub fn to_kebab_case(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() || c == '-' || c == '_' {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Run `cmd` with no stdin, killing it (and anything it spawned) if it has not
/// finished within `timeout`.
pub fn output_with_timeout(
    cmd: &mut Command,
    timeout: Duration,
    what: &str,
) -> Result<Output, String> {
    use std::os::unix::process::CommandExt;

    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        cmd.pre_exec(|| {
            if libc::setpgid(0, 0) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let child = cmd.spawn().map_err(|e| format!("{what} failed: {e}"))?;
    let pid = child.id() as i32;
    let (tx, rx) = mpsc::channel();
    let handle = thread::spawn(move || {
        let result = child.wait_with_output();
        let _ = tx.send(result);
    });

    let result = match rx.recv_timeout(timeout) {
        Ok(result) => result.map_err(|e| format!("{what} failed: {e}")),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            kill_process(pid, libc::SIGTERM);
            kill_process_group(pid, libc::SIGTERM);
            if matches!(
                rx.recv_timeout(Duration::from_millis(200)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ) {
                kill_process(pid, libc::SIGKILL);
                kill_process_group(pid, libc::SIGKILL);
            }
            let _ = handle.join();
            return Err(format!("{what} timed out after {}s", timeout.as_secs()));
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            return Err(format!("{what} failed: process watcher disconnected"));
        }
    };
    let _ = handle.join();
    result
}

fn kill_process(pid: i32, signal: i32) {
    unsafe {
        libc::kill(pid, signal);
    }
}

fn kill_process_group(pid: i32, signal: i32) {
    unsafe {
        libc::kill(-pid, signal);
    }
}

pub fn get_stdout<S>(program: S, output: Output) -> Result<String, String>
where
    S: AsRef<OsStr>,
    S: Display,
{
    let stdout = str::from_utf8(&output.stdout)
        .map_err(|e| format!("failed to parse stdout from {program}: {e}"))?
        .trim_end()
        .to_string();
    if !output.stderr.is_empty() {
        let stderr = str::from_utf8(&output.stderr)
            .map_err(|e| format!("failed to parse stderr from {program}: {e}"))?;
        return Err(format!(
            "program {program} produced output on stderr: {stderr}"
        ));
    }
    Ok(stdout)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn output_with_timeout_returns_output() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "printf ok"]);
        let output = output_with_timeout(&mut cmd, Duration::from_secs(1), "test").unwrap();
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "ok");
    }

    #[test]
    fn output_with_timeout_times_out() {
        let start = Instant::now();
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 30"]);
        let err = output_with_timeout(&mut cmd, Duration::from_millis(100), "test").unwrap_err();
        assert!(err.contains("timed out"), "unexpected error: {err}");
        assert!(
            start.elapsed() < Duration::from_secs(2),
            "timed-out subprocess should be reaped promptly"
        );
    }
}
