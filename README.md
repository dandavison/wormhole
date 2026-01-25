Wormhole manages a collection of 'projects'.

Each project is a git repo: it has a tmux window with that repo as the CWD, and it has an IDE workspace in that repo.

`wormhole project switch my-project` communicates with tmux and your OS window manager so that the tmux window and IDE workspace are selected. You have control over which of the two applications is focused.

Some projects are 'tasks'. A task is a project for which the CWD is a git worktree directory, rather than the "real" project directory.


A task should evolve through a state machine to reach a terminal state. Wormhole reports on what stage the task is at (`wormhole project status`) by reading relevant state from JIRA, GitHub, and local files (e.g. does a plan.md exist? Is there a JIRA ticket and what state is it in? Is there a PR and is it draft or open for review?).




## Switching projects

When you switch between projects, two things should happen:

1. Your editor should switch to the new project workspace.
2. Your terminal emulator should switch to a window with a shell process in the new project directory.

In addition, when you click on a terminal hyperlink to a file, it should open the file in the correct project workspace, with the correct terminal window focused.
Links to files in GitHub should do the same, if you don't want them to open in a web browser.

The aim of Wormhole is to make the above things happen.

Wormhole is for people who:
- work on multiple projects/repositories concurrently
- use an editor that does not run in the terminal
- use a terminal application that is separate from their editor
- use MacOS (it uses hammerspoon)


_This is a personal project under development, implemented only for MacOS (e.g. it uses hammerspoon in places, and the `open` command). The terminal emulator must either be Alacritty+Tmux or Wezterm. VSCode/Cursor is the only editor tested. It could be made to work with other editors (e.g. JetBrains products)._

## Installation

```bash
# Dependencies
brew install hammerspoon
ln -s /Applications/Hammerspoon.app/Contents/Frameworks/hs/hs ~/bin/

# Build
cargo build --release
cp target/release/wormhole ~/bin/

# Run server
wormhole serve

# GUI (optional)
(cd gui && make dist)
ln -fs $PWD/gui/dist/Wormhole/Wormhole.app /Applications/Wormhole.app

# Chrome extension (optional)
# Load web/chrome-extension as unpacked extension
```

Edit `src/config.rs` for editor/terminal settings.

## CLI

```bash
wormhole serve                          # Start server (port 7117)
wormhole project switch myapp           # Switch to project by name
wormhole project switch /path/to/repo   # Open/create project at path
wormhole project switch ACT-1234 --home-project myrepo  # Open task (creates worktree)
wormhole project list                   # List projects (includes tasks)
wormhole project previous               # Previous project
wormhole project next                   # Next project
wormhole project close myapp            # Close project windows
wormhole project remove myapp           # Remove project/task
wormhole project pin                    # Pin current (project, app) state
wormhole project debug                  # Debug info for all projects
wormhole project status                 # Show task status (JIRA, PR, plan.md)
wormhole project status ACT-1234        # Show status for specific project
wormhole file /path/to/file.rs:42       # Open file at line
wormhole kv get myapp land-in           # Get KV
wormhole kv set myapp land-in editor    # Set KV
wormhole jira sprint                    # List JIRA sprint issues
wormhole kill-session                   # Kill tmux session and clean up
wormhole completion bash                # Generate shell completions
```

## HTTP API

| Method | Endpoint                    | Description                       |
|--------|-----------------------------|-----------------------------------|
| GET    | `/project/switch/<name>`    | Switch/create project or task     |
| GET    | `/project/list`             | List projects (JSON, includes tasks) |
| GET    | `/project/previous`         | Previous project                  |
| GET    | `/project/next`             | Next project                      |
| POST   | `/project/close/<name>`     | Close project windows             |
| POST   | `/project/remove/<name>`    | Remove project/task               |
| POST   | `/project/pin`              | Pin current (project, app) state  |
| GET    | `/project/debug`            | Debug info                        |
| GET    | `/project/status[/<name>]`  | Task status (JIRA, PR, plan.md)   |
| GET    | `/file/<path>`              | Open file (path:line supported)   |
| GET    | `/<github_blob_path>?line=N`| Open GitHub file locally          |
| GET    | `/kv/<project>/<key>`       | Get value                         |
| PUT    | `/kv/<project>/<key>`       | Set value (body)                  |
| DELETE | `/kv/<project>/<key>`       | Delete key                        |
| GET    | `/kv/<project>`             | List project KV                   |
| GET    | `/kv`                       | List all KV                       |

Query params: `land-in=terminal|editor`, `name=<project_name>`, `line=N`, `home-project=<project>` (for tasks), `format=json`

## Environment Variables

| Variable | Description |
|----------|-------------|
| `JIRA_INSTANCE` | JIRA instance name (e.g., `mycompany` for mycompany.atlassian.net) |
| `JIRA_EMAIL` | JIRA account email |
| `JIRA_TOKEN` | JIRA API token |
| `GITHUB_REPO` | GitHub repo (e.g., `owner/repo`) for PR lookup in `jira sprint` |

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
Install the Chrome extension from `web/chrome-extension/`, or use Requestly with:
- `/^https://github.com/([^#]+/blob/[^#?]+)(?:#L(\d+))?(?:-L\d+)?$/` → `http://localhost:7117/$1?line=$2`

**Terminal hyperlinks:**
Tools like [delta](https://dandavison.github.io/delta/) and [ripgrep](https://github.com/BurntSushi/ripgrep) emit OSC 8 hyperlinks. Configure them to use `http://localhost:7117/file/` URLs.

## Shell Integration

When wormhole opens a terminal for a project, it writes environment variables to `/tmp/wormhole.env`:
```bash
export WORMHOLE_PROJECT_NAME=myproject WORMHOLE_PROJECT_DIR=/path/to/myproject
```

Wormhole doesn't try to `cd` existing shells—it opens new terminal windows/tabs at the correct directory (via tmux `new-window -c` or similar). The env file is metadata that scripts or prompts can optionally source.

Optional shell helpers in `cli/lib.sh`:
```bash
source /path/to/wormhole/cli/lib.sh

wormhole-env   # Source /tmp/wormhole.env into current shell
wormhole-cd    # cd to $WORMHOLE_PROJECT_DIR (or pass a path)
```
