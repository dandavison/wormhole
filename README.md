[_Personal project under development, currently implemented only for a MacOS: VSCode + Tmux development environment_]

Wormhole is for people who work on multiple projects/repositories concurrently.

When you switch to work on a different project, two things should happen:

1. Your editor/IDE should switch to the new project workspace.
2. Your terminal emulator should switch to the new project directory / tmux window.

Wormhole is an HTTP service providing the following commands:

- Switch to a project by name
- Switch to a project given a file path, and open that path in your editor/IDE (at the specified line number)
- Switch to a project given a github URL, and open the corresponding file path in your editor/IDE (at the specified line number)

Example workflows:

- Use the [MacOS app](https://github.com/dandavison/wormhole-gui) to switch projects
- Use Wormhole URLs to link to code from Google Docs, Notion, Swimlanes.io, etc
- Use Wormhole URLs to open code from terminal applications

Wormhole URLs open the code in your editor/IDE at the correct line, while
ensuring that your editor/IDE selects the correct project workspace for the
file, and also switch your terminal emulator to that project.

## Installation

Wormhole is currently implemented for MacOS only since it uses hammerspoon to work with your window manager.

TODO: Wormhole currently binds to port 80 by default and hence the server requires `sudo`.

1. Clone this repo
2. Check that the editor and other settings in `src/config.rs` are appropriate for your environment
3. Download hammerspoon and ensure that the `hs` executable is on your `$PATH`
   E.g.
   ```
   brew install hammerspoon
   ln -s /Applications/Hammerspoon.app/Contents/Frameworks/hs/hs ~/bin
   ```
4. Start the server with `sudo make serve`
5. Optional: install the MacOS [project-switcher UI](https://github.com/dandavison/wormhole-gui)
6. Optional: install the [requestly](https://chrome.google.com/webstore/detail/requestly-open-source-htt/mdnleldcmiljblolnjhpnblkcekpdkpa) chrome extension and create a rule like:
   URL RegEx `/https://github.com/([^#]+)#L(\d+).*/` Redirect to URL `http://localhost/$1?line=$2`
