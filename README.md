_This is a personal project under development, implemented only for MacOS (e.g. it uses hammerspoon
in places, and the `open` command). The terminal emulator must either be Alacritty+Tmux or Wezterm.
VSCode/Cursor + Tmux is the only combination tested. It could be made to work with other editors (e.g. Zed, JetBrains
products)._

Wormhole is a tool for working on software projects.

- A set of directories to search for projects is configured in `~/.wormhole/wormhole.toml` (or via
  the `WORMHOLE_SEARCH_PATHS` env var). The set of _available repos_ is the union of the git repo
  directories that are located at the top level in one of those directories. These may be submodules,
  or top-level git repos.

- A _project_ is either a repository itself (a repo-level project with no branch) or a `(repo, branch)`
  pair (a branch project). A branch project is identified as `repo:branch`.

- When opening a branch project, Wormhole ensures that a git worktree exists for it. The worktree
  always has `$branch` checked out. You always work on a branch project in its worktree: never in the
  main repo dir. Wormhole can thus determine all known branch projects by enumerating worktrees of
  available repos.

- Project worktrees are stored at `$worktree_dir/$repo_name/$encoded_branch/$repo_name` (default
  `$worktree_dir` is `~/worktrees`, configurable in `wormhole.toml`). Grouped by repo first, with
  the repo name as the leaf so editors display the repo name in the sidebar.

- Thus the set of projects is the union of the _available repos_ and the branch worktrees of those
  repos. We assume that all repo worktrees are wormhole worktrees.

- The point of truth for what projects exist is this filesystem state. The only data persisted by
  wormhole itself is associated with the `wormhole kv` interface. It is stored in JSON files named
  `$gitdir/wormhole/kv/${repo}_${branch}.json` (with branch encoded to handle `/`), where `$gitdir`
  is as defined above for the submodule and non-submodule cases. For example, if a project has an
  associated JIRA ticket, then wormhole stores the JIRA identifier in kv. (A project may also have
  an associated GitHub PR but that does not need to be stored in kv since the `gh` CLI can discover
  it using the repo remote that is stored by git on disk.)

- Wormhole is a process exposing an HTTP API, with a CLI client that is a thin wrapper over the HTTP
  API. The CLI API includes `wormhole project list`, `wormhole open`,
  `wormhole project switch`, etc.

- On server start, `wormhole project list` lists all projects discovered on disk.

- After switching to a project via `wormhole project switch`, wormhole ensures that the following
  things are true: (1) a terminal tmux window for the project exists, (2) an editor workspace for
  the project exists, (3) one or the other of these applications is given focus (the user can
  control which by using `wormhole project pin` to store the preference in kv).

- Each project gets a generated `.code-workspace` file (stored at
  `$gitdir/wormhole/workspaces/<key>.code-workspace`). This gives each project a distinct VSCode
  window identity so multiple projects can be open simultaneously. The file includes `wormhole.port`
  and `wormhole.worktreeDir` settings so the VSCode extension connects to the correct server and
  derives the project key from the worktree path.

- The following sorts of hyperlinks can thus be created:
  - Go to the terminal tmux window for a specified project
  - Go to the editor workspace for a specified project
  - Go to the editor workspace for a specified project and open a specified line in a
    specified file.

- Wormhole has a browser extension. It re-routes GitHub format URLs to wormhole. On JIRA issue pages
  or GitHub PR pages that match a wormhole project it adds buttons linking to the tmux window and the
  editor workspace. A third button brings up an embedded vscode session in an iframe, on the same
  project workspace.

- Wormhole serves a sprint dashboard with a card for each sprint issue. Each card has buttons linking
  to terminal, editor, and embedded vscode.

- The server-side handlers for wormhole API operations typically do no network or disk I/O, instead
  using in-memory data about projects. `wormhole refresh` causes this data to be refreshed by
  discovering and querying git worktrees, performing JIRA API calls to fetch latest JIRA ticket
  data, using `gh` to discover PRs and fetch latest PR data, etc.

