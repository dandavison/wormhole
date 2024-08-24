_This is a personal project under development, implemented only for MacOS (e.g. it uses hammerspoon in places, and the `open` command). VSCode is the only editor tested, but it could be made to work with others (e.g. JetBrains IDEs) with minor modifications. The terminal emulator must either be Wezterm, or Alacritty+Tmux. It could be made to work with non-Alacritty+Tmux with minor modifications._

Wormhole is for people who:
- work on multiple projects/repositories concurrently
- use an editor/IDE that does not run in the terminal
- use a terminal that is not the integrated terminal of their IDE

When you switch to work on a different project, two things should happen:

1. Your editor/IDE should switch to the new project workspace.
2. Your terminal emulator should switch to a window/tab/workspace with shell processes using the new project directory.

Wormhole makes that be true.
It is an HTTP service providing the following commands:

- Switch to a project by name
- Switch to a project given a file path, and open that path in your editor/IDE (at the specified line number)
- Switch to a project given a github URL, and open the corresponding file path in your editor/IDE (at the specified line number)

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
4. Start the server with `sudo make serve`

## Example workflows:

Wormhole is an HTTP server.
It can be used in various ways, with various HTTP clients.
Here are some ideas.

- Use the MacOS [project-switcher UI](https://github.com/dandavison/wormhole-gui) to switch projects.

- Wormhole understands GitHub URLs:

  Use the [requestly](https://chrome.google.com/webstore/detail/requestly-open-source-htt/mdnleldcmiljblolnjhpnblkcekpdkpa) chrome extension to send GitHub links to wormhole via rules like:<br>

  - URL RegEx `/^https://github.com/([^#]+/blob/[^#?]+)(?:#L(\d+))?$/` Redirect to URL `http://localhost:7117/$1?line=$2`
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
