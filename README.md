Wormhole is a tool for working on software projects.

- A set of directories is specified in `WORMHOLE_PATH`. The set of _available repos_ is the union of
  the git repo directories that are located at the top level in one of those directories. These may
  be submodules, or top-level git repos.

- A _task_ is a `(repo, branch)` pair: a branch in some git repository. The branch has a short
  descriptive name that acts as the name of the task.

- Wormhole will ensure that a git worktree for the task exists. The worktree always has `$branch`
  checked out. You always work on the task in the worktree: never in the main repo dir. Wormhole can
  thus determine all known tasks by enumerating worktrees of available repos.

- In practice, wormhole stores its worktrees at `$gitdir/wormhole/worktrees/$branch/$repo_name`. If
  the repo directory is not a submodule then `$gitdir` is `$dir/.git`; if it is a submodule then
  `$gitdir` is the gitdir entry specified in the `$dir/.git` file. The leaf directory is `$repo_name`
  so that editors display the repo name (not the branch) in the sidebar.

- A _task_ is a type of _project_. Each repo is a non-task _project_. A non-task project has no
  associated branch. Thus the set of projects is the union of the _available repos_ and the
  worktrees of those repos. We assume that all repo worktrees are wormhole worktrees.

- The point of truth for what projects and tasks exist is this filesystem state. The only data
  persisted by wormhole itself is associated with the `wormhole kv` interface. It is stored in JSON
  files named `$gitdir/wormhole/kv/${repo}_${branch}.json` (with branch encoded to handle `/`),
  where `$gitdir` is as defined above for the submodule and non-submodule cases. For example, if a
  task has an associated JIRA ticket, then wormhole stores the JIRA identifier in kv. (A task may
  also have an associated GitHub PR but that does not need to be stored in kv since the `gh` CLI can
  discover it using the repo remote that is stored by git on disk.)

- Wormhole is a process exposing an HTTP API, with a CLI client that is a thin wrapper over the HTTP
  API. The CLI API includes `wormhole project list`, `wormhole task create`,
  `wormhole project switch`, etc.

- On server start, `wormhole project list` lists all tasks discovered on disk.

- After switching to a project via `wormhole project switch`, wormhole ensures that the following
  things are true: (1) a terminal tmux window for the project exists, (2) an editor workspace for
  the project exists, (3) one or the other of these applications is given focus (the user can
  control which by using `wormhole project pin` to store the preference in kv).

- The following sorts of hyperlinks can thus be created:
  - Go to the terminal tmux window for a specified project or task
  - Go to the editor worskpace for a specified project or task
  - Go to the editor worskpace for a specified project or task and open a specified line in a
    specified file.

- Wormhole has a browser extension. It re-routes GitHub format URLs to wormhole. On JIRA issue pages
  or GitHub PR pages that match a wormhole task it adds buttons linking to the tmux window and the
  editor workspace. A third button brings up an embedded vscode session in an iframe, on the same
  task workspace.

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


_This is a personal project under development, implemented only for MacOS (e.g. it uses hammerspoon
in places, and the `open` command). The terminal emulator must either be Alacritty+Tmux or Wezterm.
VSCode/Cursor is the only editor tested. It could be made to work with other editors (e.g. JetBrains
products)._


## Installation

```bash
# Dependencies
brew install hammerspoon
ln -s /Applications/Hammerspoon.app/Contents/Frameworks/hs/hs ~/bin/

# Build
cargo build --release
ln -s target/release/wormhole ~/bin/

# Run server
wormhole server start

# To run multiple wormholes, create a new tmux session, set WORMHOLE_PORT, and run `server start-foreground`

# GUI (optional)
(cd gui && make dist)
ln -fs $PWD/gui/dist/Wormhole/Wormhole.app /Applications/

# Chrome extension (optional)
# Load web/chrome-extension as unpacked extension
```

## CLI

```bash
wormhole server start                   # Start server daemon (port 7117)
wormhole server stop                    # Stop server daemon
wormhole server attach                  # Attach to running server
wormhole open myapp                     # Switch to project by name
wormhole open /path/to/repo             # Open/create project at path
wormhole open /path/to/file.rs:42       # Open file at line in editor
wormhole open myrepo:ACT-1234           # Open task (creates worktree if needed)
wormhole project list                   # List projects (includes tasks)
wormhole project list --available       # List available projects (from WORMHOLE_PATH)
wormhole project list --active          # List only projects with tmux windows
wormhole project list --name-only       # Output project keys only (for completion)
wormhole project previous               # Previous project
wormhole project next                   # Next project
wormhole project close myapp            # Close project windows
wormhole project remove myapp           # Remove project/task
wormhole project pin                    # Pin current (project, app) state
wormhole project debug                  # Debug info for all projects
wormhole project show                   # Show task info (JIRA, PR, plan.md)
wormhole project show myrepo:ACT-1234   # Show info for specific project/task
wormhole kv get myapp land-in           # Get KV
wormhole kv set myapp land-in editor    # Set KV
wormhole kv delete myapp land-in        # Delete KV
wormhole kv list myapp                  # List all KV for project
wormhole task create <jira-url>         # Create task from JIRA URL
wormhole task create-from-sprint        # Create tasks for all sprint issues
wormhole jira sprint list               # List JIRA sprint issues
wormhole jira sprint show               # Show detailed sprint status
wormhole refresh                        # Refresh in-memory data from disk/APIs
wormhole kill                           # Kill tmux session and clean up
wormhole doctor persisted-data          # Report on worktrees and KV files
wormhole doctor migrate-worktrees      # Migrate worktrees to new layout
wormhole completion bash                # Generate shell completions
```