- Wormhole has some hammerspoon lua code binding keys to wormhole client actions.

- Wormhole has a GUI application written in Swift exposing a project selector interface for
  switching to projects.

- Wormhole has some shell code (1) creating a prompt in zsh that displays the repo and branch with
  OSC8 hyperlinks, and (2) exposing a `cd` utility that `cd`s to the project root dir.




## The grid

By default, wormhole doesn't even think about Cursor/VSCode. The Hammerspoon switcher slides left and
right effortlessly between tmux windows, and a new wormhole open is a very fast tmux spawn.

I think of wormhole as a two-dimensional grid. The x axis is projects. The y axis has two rows: the
lower row is tmux, and the upper is the editor. The upper row is nullable (the lower is
non-nullable), and entirely absent by default. But any column can opt to create a non-null upper-row
cell, and can also pin the column to that cell — in which case the usual left-to-right fast slide
across tmux windows by the Hammerspoon switcher will be disrupted by visiting an editor.




## Design Principles

**Remote-capable architecture**: All workspace operations (tmux sessions, git/worktree management,
running commands in project context) must go through the wormhole server's HTTP API. The CLI is a
thin HTTP client. This ensures that, in the future, the wormhole client can manage projects where
the terminal and editor sessions are hosted on a remote machine or VM. Presentation-only operations
(window focus, app activation via Hammerspoon, opening a local editor) remain client-side.

**CLI is a thin wrapper with JSON and unstructured text output options**: CLI commands should all
support -o json. They should all model their output data via a struct that can be rendered to JSON
by serde, and also has a method to be rendered as pretty-formatted unstructured text for humans (the
default) (this may be markdown in some cases; if so -o md should be explicitly offered).


## Installation

```bash
# Dependencies
brew install hammerspoon
ln -s /Applications/Hammerspoon.app/Contents/Frameworks/hs/hs ~/bin/

# Build
cargo build --release
ln -s $PWD/target/release/wormhole ~/bin/

# Run server
wormhole server start

# To run multiple wormholes, create a new tmux session, set WORMHOLE_PORT, and run `server start-foreground`

# GUI (optional)
(cd gui && make dist)
ln -fs $PWD/gui/dist/Wormhole/Wormhole.app /Applications/

# Chrome extension (optional)
# Load chrome-extension as unpacked extension
```

### Shell completions

Add one of the following to your shell config:

```bash
# bash (~/.bashrc)
eval "$(wormhole completion bash)"

# zsh (~/.zshrc)
eval "$(wormhole completion zsh)"

# fish (~/.config/fish/config.fish)
wormhole completion fish | source
```

## CLI

