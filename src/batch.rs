use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::SystemTime;
use tokio::sync::watch;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

lazy_static! {
    static ref STORE: Mutex<Vec<Batch>> = Mutex::new(Vec::new());
    static ref VERSION: (watch::Sender<u64>, watch::Receiver<u64>) = watch::channel(0);
}

pub fn notify_change() {
    VERSION.0.send_modify(|v| *v = v.wrapping_add(1));
}

pub fn subscribe() -> watch::Receiver<u64> {
    VERSION.1.clone()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RunStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize)]
pub struct Run {
    pub key: String,
    pub dir: PathBuf,
    pub status: RunStatus,
    pub exit_code: Option<i32>,
    pub stdout_path: PathBuf,
    pub stderr_path: PathBuf,
    #[serde(skip)]
    pub pid: Option<u32>,
    pub started_at: Option<SystemTime>,
    pub finished_at: Option<SystemTime>,
}

#[derive(Debug, Serialize)]
pub struct Batch {
    pub id: String,
    pub command: Vec<String>,
    pub created_at: SystemTime,
    pub runs: Vec<Run>,
}

impl Batch {
    pub fn completed_count(&self) -> usize {
        self.runs
            .iter()
            .filter(|r| {
                matches!(
                    r.status,
                    RunStatus::Succeeded | RunStatus::Failed | RunStatus::Cancelled
                )
            })
            .count()
    }

    pub fn is_done(&self) -> bool {
        self.completed_count() == self.runs.len()
    }
}

#[derive(Deserialize)]
pub struct RunSpec {
    pub key: String,
    pub dir: PathBuf,
}

#[derive(Deserialize)]
pub struct BatchRequest {
    pub command: Vec<String>,
    pub runs: Vec<RunSpec>,
}

pub struct Store<'a>(MutexGuard<'a, Vec<Batch>>);

pub fn lock() -> Store<'static> {
    Store(STORE.lock().unwrap())
}

