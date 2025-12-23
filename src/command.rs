use std::env;
use std::ffi::OsStr;
use std::fmt::{Debug, Display};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use serde_json;

use crate::ps;

/// Core command execution modes
#[derive(Debug, Clone)]
pub enum ExecutionMode {
    /// Wait for completion and capture output
    Output,
    /// Spawn and don't wait (fire and forget)
    Spawn,
}

#[derive(Debug)]
pub struct CommandResult {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    #[allow(dead_code)]
    pub status: Option<i32>,
}

/// Single point of command execution for the entire application.
/// In test mode (WORMHOLE_TEST_MODE=capture), logs commands instead of executing them.
pub fn execute_raw<S, I, P>(
    program: S,
    args: I,
    current_dir: Option<P>,
    mode: ExecutionMode,
) -> Option<CommandResult>
where
    S: AsRef<OsStr> + Display + Debug + Copy,
    I: IntoIterator<Item = S> + Debug,
    P: AsRef<Path> + Debug,
{
    let args_vec: Vec<S> = args.into_iter().collect();

    ps!(
        "execute_raw({}, {:?}, {:?}, {:?})",
        program,
        args_vec,
        current_dir,
        mode
    );

    // Test mode: capture ALL commands instead of executing
    if let Ok(capture_file) = env::var("WORMHOLE_TEST_MODE") {
        capture_command_for_test(
            &program,
            &args_vec,
            current_dir.as_ref(),
            &mode,
            &capture_file,
        );
        // Return mock successful result
        return Some(CommandResult {
            stdout: Vec::new(),
            stderr: Vec::new(),
            status: Some(0),
        });
    }

    // Integration test mode: intercept editor commands only
    if env::var("WORMHOLE_INTEGRATION_TEST_MODE").is_ok() {
        // Replace editor-related commands with no-ops
        let prog_str = program.to_string();
        if prog_str == "open"
            || prog_str == "test-editor"
            || prog_str == "cursor"
            || prog_str == "code"
            || prog_str == "emacsclient"
        {
            // Log but don't execute editor commands
            ps!(
                "Integration test: skipping editor command: {} {:?}",
                program,
                args_vec
            );
            return Some(CommandResult {
                stdout: Vec::new(),
                stderr: Vec::new(),
                status: Some(0),
            });
        }
        // All other commands (including tmux) execute normally
    }

    let mut cmd = Command::new(program.as_ref());
    cmd.args(args_vec.iter().map(|a| a.as_ref()));

    if let Some(dir) = current_dir {
        cmd.current_dir(dir);
    }

    match mode {
        ExecutionMode::Output => match cmd.output() {
            Ok(output) => Some(CommandResult {
                stdout: output.stdout,
                stderr: output.stderr,
                status: output.status.code(),
            }),
            Err(e) => {
                crate::util::panic(&format!("Failed to execute {}: {}", program, e));
            }
        },
        ExecutionMode::Spawn => {
            match cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
                Ok(_) => None, // Fire and forget
                Err(e) => {
                    crate::util::panic(&format!("Failed to spawn {}: {}", program, e));
                }
            }
        }
    }
}

fn capture_command_for_test<S, P>(
    program: &S,
    args: &[S],
    current_dir: Option<&P>,
    mode: &ExecutionMode,
    capture_file: &str,
) where
    S: AsRef<OsStr> + Display + Debug,
    P: AsRef<Path> + Debug,
{
    let entry = serde_json::json!({
        "program": program.to_string(),
        "args": args.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
        "cwd": current_dir.map(|d| d.as_ref().display().to_string()).unwrap_or_else(|| "/tmp".to_string()),
        "mode": match mode {
            ExecutionMode::Output => "output",
            ExecutionMode::Spawn => "spawn",
        }
    });

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(capture_file)
    {
        writeln!(file, "{}", entry.to_string()).ok();
    }
}

// ===== High-level convenience functions =====

/// Execute command and return stdout as String (original execute_command behavior)
pub fn execute_command<S, I, P>(program: S, args: I, current_dir: P) -> String
where
    S: AsRef<OsStr> + Display + Debug + Copy,
    I: IntoIterator<Item = S> + Debug,
    P: AsRef<Path> + Debug,
{
    if let Some(result) = execute_raw(program, args, Some(current_dir), ExecutionMode::Output) {
        if !result.stderr.is_empty() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            crate::util::panic(&format!(
                "program {} produced output on stderr: {}",
                program, stderr
            ));
        }
        String::from_utf8_lossy(&result.stdout)
            .trim_end()
            .to_string()
    } else {
        String::new()
    }
}

/// Execute command and return raw output (for hammerspoon which needs stderr)
pub fn execute_with_stderr<S, I>(program: S, args: I) -> CommandResult
where
    S: AsRef<OsStr> + Display + Debug + Copy,
    I: IntoIterator<Item = S> + Debug,
{
    execute_raw(program, args, None::<&Path>, ExecutionMode::Output).unwrap_or_else(|| {
        CommandResult {
            stdout: Vec::new(),
            stderr: Vec::new(),
            status: Some(1),
        }
    })
}

/// Spawn a command without waiting (for desktop notifications)
pub fn spawn_detached<S, I>(program: S, args: I)
where
    S: AsRef<OsStr> + Display + Debug + Copy,
    I: IntoIterator<Item = S> + Debug,
{
    execute_raw(program, args, None::<&Path>, ExecutionMode::Spawn);
}