```bash
wormhole server start                   # Start server daemon (port 7117)
wormhole server stop                    # Stop server daemon
wormhole server attach                  # Attach to running server
wormhole open myapp                     # Switch to project by name
wormhole open /path/to/repo             # Open/create project at path
wormhole open /path/to/file.rs:42       # Open file at line in editor
wormhole open myrepo:my-branch          # Open branch project (creates worktree if needed)
wormhole open :my-branch                # Open an existing branch project, or use the default project
wormhole open https://github.com/o/r/pull/123 # Resolve a PR/issue URL into a worktree, then switch
wormhole open o/r#123                    # Same, short form (also: bare #123 against cwd remote)
wormhole open ACT-1234                   # Resolve a JIRA key/URL into a worktree, then switch
wormhole open ACT-1234 -p myrepo        # Home project for the new worktree; --dry-run to preview
wormhole project list                   # List projects (current and available)
wormhole project list --available       # List available projects (from search_paths)
wormhole project list --active          # List only projects with tmux windows
wormhole project list --name-only       # Output project keys only (for completion)
wormhole project previous               # Previous project
wormhole project next                   # Next project
wormhole project close myapp            # Close project windows
wormhole project close myapp --remove   # Close and remove (deletes worktree/KV for branch projects)
wormhole project close --all -i         # Prompt before closing each (RET = close, n = skip)
wormhole project pin                    # Pin current (project, app) state
wormhole project debug                  # Debug info for all projects
wormhole project show                   # Show project info (JIRA, PR, CLAUDE.md)
wormhole project show myrepo:ACT-1234   # Show info for specific project
wormhole project message myapp -m editor/close           # Send intent to project
wormhole project message myapp -m editor/toggleZenMode   # Toggle zen mode
wormhole project message --all -m gopls/stop             # Send intent to all open projects
wormhole project for-each <command>     # Run command in each project dir
wormhole kv get myapp land-in           # Get KV
wormhole kv set myapp land-in editor    # Set KV
wormhole kv delete myapp land-in        # Delete KV
wormhole kv delete --all land-in        # Delete a key from every project (e.g. clear pins)
wormhole kv list myapp                  # List all KV for project
wormhole project create-from-sprint     # Create projects for all sprint issues
wormhole project create-from-review-requests # Create projects from PR review requests
wormhole project done myrepo:ACT-1234   # Mark a non-JIRA project as done
wormhole project hide myrepo:ACT-1234   # Hide a project from the dashboard
wormhole project reopen myrepo:ACT-1234 # Unhide / clear local done status
wormhole jira sprint list               # List JIRA sprint issues
wormhole jira sprint show               # Show detailed sprint status
wormhole refresh                        # Refresh in-memory data from disk/APIs
wormhole kill                           # Kill tmux session and clean up
wormhole doctor persisted-data          # Report on worktrees and KV files
wormhole doctor conform                 # Conform project worktrees
wormhole doctor list-editor-windows     # List editor windows (flags stranded ones)
wormhole doctor close-editor-windows myrepo:branch  # Close a window by key (no need to focus it)
wormhole doctor close-editor-windows --stranded     # Close all stranded windows
wormhole completion bash                # Generate shell completions
```

## HTTP API

| Method | Endpoint                      | Description                       |
|--------|-------------------------------|-----------------------------------|
| GET    | `/project/list`               | List projects (JSON)              |
| GET    | `/project/neighbors`          | Project ring for navigation UI    |
| GET    | `/project/switch/<name>`      | Switch/create project             |
| GET    | `/project/create/<branch>`    | Create branch project with branch name |
| GET    | `/project/previous`           | Previous project                  |
| GET    | `/project/next`               | Next project                      |
| POST   | `/project/close/<name>`       | Close project windows (?remove=true to delete) |
| POST   | `/project/pin`                | Pin current (project, app) state  |
| GET    | `/project/current/poll`       | Poll for current project changes  |
| GET    | `/project/current/editor`     | Focus/open the current project's editor |
| GET    | `/project/current/terminal`   | Focus the current project's terminal |
| GET    | `/project/debug`              | Debug info                        |
| GET    | `/project/worktrees`          | Every project's working tree (for path attribution) |
| GET    | `/project/show[/<name>]`      | Project info (JIRA, PR, CLAUDE.md)|
| POST   | `/project/describe`           | Describe URL (JIRA/GitHub lookup) |
| GET    | `/project/vscode/<name>`      | Get embedded VSCode URL           |
| GET    | `/project/messages/<name>`    | Poll messages                     |
| POST   | `/project/messages/<name>`    | Publish messages                  |
| POST   | `/project/refresh`            | Refresh all in-memory data        |
| POST   | `/project/refresh/<name>`     | Refresh single project            |
| POST   | `/project/refresh-tasks`      | Refresh project worktrees         |
| POST   | `/project/create-from-review-requests` | Create projects from PR review requests |
| POST   | `/project/create-from-github-ref` | Create project from a GitHub PR/issue ref |
| POST   | `/batch`                      | Start a new batch                 |
| GET    | `/batch`                      | List batches                      |
| GET    | `/batch/<id>`                 | Batch status                      |
| GET    | `/batch/<id>/output`          | Batch output                      |
| POST   | `/batch/<id>/cancel`          | Cancel batch                      |
| GET    | `/`                           | Sprint dashboard HTML             |
| GET    | `/shell`                      | Shell env vars (pwd query param)  |
| GET    | `/file/<path>`                | Open file (path:line supported)   |
| GET    | `/<github_blob_path>?line=N`  | Open GitHub file locally          |
| GET    | `/asset/<path>`               | Serve static assets               |
| GET    | `/doctor/persisted-data`      | Report on worktrees and KV files  |
| POST   | `/doctor/conform`             | Conform project worktrees         |
| GET    | `/doctor/editor-windows`      | List editor windows (with stranded status) |
| POST   | `/doctor/close-editor-windows`| Close editor windows by key or `--stranded` |
| GET    | `/jira/sprint/list`           | List JIRA sprint issues           |
| GET    | `/jira/sprint/show`           | Detailed sprint status            |
| GET    | `/kv/<project>/<key>`         | Get value                         |
| PUT    | `/kv/<project>/<key>`         | Set value (body)                  |
| DELETE | `/kv/<project>/<key>`         | Delete key                        |
| DELETE | `/kv-all/<key>`               | Delete key from all projects      |
| GET    | `/kv/<project>`               | List project KV                   |
| GET    | `/kv`                         | List all KV                       |
| POST   | `/terminal/run`               | Run a command in a pane of a project, in a named directory (`?project=&cwd=&cmd=&pid=`) |