impl<'a> Store<'a> {
    pub fn get(&self, id: &str) -> Option<&Batch> {
        self.0.iter().find(|b| b.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Batch> {
        self.0.iter_mut().find(|b| b.id == id)
    }

    pub fn all(&self) -> &[Batch] {
        &self.0
    }

    pub fn insert(&mut self, batch: Batch) {
        self.0.push(batch);
    }
}

// -- API response types --

#[derive(Serialize, Deserialize)]
pub struct RunResponse {
    pub key: String,
    pub dir: PathBuf,
    pub status: RunStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct BatchResponse {
    pub id: String,
    pub command: Vec<String>,
    pub created_at: f64,
    pub total: usize,
    pub completed: usize,
    pub done: bool,
    pub runs: Vec<RunResponse>,
}

#[derive(Serialize, Deserialize)]
pub struct BatchListResponse {
    pub batches: Vec<BatchSummary>,
}

#[derive(Serialize, Deserialize)]
pub struct BatchSummary {
    pub id: String,
    pub command: Vec<String>,
    pub created_at: f64,
    pub total: usize,
    pub completed: usize,
    pub done: bool,
}

fn system_time_to_epoch(t: SystemTime) -> f64 {
    t.duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

impl Batch {
    pub fn to_response(&self) -> BatchResponse {
        BatchResponse {
            id: self.id.clone(),
            command: self.command.clone(),
            created_at: system_time_to_epoch(self.created_at),
            total: self.runs.len(),
            completed: self.completed_count(),
            done: self.is_done(),
            runs: self.runs.iter().map(Run::to_response).collect(),
        }
    }

    pub fn to_summary(&self) -> BatchSummary {
        BatchSummary {
            id: self.id.clone(),
            command: self.command.clone(),
            created_at: system_time_to_epoch(self.created_at),
            total: self.runs.len(),
            completed: self.completed_count(),
            done: self.is_done(),
        }
    }
}

impl Run {
    fn to_response(&self) -> RunResponse {
        let is_done = matches!(
            self.status,
            RunStatus::Succeeded | RunStatus::Failed | RunStatus::Cancelled
        );
        RunResponse {
            key: self.key.clone(),
            dir: self.dir.clone(),
            status: self.status,
            exit_code: self.exit_code,
            started_at: self.started_at.map(system_time_to_epoch),
            finished_at: self.finished_at.map(system_time_to_epoch),
            stdout: if is_done {
                fs::read_to_string(&self.stdout_path).ok()
            } else {
                None
            },
            stderr: if is_done {
                fs::read_to_string(&self.stderr_path).ok()
            } else {
                None
            },
        }
    }
}

impl BatchResponse {
    pub fn render_terminal(&self) -> String {
        let mut sorted: Vec<&RunResponse> = self.runs.iter().collect();
        sorted.sort_by(|a, b| a.key.cmp(&b.key));
        let mut out = String::new();
        for run in &sorted {
            let status_str = match run.status {
                RunStatus::Failed => {
                    let code = run
                        .exit_code
                        .map(|c| format!(" (exit {})", c))
                        .unwrap_or_default();
                    format!(" FAILED{}", code)
                }
                RunStatus::Cancelled => " CANCELLED".to_string(),
                _ => String::new(),
            };
            out.push_str(&format!("## {}{}\n", run.key, status_str));
            if let Some(ref s) = run.stdout {
                if !s.is_empty() {
                    out.push_str(s);
                    if !s.ends_with('\n') {
                        out.push('\n');
                    }
                }
            }
            if let Some(ref s) = run.stderr {
                if !s.is_empty() {
                    out.push_str(s);
                    if !s.ends_with('\n') {
                        out.push('\n');
                    }
                }
            }
            out.push('\n');
        }
        let failed = sorted
            .iter()
            .filter(|r| r.status == RunStatus::Failed)
            .count();
        let cancelled = sorted
            .iter()
            .filter(|r| r.status == RunStatus::Cancelled)
            .count();
        if failed > 0 || cancelled > 0 {
            let succeeded = sorted
                .iter()
                .filter(|r| r.status == RunStatus::Succeeded)
                .count();
            out.push_str(&format!(
                "{}/{} succeeded, {} failed, {} cancelled\n",
                succeeded, self.total, failed, cancelled
            ));
        }
        out
    }
}

impl BatchListResponse {
    pub fn render_terminal(&self) -> String {
        if self.batches.is_empty() {
            return "No batches\n".to_string();
        }
        let mut out = String::new();
        for b in &self.batches {
            let status_str = if b.done { "done" } else { "running" };
            out.push_str(&format!(
                "{} ({}/{}) [{}] {}\n",
                b.id,
                b.completed,
                b.total,
                status_str,
                b.command.join(" ")
            ));
        }
        out
    }
}

/// Create a new batch from a request, returning the batch ID.
/// Does not start execution — call `spawn_batch` after.
pub fn create_batch(req: BatchRequest) -> String {
    let id = format!("b{}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
    let output_dir =
        std::env::temp_dir().join(format!("wormhole-batch-{}-{}", std::process::id(), id));
    let _ = fs::create_dir_all(&output_dir);

    let runs = req
        .runs
        .into_iter()
        .enumerate()
        .map(|(i, spec)| Run {
            key: spec.key,
            dir: spec.dir,
            status: RunStatus::Pending,
            exit_code: None,
            stdout_path: output_dir.join(format!("{}.stdout", i)),
            stderr_path: output_dir.join(format!("{}.stderr", i)),
            pid: None,
            started_at: None,
            finished_at: None,
        })
        .collect();

    let batch = Batch {
        id: id.clone(),
        command: req.command,
        created_at: SystemTime::now(),
        runs,
    };
    lock().insert(batch);
    id
}

/// Spawn all runs in a batch. Each run gets its own thread.
pub fn spawn_batch(batch_id: &str) {
    let store = lock();
    let batch = match store.get(batch_id) {
        Some(b) => b,
        None => return,
    };

    let command = batch.command.clone();
    let run_specs: Vec<(usize, PathBuf, PathBuf, PathBuf)> = batch
        .runs
        .iter()
        .enumerate()
        .map(|(i, r)| {
            (
                i,
                r.dir.clone(),
                r.stdout_path.clone(),
                r.stderr_path.clone(),
            )
        })
        .collect();
    let id = batch_id.to_string();
    drop(store);

    for (idx, dir, stdout_path, stderr_path) in run_specs {
        let cmd = command.clone();
        let batch_id = id.clone();
        std::thread::spawn(move || {
            run_command(&batch_id, idx, &cmd, &dir, &stdout_path, &stderr_path)
        });
    }
}

fn run_command(
    batch_id: &str,
    idx: usize,
    command: &[String],
    dir: &PathBuf,
    stdout_path: &PathBuf,
    stderr_path: &PathBuf,
) {
    // Mark running
    {
        let mut store = lock();
        if let Some(batch) = store.get_mut(batch_id) {
            let run = &mut batch.runs[idx];
            if run.status == RunStatus::Cancelled {
                return;
            }
            run.status = RunStatus::Running;
            run.started_at = Some(SystemTime::now());
        }
        notify_change();
    }

    let stdout_file = fs::File::create(stdout_path).ok();
    let stderr_file = fs::File::create(stderr_path).ok();

    let shell_cmd = shell_command_line(command);
    let result = std::process::Command::new("sh")
        .args(["-c", &shell_cmd])
        .current_dir(dir)
        .stdout(
            stdout_file
                .map(std::process::Stdio::from)
                .unwrap_or(std::process::Stdio::null()),
        )
        .stderr(
            stderr_file
                .map(std::process::Stdio::from)
                .unwrap_or(std::process::Stdio::null()),
        )
        .spawn();

    match result {
        Ok(mut child) => {
            {
                let mut store = lock();
                if let Some(batch) = store.get_mut(batch_id) {
                    batch.runs[idx].pid = Some(child.id());
                }
            }

            let exit = child.wait();
            let mut store = lock();
            if let Some(batch) = store.get_mut(batch_id) {
                let run = &mut batch.runs[idx];
                run.finished_at = Some(SystemTime::now());
                run.pid = None;
                match exit {
                    Ok(status) => {
                        run.exit_code = status.code();
                        if run.status == RunStatus::Running {
                            run.status = if status.success() {
                                RunStatus::Succeeded
                            } else {
                                RunStatus::Failed
                            };
                        }
                    }
                    Err(e) => {
                        if run.status == RunStatus::Running {
                            run.status = RunStatus::Failed;
                        }
                        let _ = fs::write(stderr_path, format!("wait error: {}\n", e));
                    }
                }
            }
            notify_change();
        }
        Err(e) => {
            let _ = fs::write(stderr_path, format!("spawn error: {}\n", e));
            let mut store = lock();
            if let Some(batch) = store.get_mut(batch_id) {
                let run = &mut batch.runs[idx];
                run.status = RunStatus::Failed;
                run.finished_at = Some(SystemTime::now());
            }
            notify_change();
        }
    }
}

/// Build a string to pass to `sh -c`. Single-element commands are passed
/// verbatim (the user supplied a shell command string). Multi-element
/// commands have each arg shell-escaped so word boundaries are preserved.
fn shell_command_line(command: &[String]) -> String {
    if command.len() == 1 {
        return command[0].clone();
    }
    command
        .iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ")
}

fn shell_escape(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_alphanumeric() || "-_./=:@%+,".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Cancel a batch: SIGTERM running processes, mark pending/running as Cancelled.
pub fn cancel_batch(batch_id: &str) -> bool {
    let mut store = lock();
    let batch = match store.get_mut(batch_id) {
        Some(b) => b,
        None => return false,
    };
    for run in &mut batch.runs {
        match run.status {
            RunStatus::Pending => {
                run.status = RunStatus::Cancelled;
                run.finished_at = Some(SystemTime::now());
            }
            RunStatus::Running => {
                if let Some(pid) = run.pid {
                    unsafe {
                        libc::kill(pid as i32, libc::SIGTERM);
                    }
                }
                // Status will be updated to Cancelled when the process exits,
                // but mark it now so the API reflects it immediately.
                run.status = RunStatus::Cancelled;
            }
            _ => {}
        }
    }
    notify_change();
    true
}

/// Remove completed batches older than the given duration.
#[allow(dead_code)]
pub fn gc(max_age: std::time::Duration) {
    let cutoff = SystemTime::now() - max_age;
    let mut store = lock();
    store.0.retain(|batch| {
        if !batch.is_done() {
            return true;
        }
        if batch.created_at > cutoff {
            return true;
        }
        // Clean up output files and directory
        let mut output_dir = None;
        for run in &batch.runs {
            if output_dir.is_none() {
                output_dir = run.stdout_path.parent().map(|p| p.to_path_buf());
            }
            let _ = fs::remove_file(&run.stdout_path);
            let _ = fs::remove_file(&run.stderr_path);
        }
        if let Some(dir) = output_dir {
            let _ = fs::remove_dir(&dir);
        }
        false
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get_batch() {
        let req = BatchRequest {
            command: vec!["echo".into(), "hello".into()],
            runs: vec![
                RunSpec {
                    key: "proj-a".into(),
                    dir: "/tmp".into(),
                },
                RunSpec {
                    key: "proj-b".into(),
                    dir: "/tmp".into(),
                },
            ],
        };
        let id = create_batch(req);
        let store = lock();
        let batch = store.get(&id).unwrap();
        assert_eq!(batch.command, vec!["echo", "hello"]);
        assert_eq!(batch.runs.len(), 2);
        assert_eq!(batch.runs[0].status, RunStatus::Pending);
        assert_eq!(batch.runs[1].key, "proj-b");
        assert_eq!(batch.completed_count(), 0);
        assert!(!batch.is_done());
    }

    #[test]
    fn test_completed_count() {
        let req = BatchRequest {
            command: vec!["true".into()],
            runs: vec![
                RunSpec {
                    key: "a".into(),
                    dir: "/tmp".into(),
                },
                RunSpec {
                    key: "b".into(),
                    dir: "/tmp".into(),
                },
            ],
        };
        let id = create_batch(req);
        {
            let mut store = lock();
            let batch = store.get_mut(&id).unwrap();
            batch.runs[0].status = RunStatus::Succeeded;
            batch.runs[1].status = RunStatus::Running;
        }
        let store = lock();
        let batch = store.get(&id).unwrap();
        assert_eq!(batch.completed_count(), 1);
        assert!(!batch.is_done());
    }

    // Spawn/execution tests (real command, failed command, shell features,
    // bad command) are covered by integration tests in tests/test_batch.rs.

    #[test]
    fn test_gc_removes_old_batches() {
        let req = BatchRequest {
            command: vec!["true".into()],
            runs: vec![RunSpec {
                key: "gc-test".into(),
                dir: "/tmp".into(),
            }],
        };
        let id = create_batch(req);
        {
            let mut store = lock();
            let batch = store.get_mut(&id).unwrap();
            batch.runs[0].status = RunStatus::Succeeded;
            // Backdate creation
            batch.created_at = std::time::SystemTime::UNIX_EPOCH;
        }
        gc(std::time::Duration::from_secs(1));
        let store = lock();
        assert!(store.get(&id).is_none(), "old batch should be evicted");
    }

    #[test]
    fn test_cancel_batch() {
        let req = BatchRequest {
            command: vec!["sleep".into(), "999".into()],
            runs: vec![
                RunSpec {
                    key: "a".into(),
                    dir: "/tmp".into(),
                },
                RunSpec {
                    key: "b".into(),
                    dir: "/tmp".into(),
                },
            ],
        };
        let id = create_batch(req);
        {
            let mut store = lock();
            let batch = store.get_mut(&id).unwrap();
            batch.runs[0].status = RunStatus::Running;
            // runs[1] stays Pending
        }
        assert!(cancel_batch(&id));
        let store = lock();
        let batch = store.get(&id).unwrap();
        assert_eq!(batch.runs[0].status, RunStatus::Cancelled);
        assert_eq!(batch.runs[1].status, RunStatus::Cancelled);
    }

    #[test]
    fn test_render_terminal_shows_failure_info() {
        let batch = BatchResponse {
            id: "b1".into(),
            command: vec!["test".into()],
            created_at: 0.0,
            total: 2,
            completed: 2,
            done: true,
            runs: vec![
                RunResponse {
                    key: "alpha".into(),
                    dir: "/tmp".into(),
                    status: RunStatus::Succeeded,
                    exit_code: Some(0),
                    started_at: Some(0.0),
                    finished_at: Some(1.0),
                    stdout: Some("ok\n".into()),
                    stderr: None,
                },
                RunResponse {
                    key: "beta".into(),
                    dir: "/tmp".into(),
                    status: RunStatus::Failed,
                    exit_code: Some(127),
                    started_at: Some(0.0),
                    finished_at: Some(1.0),
                    stdout: None,
                    stderr: Some("sh: bad_cmd: command not found\n".into()),
                },
            ],
        };
        let out = batch.render_terminal();
        assert!(
            out.contains("## alpha\n"),
            "succeeded run has plain heading"
        );
        assert!(
            out.contains("## beta FAILED (exit 127)\n"),
            "failed run shows FAILED and exit code"
        );
        assert!(
            out.contains("command not found"),
            "stderr from failed run is shown"
        );
        assert!(
            out.contains("1/2 succeeded, 1 failed"),
            "summary line present"
        );
    }

    #[test]
    fn test_render_terminal_failed_no_exit_code() {
        let batch = BatchResponse {
            id: "b1".into(),
            command: vec!["test".into()],
            created_at: 0.0,
            total: 1,
            completed: 1,
            done: true,
            runs: vec![RunResponse {
                key: "proj".into(),
                dir: "/tmp".into(),
                status: RunStatus::Failed,
                exit_code: None,
                started_at: Some(0.0),
                finished_at: Some(1.0),
                stdout: None,
                stderr: Some("spawn error: No such file or directory\n".into()),
            }],
        };
        let out = batch.render_terminal();
        assert!(out.contains("## proj FAILED\n"), "FAILED without exit code");
        assert!(out.contains("spawn error"), "spawn error shown");
    }
}
