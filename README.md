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
wormhole project myapp                  # Switch to project by name
wormhole project /path/to/repo          # Open/create project at path
wormhole file /path/to/file.rs:42       # Open file at line
wormhole previous                       # Previous project
wormhole next                           # Next project
wormhole pin                            # Pin current (project, app) state
wormhole list                           # List projects
wormhole kv get myapp land-in           # Get KV
wormhole kv set myapp land-in editor    # Set KV
wormhole close myapp                    # Close project windows
wormhole remove myapp                   # Remove from wormhole
```

## HTTP API

| Method | Endpoint | Description |
|--------|----------|-------------|
| GET | `/project/<name_or_path>` | Switch/create project |
| GET | `/previous-project/` | Previous project |
| GET | `/next-project/` | Next project |
| POST | `/pin/` | Pin current (project, app) state |
| GET | `/file/<path>` | Open file (path:line supported) |
| GET | `/<github_blob_path>?line=N` | Open GitHub file locally |
| GET | `/list-projects/` | List projects (JSON) |
| GET | `/debug-projects/` | Debug info |
| POST | `/close-project/<name>` | Close project windows |
| POST | `/remove-project/<name>` | Remove project |
| GET | `/kv/<project>/<key>` | Get value |
| PUT | `/kv/<project>/<key>` | Set value (body) |
| DELETE | `/kv/<project>/<key>` | Delete key |
| GET | `/kv/<project>` | List project KV |
| GET | `/kv` | List all KV |

Query params: `land-in=terminal|editor`, `name=<project_name>`, `line=N`

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
