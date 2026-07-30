use std::io::Read;
use std::process::{Command, Stdio};
use std::str;
use std::thread;
use std::time::{Duration, Instant};

use crate::util::{error, panic, warn};
use crate::wormhole::Application;
use crate::{config, ps};

pub fn current_application() -> Application {
    match str::from_utf8(&execute(
        r#"
        local focusedWindow = hs.window.focusedWindow()
        if focusedWindow then
            local app = focusedWindow:application()
            if app then
                print(app:title())
            end
        end
    "#,
    ))
    .map(str::trim)
    {
        Ok(app_title) => {
            if app_title == config::TERMINAL.application_name() {
                Application::Terminal
            } else {
                Application::Editor
            }
        }
        Err(err) => {
            warn(&format!("current_application() ERROR: {err}"));
            Application::Editor
        }
    }
}

pub fn launch_or_focus(application_name: &str) {
    if crate::util::debug() {
        ps!("launch_or_focus({application_name})");
    }
    execute(&format!(
        r#"
        hs.application.launchOrFocus("{application_name}")
    "#,
    ));
}

pub fn alert(message: &str) {
    execute(&format!(r#"hs.alert.show("{message}", 0.5)"#,));
}

const HS_TIMEOUT: Duration = Duration::from_secs(5);

pub fn execute(lua: &str) -> Vec<u8> {
    let mut child = Command::new("hs")
        .arg("-c")
        .arg(lua)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|_| panic("Failed to execute hammerspoon"));

    // Drain pipes on threads so a chatty child can't deadlock on a full pipe.
    let mut stdout_pipe = child.stdout.take().unwrap();
    let mut stderr_pipe = child.stderr.take().unwrap();
    let stdout_reader = thread::spawn(move || read_all(&mut stdout_pipe));
    let stderr_reader = thread::spawn(move || read_all(&mut stderr_pipe));

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() >= HS_TIMEOUT => {
                let _ = child.kill();
                let _ = child.wait();
                error(&format!(
                    "hammerspoon (`hs`) did not respond within {}s — is Hammerspoon.app running with hs.ipc loaded? Launch it and retry.",
                    HS_TIMEOUT.as_secs()
                ));
                return Vec::new();
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(err) => {
                warn(&format!("hammerspoon wait() ERROR: {err}"));
                return Vec::new();
            }
        }
    }

    let stderr = stderr_reader.join().unwrap_or_default();
    for line in str::from_utf8(&stderr).unwrap_or("").split_terminator("\n") {
        error(line);
    }
    stdout_reader.join().unwrap_or_default()
}

fn read_all(pipe: &mut impl Read) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = pipe.read_to_end(&mut buf);
    buf
}
