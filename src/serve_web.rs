use std::collections::HashMap;
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use lazy_static::lazy_static;

pub struct ServeWebInstance {
    #[allow(dead_code)]
    pub task_id: String,
    pub port: u16,
    #[allow(dead_code)]
    pub path: PathBuf,
    child: Child,
}

pub struct ServeWebManager {
    instances: HashMap<String, ServeWebInstance>,
    base_port: u16,
}

lazy_static! {
    static ref SERVE_WEB: Mutex<ServeWebManager> = Mutex::new(ServeWebManager::new(18000));
}

pub fn manager() -> std::sync::MutexGuard<'static, ServeWebManager> {
    SERVE_WEB.lock().unwrap()
}

impl ServeWebManager {
    pub fn new(base_port: u16) -> Self {
        Self {
            instances: HashMap::new(),
            base_port,
        }
    }

    pub fn get_or_start(&mut self, task_id: &str, path: &Path) -> Result<u16, String> {
        if let Some(instance) = self.instances.get_mut(task_id) {
            if is_running(&mut instance.child) {
                return Ok(instance.port);
            }
            self.instances.remove(task_id);
        }

        let port = self.port_for_task(task_id);

        let child =
            Command::new("/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code")
                .args([
                    "serve-web",
                    "--port",
                    &port.to_string(),
                    "--without-connection-token",
                    "--accept-server-license-terms",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| format!("Failed to start VS Code serve-web: {}", e))?;

        self.instances.insert(
            task_id.to_string(),
            ServeWebInstance {
                task_id: task_id.to_string(),
                port,
                path: path.to_path_buf(),
                child,
            },
        );

        wait_for_port(port)?;
        Ok(port)
    }

    pub fn stop(&mut self, task_id: &str) {
        if let Some(mut instance) = self.instances.remove(task_id) {
            let _ = instance.child.kill();
        }
    }

    pub fn stop_all(&mut self) {
        for (_, mut instance) in self.instances.drain() {
            let _ = instance.child.kill();
        }
    }

    fn port_for_task(&self, task_id: &str) -> u16 {
        let hash = task_id
            .bytes()
            .fold(0u32, |acc, b| acc.wrapping_add(b as u32));
        self.base_port + (hash % 1000) as u16
    }
}

fn is_running(child: &mut Child) -> bool {
    matches!(child.try_wait(), Ok(None))
}

fn wait_for_port(port: u16) -> Result<(), String> {
    let start = Instant::now();
    let timeout = Duration::from_secs(10);
    let poll = Duration::from_millis(100);

    while start.elapsed() < timeout {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Ok(());
        }
        std::thread::sleep(poll);
    }
    Err(format!(
        "VSCode server failed to start on port {} within 10s",
        port
    ))
}

impl Drop for ServeWebManager {
    fn drop(&mut self) {
        self.stop_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_for_task_deterministic() {
        let mgr = ServeWebManager::new(18000);
        let port1 = mgr.port_for_task("ACT-123");
        let port2 = mgr.port_for_task("ACT-123");
        assert_eq!(port1, port2);
    }

    #[test]
    fn test_port_for_task_different_tasks() {
        let mgr = ServeWebManager::new(18000);
        let port1 = mgr.port_for_task("ACT-123");
        let port2 = mgr.port_for_task("ACT-456");
        assert_ne!(port1, port2);
    }

    #[test]
    fn test_port_for_task_in_range() {
        let mgr = ServeWebManager::new(18000);
        for task in ["ACT-1", "ACT-999", "LONG-TASK-NAME-12345"] {
            let port = mgr.port_for_task(task);
            assert!(port >= 18000 && port < 19000, "port {} out of range", port);
        }
    }
}