## HTTP API

| Method | Endpoint                    | Description                       |
|--------|-----------------------------|-----------------------------------|
| GET    | `/project/list`             | List projects (JSON, includes tasks) |
| GET    | `/project/neighbors`        | Project ring for navigation UI    |
| GET    | `/project/switch/<name>`    | Switch/create project or task     |
| GET    | `/project/create/<branch>`  | Create task with branch name      |
| GET    | `/project/previous`         | Previous project                  |
| GET    | `/project/next`             | Next project                      |
| POST   | `/project/close/<name>`     | Close project windows             |
| POST   | `/project/remove/<name>`    | Remove project/task               |
| POST   | `/project/pin`              | Pin current (project, app) state  |
| GET    | `/project/debug`            | Debug info                        |
| GET    | `/project/show[/<name>]`    | Task info (JIRA, PR, plan.md)     |
| POST   | `/project/describe`         | Describe URL (JIRA/GitHub lookup) |
| GET    | `/project/vscode/<name>`    | Get embedded VSCode URL           |
| POST   | `/project/refresh`          | Refresh all in-memory data        |
| POST   | `/project/refresh/<name>`   | Refresh single project            |
| POST   | `/project/refresh-tasks`    | Refresh task worktrees            |
| GET    | `/dashboard`                | Sprint dashboard HTML             |
| GET    | `/shell`                    | Shell env vars (pwd query param)  |
| GET    | `/file/<path>`              | Open file (path:line supported)   |
| GET    | `/<github_blob_path>?line=N`| Open GitHub file locally          |
| GET    | `/kv/<project>/<key>`       | Get value                         |
| PUT    | `/kv/<project>/<key>`       | Set value (body)                  |
| DELETE | `/kv/<project>/<key>`       | Delete key                        |
| GET    | `/kv/<project>`             | List project KV                   |
| GET    | `/kv`                       | List all KV                       |

Query params: `land-in=terminal|editor`, `line=N`, `home-project=<project>`, `branch=<branch>`, `active=true`, `skip-editor=true`, `focus-terminal=true`, `sync=true`, `pwd=<path>`

## Environment Variables

| Variable                | Description                                                        |
|-------------------------|--------------------------------------------------------------------|
| `JIRA_INSTANCE`         | JIRA instance name (e.g., `mycompany` for mycompany.atlassian.net) |
| `JIRA_EMAIL`            | JIRA account email                                                 |
| `JIRA_TOKEN`            | JIRA API token                                                     |
| `GITHUB_REPO`           | GitHub repo (e.g., `owner/repo`) for PR lookup in `jira sprint`    |
| `WORMHOLE_DEFAULT_HOME` | Default home project for `jira sprint create`                      |

## Example Workflows

**Hammerspoon keybindings:**
```lua
package.path = package.path .. ";/path/to/wormhole/hammerspoon/?.lua"
local wormhole = require("wormhole")

hs.hotkey.bind({ "cmd", "control" }, "left", wormhole.previous)
hs.hotkey.bind({ "cmd", "control" }, "right", wormhole.next)
hs.hotkey.bind({ "cmd", "control" }, ".", wormhole.pin)
hs.hotkey.bind({}, "f13", wormhole.select)
```

**GitHub links → local editor:**
Install the Chrome extension as an unpacked extension from `web/chrome-extension/`

**Terminal hyperlinks:**
Tools like [delta](https://dandavison.github.io/delta/) and [ripgrep](https://github.com/BurntSushi/ripgrep) emit OSC 8 hyperlinks. Configure them to use `http://localhost:7117/file/` URLs.

## Shell Integration

When wormhole opens a terminal for a project, it sets environment variables:
- `WORMHOLE_PROJECT_NAME` - project or task name
- `WORMHOLE_PROJECT_DIR` - project root directory
- `WORMHOLE_JIRA_URL` - JIRA issue URL (if task has JIRA)
- `WORMHOLE_GITHUB_REPO` - GitHub repo (e.g., `owner/repo`)
- `WORMHOLE_GITHUB_PR_URL` - PR URL (if task has open PR)

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