Query params: `land-in=terminal|editor|terminal-only|none`, `line=N`, `home-project=<project>`, `branch=<branch>`, `active=true`, `current=true`, `completed=true`, `dry-run=true`, `sync=true`, `pwd=<path>`, `run=<id>`, `offset=N`, `role=<role>`, `wait=N`, `remove=true`, `cmd=<command>`, `cwd=<path>`, `pid=N`

## Message Intents

The wormhole server routes JSON-RPC 2.0 notifications to editor extensions via the message channel.
The VSCode/Cursor extension translates intents to native editor commands. `project close` uses this
internally; intents can also be sent manually via `wormhole project message`.

| Intent                  | VSCode command                       | Description              |
|-------------------------|--------------------------------------|--------------------------|
| `editor/close`          | `workbench.action.closeWindow`       | Close the editor window  |
| `editor/toggleZenMode`  | `workbench.action.toggleZenMode`     | Toggle zen mode          |
| `gopls/stop`            | `go.languageserver.restart`          | Stop gopls (frees memory; sets `go.useLanguageServer=false`) |
| `gopls/start`           | `go.languageserver.restart`          | Restart gopls (sets `go.useLanguageServer=true`) |
| `echo`                  | _(writes KV `last-message=echo`)_    | Test connectivity        |
| `claude-code/resume`    | `claude-vscode.editor.open`          | Resume CC session        |
| `claude-code/start`     | `claude-vscode.terminal.open`        | Start CC with prompt     |

`gopls/stop` and `gopls/start` no-op in non-Go projects (where the Go extension
is inactive), so they are safe to broadcast to all open projects with `--all`.

```bash
wormhole project message myapp -m editor/close
wormhole project message myapp -m editor/toggleZenMode
wormhole project message myapp -m editor/close -t '*'  # broadcast to all roles
wormhole project message --all -m gopls/stop           # stop gopls in all open Go projects
wormhole project message --all -m gopls/stop -x repo:branch  # ...except an excluded project
```

## Configuration

Global config lives at `~/.wormhole/wormhole.toml`:

```toml
# Directories to search for projects (replaces WORMHOLE_SEARCH_PATHS env var).
# Plain strings and objects with per-path excludes can be mixed.
search_paths = [
    "~/src/repos",
    { path = "~/src", exclude = ["node_modules", "venv"] },
]

# Where project worktrees are created (default: ~/worktrees)
worktree_dir = "~/worktrees"

# Default editor (overridable at runtime; see WORMHOLE_EDITOR for valid names).
editor = "cursor"

# Per-project editor overrides: first glob to match the project (repo) name wins,
# else the `editor` default is used. Valid editor names are as for WORMHOLE_EDITOR.
editors = [
    { glob = "mathematics", editor = "emacs" },
    { glob = "*-docs", editor = "code" },
]

# The member branches of each project (see "Project branches" below). The key is
# a glob over project keys; the values are globs, or /regexes/, over branch
# names, in which `{project}` stands for the project's own branch.
[project_branches]
"*" = ["{project}-*", "{project}/*"]
"wormhole:multi-branch-tasks" = ["mbt-fixups", "mbt/*", "/^stack-[0-9]+$/"]

# Panes to add to a project's tmux window when it is first opened. The key is a
# glob over project keys, as above; a project gets the panes of every entry
# matching it. The window opens with a shell in the project's working tree; each
# pane splits off one more pane there and types `command` into its shell (a
# plain shell if omitted). Each split divides the pane focused at that point.
#   split: right (default), left, below or above
#   size:  cells or a percentage, e.g. "80" or "50%" (default: share equally)
#   focus: whether the new pane takes focus (default: true)
# This opens a Claude Code session beside the shell in every project, and a
# server below it in one repo's projects:
[project_layout]
"*" = [
    { split = "right", command = "claude" },
]
"myapp:*" = [
    { split = "below", size = "30%", command = "make serve", focus = false },
]
```

### Project branches

A project is identified by one branch (`repo:branch`), and its worktree lives at a path derived
from that branch. Work on a project often spans several branches: a stack of PRs, a follow-up, a
rebased copy. The `project_branches` table declares which further branches — the project's
_member branches_ — belong to the existing project instead of becoming projects of their own.

A project's patterns are the union of those of every entry whose key glob matches its
`repo:branch` key, with `{project}` replaced by its branch. A pattern is a glob unless delimited
by `/`, in which case the text between the delimiters is a regex; both must match the whole
branch name. With the entries above, `feature-x-2` and `feature-x/fixup` are member branches of
`repo:feature-x`, and `mbt-fixups`, `mbt/2` and `stack-3` are member branches of
`wormhole:multi-branch-tasks` alone.

Concretely, given a project and its member branches:

- Checking out a member branch inside the project's worktree leaves the project's identity,
  worktree, tmux window and editor workspace alone, even once the original branch has been merged
  and deleted: the worktree directory name carries the identity. (Without a match, the project is
  renamed after the checked-out branch and `wormhole doctor conform` relocates the worktree.)
- `wormhole open repo:feature-x-2`, `wormhole open :feature-x-2`, and opening a PR or issue whose
  branch is a member branch all land in the existing project; no second worktree is created.
- The browser extension's buttons on a PR page link to the project whose branch the PR's head
  branch is, or is a member branch of, even when the worktree currently has a different branch
  checked out.

Membership is exclusive, and never decided by contest: if two projects claim the same branch (say
`feature` and `feature-x` both exist, and the global entry above makes `feature-x-2` a member
branch of each), it belongs to neither and the operation that asked for it fails with an error
naming both. A branch is never a member branch of itself, and a branch that already has a project
of its own is that project regardless of the table. The table is re-read by `wormhole refresh`.

`~` is expanded to `$HOME` at load time. `WORMHOLE_CONFIG` names a different file to read.

Env var names follow the convention `WORMHOLE_` + SCREAMING_SNAKE of the TOML key. Env vars
override config file values when set. `WORMHOLE_SEARCH_PATHS` is colon-separated; per-path excludes,
`editors`, `project_branches` and `project_layout` are config-file-only features.

_Wormhole is pre-1.0. Configuration and environment variable names may change without backward
compatibility._

## Environment Variables

