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