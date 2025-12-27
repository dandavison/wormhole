_This is a personal project under development, implemented only for MacOS (e.g. it uses hammerspoon in places, and the `open` command). VSCode is the only editor tested, but it could be made to work with others (e.g. JetBrains IDEs) with minor modifications. The terminal emulator must either be Wezterm, or Alacritty+Tmux. It could be made to work with non-Alacritty+Tmux with minor modifications._

Wormhole is for people who:
- work on multiple projects/repositories concurrently
- use an editor/IDE that does not run in the terminal
- use a terminal that is not the integrated terminal of their IDE

When you switch to work on a different project, two things should happen:

1. Your editor/IDE should switch to the new project workspace.
2. Your terminal emulator should switch to a window/tab/workspace with shell processes using the new project directory.

Wormhole makes that be true.

## CLI Usage

Wormhole provides a unified CLI for both running the server and interacting with it:

```bash
# Start the server (default if no subcommand given)
wormhole serve
wormhole

# Switch to a project by name
wormhole project myapp

# Open/create a project at a path
wormhole project /path/to/repo --name myapp

# Open a file (switches to containing project)
wormhole file /path/to/repo/src/main.rs:42

# Navigate between projects
wormhole previous
wormhole next

# Pin current (project, app) state for toggle behavior
wormhole pin

# List projects
wormhole list

# Key-value storage
wormhole kv get myapp land-in
wormhole kv set myapp land-in editor
wormhole kv delete myapp land-in
wormhole kv list myapp

# Close/remove projects
wormhole close myapp
wormhole remove myapp

# Debug info
wormhole debug
```

Run `wormhole --help` or `wormhole <command> --help` for detailed usage.

## Installation

Wormhole binds to port 7117 by default.

1. Clone this repo
2. Check that the editor and other settings in `src/config.rs` are appropriate for your environment
3. Install hammerspoon and ensure that the `hs` executable is on your `$PATH`
   E.g.
   ```
   brew install hammerspoon
   ln -s /Applications/Hammerspoon.app/Contents/Frameworks/hs/hs ~/bin
   ```
4. Build and install: `cargo build --release && cp target/release/wormhole ~/bin/`
5. Start the server with `sudo wormhole serve` (or `sudo make serve`)

## Example workflows

Wormhole runs as an HTTP server that you interact with via the CLI or direct HTTP requests.
Here are some ideas.