| Variable                  | Description                                                        |
|---------------------------|--------------------------------------------------------------------|
| `WORMHOLE_CONFIG`         | Config file to read instead of `~/.wormhole/wormhole.toml`         |
| `WORMHOLE_SEARCH_PATHS`   | Colon-separated directories to search for projects                 |
| `WORMHOLE_WORKTREE_DIR`   | Where project worktrees are created (default: `~/worktrees`)       |
| `WORMHOLE_DEFAULT_PROJECT` | Project used for a new branch project opened as `:branch`         |
| `WORMHOLE_PROJECT_BRANCHES` | Whitespace-separated patterns, overriding `project_branches` with one `"*"` entry |
| `WORMHOLE_PORT`           | HTTP API port (default: 7117)                                      |
| `WORMHOLE_EDITOR`         | Editor to use (`cursor`, `code`, `code-insiders`, `emacs`, `none`) |
| `JIRA_INSTANCE`           | JIRA instance name (e.g., `mycompany` for mycompany.atlassian.net) |
| `JIRA_EMAIL`              | JIRA account email                                                 |
| `JIRA_TOKEN`              | JIRA API token                                                     |
| `GITHUB_REPO`             | GitHub repo (e.g., `owner/repo`) for PR lookup in `jira sprint`    |
| `WORMHOLE_DEFAULT_HOME`   | Default home project for `jira sprint create`                      |

## Example Workflows

**Hammerspoon keybindings:**
```lua
package.path = package.path .. ";/path/to/wormhole/hammerspoon/?.lua"
local wormhole = require("wormhole")

hs.hotkey.bind({ "cmd", "control" }, "left", wormhole.previous)
hs.hotkey.bind({ "cmd", "control" }, "right", wormhole.next)
hs.hotkey.bind({ "cmd", "control" }, "up", wormhole.editor)  -- focus current project's editor
hs.hotkey.bind({ "cmd", "control" }, "down", wormhole.terminal)  -- focus current project's terminal
hs.hotkey.bind({ "cmd", "control" }, ".", wormhole.pin)
hs.hotkey.bind({}, "f13", wormhole.select)
```

**GitHub links → local editor:**
Install the Chrome extension as an unpacked extension from `chrome-extension/`

**Terminal hyperlinks:**
Tools like [delta](https://dandavison.github.io/delta/) and [ripgrep](https://github.com/BurntSushi/ripgrep) emit OSC 8 hyperlinks. Configure them to use `http://localhost:7117/file/` URLs.

## Shell Integration

When wormhole opens a terminal for a project, it sets environment variables:
- `WORMHOLE_PROJECT_NAME` - project name
- `WORMHOLE_PROJECT_DIR` - project root directory
- `WORMHOLE_JIRA_URL` - JIRA issue URL (if project has JIRA)
- `WORMHOLE_GITHUB_REPO` - GitHub repo (e.g., `owner/repo`)
- `WORMHOLE_GITHUB_PR_URL` - PR URL (if project has open PR)

**Zsh prompt** (`shell/zsh/prompt.sh`):

Shows project name and git branch with OSC 8 hyperlinks:
- Project name links to JIRA issue (if available)
- Branch links to GitHub PR (if exists) or compare URL

```bash
source /path/to/wormhole/shell/zsh/prompt.sh
```

**Shell helpers** (`shell/lib.sh`):

```bash
source /path/to/wormhole/shell/lib.sh

wormhole-cd              # cd to $WORMHOLE_PROJECT_DIR
wormhole-cd /some/path   # cd to specified path
wormhole-open            # open current directory in wormhole
wormhole-open /path      # open specified path in wormhole
wormhole-shell-switch /path  # switch shell session to different project
wormhole-shell-reset     # re-fetch env vars from wormhole server
```


## Agent instructions
- Prefer commands from the Makefile over direct `cargo` commands.
- Always build with `--release` unless instructed otherwise.
- Never run `make integration-test-ui-ask-for-permission-to-run` without asking the user first — it
  drives a real editor window and steals focus.

At the start of the conversation output the following so that I know you've read these instructions:

📖 wormhole README
