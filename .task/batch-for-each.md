# `wormhole project for-each` via server-managed batch execution

## Data model

A new `src/batch.rs` with in-memory state, following the `projects.rs` pattern
(`lazy_static! + Mutex + guard wrapper`).

**Run**: a single command execution in a directory.

```rust
struct Run {
    key: String,             // opaque label (CLI sets this to project_key)
    dir: PathBuf,
    status: RunStatus,       // Pending, Running, Succeeded, Failed, Cancelled
    exit_code: Option<i32>,
    stdout_path: PathBuf,    // temp file
    stderr_path: PathBuf,    // temp file
    pid: Option<u32>,
    started_at: Option<SystemTime>,
    finished_at: Option<SystemTime>,
}
```

**Batch**: a group of runs.

```rust
struct Batch {
    id: String,              // short random ID
    command: Vec<String>,
    created_at: SystemTime,
    runs: Vec<Run>,
}
```

All timestamps are `SystemTime` (serializable, meaningful across machines).

Output is captured to temp files (one stdout + one stderr per run). The server
reads these on demand when the CLI queries results.

Completed batches are cleaned up by a GC abstraction (e.g. `wormhole doctor gc`
or a periodic server-side sweep). For now, a simple `batch::gc()` function that
evicts completed batches older than 1 hour, callable from a future `doctor gc`
command or triggered lazily.

## Server: generic batch execution

The server has no concept of "for-each" or projects. It accepts and executes
batches of commands.

### Endpoints

- `POST /batch` -- body: `{"command": [...], "runs": [{"key": "...", "dir":
  "..."}, ...]}`. Creates a batch, spawns all commands in background threads,
  returns batch ID and initial status JSON.
- `GET /batch/<id>` -- returns full batch status (each run's state, exit code,
  timestamps). Supports long-poll via `Prefer: wait=N` header. If
  `?output=true`, includes stdout/stderr file contents.
- `GET /batch` -- lists all batches (summary: id, command, counts by status).
- `POST /batch/<id>/cancel` -- kills all running processes (SIGTERM), marks
  pending runs as Cancelled, marks running runs as Cancelled once they exit.

### Execution

Each run is spawned via `std::thread::spawn` + `std::process::Command` with
stdout/stderr redirected to files. On completion, the thread acquires the lock,
updates the run's status/exit code, and calls `notify_batch_change()`.

### Long-poll mechanism

Follow the existing pattern in `src/handlers/project.rs` (`poll_until` +
`tokio::sync::watch`):

- `src/batch.rs` owns a `watch` channel (like `STATE_VERSION` in `projects.rs`)
- `notify_batch_change()` fires whenever a run completes
- The `GET /batch/<id>` handler uses `poll_until()` with a predicate comparing
  completed-run count against the client's last-seen count (passed as
  `?completed=N` query param)
- The CLI long-polls in a loop: send completed count, block until server returns
  with new completions, print them, repeat

`poll_until` currently lives in `src/handlers/project.rs` but is generic. It
depends on `projects::subscribe_to_changes()`. Generalize it to accept a
`watch::Receiver` as argument so both projects and batch can use it.

## CLI

`wormhole project for-each [--active] [--status] [-o json|text] <command...>`

- With `--status` (no command): `GET /batch`, display summary of all batches.
- With a command: fetch project list from `/project/list`, build batch request,
  `POST /batch` to start, then long-poll `GET /batch/<id>?completed=N` printing
  results as runs complete. Ctrl-C detaches the CLI but runs continue
  server-side.
- Both modes support `--output json` and pretty text.

## Design rationale

- **Server is generic**: the batch API knows nothing about projects or for-each.
  The CLI resolves projects to (key, dir) pairs and submits them. This keeps the
  server's concerns minimal and makes the batch API reusable.
- **Subprocesses, not tmux**: output is captured to files via
  `std::process::Command`. Tmux is a viewing layer that can be added later, not
  the execution mechanism. This gives clean stdout/stderr separation and
  structured exit codes.
- **Remote-capable**: all execution goes through the HTTP API. A phone client or
  remote CLI can start batches, poll status, and read output. The temp files are
  an internal server detail; output is served over HTTP.
- **Agent-ready**: the batch API is command-agnostic. Today it runs `git status`;
  tomorrow it runs `claude --print "fix the tests"`. Agent-specific concerns
  (prompt, model, tools) are encoded in the command string, not in wormhole.

## What is intentionally deferred

- Concurrency limits (spawns all at once for now)
- Dashboard integration
- Tmux pane viewing / attach
- Retry / re-run of individual failed runs
- Persistence across server restarts