- Use the MacOS [project-switcher UI](https://github.com/dandavison/wormhole-gui) to switch projects.

- Wormhole understands GitHub URLs:

  Use the [requestly](https://chrome.google.com/webstore/detail/requestly-open-source-htt/mdnleldcmiljblolnjhpnblkcekpdkpa) chrome extension to send GitHub links to wormhole via rules like:<br>

  - URL RegEx `/^https://github.com/([^#]+/blob/[^#?]+)(?:#L(\d+))?(?:-L\d+)?$/` Redirect to URL `http://localhost:7117/$1?line=$2`
  - URL RegEx `/^https://github.com/([^#]+/blob/[^#]+\?line=\d+)$/` Redirect to URL `http://wormhole:7117/$1`

  Now GitHub URLs in Google Docs, Notion, Swimlanes.io, etc will open via Wormhole.

- Open code from terminal applications that create terminal hyperlinks, such as [delta](https://dandavison.github.io/delta/grep.html?highlight=hyperlinks#grep) and [ripgrep](https://github.com/BurntSushi/ripgrep/discussions/2611).

- Hammerspoon can send HTTP requests via a keybinding, e.g.
  ```lua
  hs.hotkey.bind({ "cmd", "control" }, "left", function()
     hs.http.get("http://localhost:7117/previous-project/", nil)
  end)
  ```

The fundamental point is that Wormhole opens code in your editor/IDE at the correct line, while ensuring that your editor/IDE selects the correct project workspace for the file, and also switches your terminal emulator to that project.

## HTTP API Reference

Wormhole runs as an HTTP server on port 7117 (configurable in `src/config.rs`).

### Project Navigation

#### `GET /project/<name_or_path>`
Unified project endpoint: switches to a project by name or path, creating it if necessary (upsert behavior).
- **Path**: Project name or absolute path
- **Behavior**:
  1. If `<name_or_path>` matches an existing project name → switches to that project
  2. If `<name_or_path>` is an absolute path matching an existing project → switches to that project
  3. If `<name_or_path>` is an absolute path with no matching project → creates a new project and switches to it
- **Query Parameters**:
  - `land-in` - Which application to focus: `terminal` or `editor`
  - `name` - Project name (used when creating new project from path)
- **Examples**:
  - `/project/myapp` - Switch to project named "myapp"
  - `/project//Users/me/repos/myapp?name=myapp` - Open/create project at path with name "myapp"
  - `/project/myapp?land-in=editor` - Switch to "myapp" and focus editor

#### `GET /previous-project/`
Switches to the previous project in the rotation.
- **Query Parameters**:
  - `land-in` - Which application to focus: `terminal` or `editor`
- **Example**: `/previous-project/?land-in=terminal`

#### `GET /next-project/`
Switches to the next project in the rotation.
- **Query Parameters**:
  - `land-in` - Which application to focus: `terminal` or `editor`
- **Example**: `/next-project/`

#### `POST /pin/`
Pins the current state by saving which application (editor or terminal) is currently focused to the project's KV store as `land-in`.
- **Example workflow**:
  1. Switch to project A and focus editor, then `POST /pin/` → saves `land-in=editor` for A
  2. Switch to project B and focus terminal, then `POST /pin/` → saves `land-in=terminal` for B
  3. Now using `/previous-project/` and `/next-project/` will toggle between (A, editor) and (B, terminal)

### File Opening

#### `GET /file/<absolute_path>`
Opens a specific file in the appropriate project.
- **Path**: Absolute file path, optionally with line number using `:` separator
- **Query Parameters**:
  - `land-in` - Which application to focus: `terminal` or `editor`
- **Examples**:
  - `/file//Users/me/myapp/src/main.rs:42` - Opens main.rs at line 42
  - `/file//Users/me/myapp/README.md?land-in=editor`
- **Note**: Line numbers must be specified in the path with `:`, not as a query parameter

### GitHub Integration

#### `GET /<github_path>`
Handles GitHub URLs for direct file opening (always focuses editor).
- **Path**: GitHub blob path (e.g., `/owner/repo/blob/branch/path/to/file.rs`)
- **Query Parameters**:
  - `line` - Line number to jump to
- **Example**: `/dandavison/delta/blob/main/src/main.rs?line=25`
- **Note**: Always lands in editor. If the repository isn't recognized as a local project, redirects to GitHub.

### Key-Value Storage

#### `GET /kv/<project>/<key>`
Retrieves a stored value for a project.
- **Path**: Project name and key name
- **Response**: Plain text value
- **Example**: `/kv/myproject/land-in` returns `terminal`

#### `PUT /kv/<project>/<key>`
Sets a value for a project key.
- **Path**: Project name and key name
- **Body**: Plain text value to store
- **Example**: `curl -X PUT http://wormhole:7117/kv/myproject/land-in -d "editor"`

#### `DELETE /kv/<project>/<key>`
Deletes a key from a project.
- **Path**: Project name and key name
- **Example**: `curl -X DELETE http://wormhole:7117/kv/myproject/land-in`

#### `GET /kv/<project>`
Retrieves all key-value pairs for a project.
- **Path**: Project name
- **Response**: JSON object with all key-value pairs
- **Example**: `/kv/myproject` returns `{"land-in": "terminal", "theme": "dark"}`

#### `GET /kv`
Retrieves all key-value pairs for all projects.
- **Response**: JSON object with projects as keys and their KV pairs as values
- **Example**: Returns `{"myproject": {"land-in": "terminal"}, "other": {"land-in": "editor"}}`

### Project Management

#### `GET /list-projects/`
Lists all currently open projects (one per line).
- **Response**: Plain text list of project names (rotated so current project is last)

#### `GET /debug-projects/`
Returns detailed debug information about all known projects.
- **Response**: Indexed list with project names, paths, and aliases

#### `POST /remove-project/<name>`
Removes a project from wormhole.
- **Path**: Project name to remove
- **Example**: `/remove-project/myapp`

#### `POST /close-project/<name>`
Closes the editor and terminal windows for a project.
- **Path**: Project name to close
- **Example**: `/close-project/myapp`
