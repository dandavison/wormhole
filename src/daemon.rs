use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

pub struct TmuxSession {
    pub socket: String,
    pub session: String,
}

impl TmuxSession {
    pub fn new(socket: &str, session: &str) -> Self {
        Self {
            socket: socket.to_string(),
            session: session.to_string(),
        }
    }

    pub fn start(
        &self,
        binary: &str,
        port: Option<u16>,
        working_dir: Option<&str>,
        envs: &[(&str, &str)],
    ) -> Result<(), String> {
        self.stop();
        let mut cmd = Command::new("tmux");
        cmd.args(["-L", &self.socket, "new-session", "-d", "-s", &self.session]);
        if let Some(dir) = working_dir {
            cmd.args(["-c", dir]);
        }
        cmd.args([binary, "server", "start-foreground"]);
        if let Some(p) = port {
            cmd.env("WORMHOLE_PORT", p.to_string());
        }
        for (k, v) in envs {
            cmd.env(k, v);
        }
        let status = cmd
            .status()
            .map_err(|e| format!("Failed to start tmux: {}", e))?;
        if !status.success() {
            return Err(format!("tmux exited with status: {}", status));
        }
        Ok(())
    }

    pub fn stop(&self) {
        let _ = Command::new("tmux")
            .args(["-L", &self.socket, "kill-server"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    pub fn is_running(&self) -> bool {
        Command::new("tmux")
            .args(["-L", &self.socket, "has-session", "-t", &self.session])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn attach(&self) -> Result<(), String> {
        use std::os::unix::process::CommandExt;
        let err = Command::new("tmux")
            .args(["-L", &self.socket, "attach-session", "-t", &self.session])
            .exec();
        Err(format!("Failed to attach: {}", err))
    }
}

pub fn wait_for_ready(port: u16, timeout: Duration) -> bool {
    let start = Instant::now();
    let url = format!("http://127.0.0.1:{}/project/list", port);
    while start.elapsed() < timeout {
        if ureq::get(&url).call().is_ok() {
            return true;
        }
        thread::sleep(Duration::from_millis(250));
    }
    false
}

pub fn daemon() -> TmuxSession {
    TmuxSession::new("wormhole-daemon", "wormhole")
}
