---
name: Config and worktree relocation
overview: Introduce a global `~/.wormhole/config.toml` config file and relocate task worktrees from `$gitdir/wormhole/worktrees/` to a configurable external directory (defaulting to `~/worktrees/`).
todos:
  - id: global-config
    content: Introduce ~/.wormhole/config.toml with search_paths (including per-path excludes), replace WORMHOLE_PATH and per-directory .wormhole.toml
    status: pending
  - id: worktree-relocation
    content: Add worktree_dir config, change path formula to ~/worktrees/$repo/$branch/$repo, update discovery/project/task/doctor/kv
    status: pending
  - id: vscode-extension
    content: Update projectKeyFromPath() in VSCode extension for new path structure
    status: pending
  - id: migration
    content: Add wormhole doctor migrate-worktrees command using git worktree repair
    status: pending
  - id: tests-docs
    content: Update unit tests, integration tests, README, and research doc
    status: pending
isProject: false
---

# Global Config File and Worktree Relocation

## Design

### Config file: `~/.wormhole/config.toml`

```toml
# Directories to search for projects (replaces WORMHOLE_PATH env var).
# Plain strings and objects with per-path excludes can be mixed.
search_paths = [
    "~/src/temporal-all/repos",
    { path = "~/src", exclude = ["node_modules", "venv"] },
]

# Where task worktrees are created (default: ~/worktrees)
worktree_dir = "~/worktrees"
```

- Entries are either a plain string (path only) or an object with `path` and optional `exclude`. Implemented via `#[serde(untagged)]` enum.
- Env var names follow the convention `WORMHOLE_` + SCREAMING_SNAKE of the TOML key: `WORMHOLE_SEARCH_PATHS`, `WORMHOLE_WORKTREE_DIR`, `WORMHOLE_PORT`, `WORMHOLE_EDITOR`. Env vars override config file values when set. (`WORMHOLE_SEARCH_PATHS` is colon-separated; per-path excludes are a config-file-only feature.)
- The per-directory `.wormhole.toml` is removed; its `available.exclude` patterns move into the per-path `exclude` field above.
- `~` is expanded to `$HOME` at load time.

### New worktree path formula

```
$worktree_dir/$repo_name/$encoded_branch/$repo_name
```

Example: `~/worktrees/cli/standalone-activity-client/cli`

Grouped by repo first (easy to `ls ~/worktrees/cli/` to see all tasks), leaf is `$repo_name` for VSCode sidebar.

### Discovery change

Currently `discover_tasks()` filters `git worktree list` output by `wt.path.starts_with($gitdir/wormhole/worktrees)`. After the change, it filters by `wt.path.starts_with($worktree_dir/$repo_name)`. Same mechanism, different prefix.

### What stays in `$gitdir/wormhole/`

- `kv/` -- internal metadata, not user-facing
- `workspaces/` -- generated `.code-workspace` files, not user-facing

These are small JSON files that benefit from being co-located with the repo.

## Files to change

### 1. [src/config.rs](src/config.rs) -- new global config

- Add `GlobalConfig` struct with `search_paths` (Vec of path+exclude), `worktree_dir` (PathBuf).
- Load from `~/.wormhole/config.toml`, falling back to env vars.
- Replace the current `search_paths()` to check config file first, then `WORMHOLE_SEARCH_PATHS` (renamed from `WORMHOLE_PATH`).
- Add `worktree_dir()` returning the configured or default `~/worktrees`.
- Remove the CWD-based `.wormhole.toml` loading and its `WormholeConfig`/`AvailableConfig` structs.
- Migrate `is_excluded()` to use per-path excludes from the new config.

### 2. [src/git.rs](src/git.rs) -- worktree path functions

- `task_worktree_path()`: change signature to take `worktree_dir` instead of `git_common_dir`. New body: `worktree_dir.join(repo_name).join(encode_branch_for_path(branch)).join(repo_name)`.
- `worktree_base_path()`: change to take `worktree_dir` and `repo_name`, return `worktree_dir.join(repo_name)`. Or remove it and inline.
- `find_orphan_worktree_dirs()`: adapt to walk `$worktree_dir/$repo_name/` instead of `$gitdir/wormhole/worktrees/`.

### 3. [src/task.rs](src/task.rs) -- task creation/removal

- `create_task()` (line 169): use `config::worktree_dir()` instead of `git::git_common_dir()`.
- `remove_task()`: no change needed (already uses `project.worktree_path()`).

### 4. [src/project.rs](src/project.rs) -- `Project::worktree_path()`

- Change to use `config::worktree_dir()` instead of `self.cached.git_common_dir`. This means `worktree_path()` no longer depends on `git_common_dir`, simplifying the submodule case.

### 5. [src/projects.rs](src/projects.rs) -- task discovery

- `discover_tasks()` (line 346): filter worktrees by `config::worktree_dir().join(project_name)` prefix instead of `git_common_dir.join("wormhole/worktrees")`.

### 6. [src/kv.rs](src/kv.rs) -- discovery helper

- Line 185: update the worktree filter prefix in `load_kv_data_for_available_projects()`.

### 7. [src/handlers/doctor.rs](src/handlers/doctor.rs) -- conform and persisted-data

- Update `worktree_base` references to use new path.
- `find_orphan_worktree_dirs`: adapt to new directory structure.

### 8. [vscode-extension/src/project-key.ts](vscode-extension/src/project-key.ts)

- The current marker `/wormhole/worktrees/` won't appear in the new paths. Change detection to look for the `$worktree_dir` prefix, or (simpler) detect the `$repo/$branch/$repo` pattern where the first and last path segments match. The extension can read the worktree dir from the `wormhole.worktreeDir` workspace setting (injected into the `.code-workspace` file alongside `wormhole.port`).

### 9. Tests

- Unit tests in [src/git.rs](src/git.rs) (worktree path construction, orphan detection, submodule tests): update expected paths.
- Integration tests in [tests/test_integration.rs](tests/test_integration.rs): update path assertions.
- [tests/harness.rs](tests/harness.rs): may need a test config file or env var for worktree dir.

### 10. [README.md](README.md) / docs

- Update worktree path documentation.
- Document `~/.wormhole/config.toml` format and the `WORMHOLE`_ + SCREAMING_SNAKE env var convention.
- Rename `WORMHOLE_PATH` to `WORMHOLE_SEARCH_PATHS` throughout.
- Add a note that wormhole is pre-1.0 with no backward compatibility guarantees.
- Update [.task/research-worktrees.md](.task/research-worktrees.md).

## Migration

Existing worktrees in `$gitdir/wormhole/worktrees/` will not be found after the change. Options:

- `**wormhole doctor migrate-worktrees**`: move existing worktrees to the new location and run `git worktree repair` (which updates git's internal bookkeeping for moved worktrees). This is the cleanest approach.
- Alternatively, users can just `wormhole project remove` old tasks and recreate them. Since branches are preserved, no work is lost.

## Phasing

The work naturally splits into two commits:

1. **Global config file**: introduce `~/.wormhole/config.toml` with `search_paths` (including per-path excludes), remove per-directory `.wormhole.toml`. Worktrees stay in `$gitdir` for now.
2. **Worktree relocation**: add `worktree_dir` config, change path formula, update discovery/extension/tests, add migration command.

