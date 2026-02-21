# Wormhole GitHub Integration

Chrome extension that integrates GitHub with your local Wormhole server.

## Features

### 1. File Link Redirection

When you click a GitHub link like:
- `https://github.com/owner/repo/blob/main/src/file.rs`
- `https://github.com/owner/repo/blob/main/src/file.rs#L42`

It redirects to Wormhole, which opens the file in your local editor at the correct line.

### 2. Project Switch Buttons

Adds **Terminal** and **Cursor** buttons to GitHub pages:

- **On PR pages**: Buttons switch to the task (using the branch name as task ID)
- **On repo pages**: Buttons switch to the project (using the repo name)

## Installation

1. Open Chrome and go to `chrome://extensions/`
2. Enable "Developer mode" (toggle in top right)
3. Click "Load unpacked"
4. Select this directory (`chrome-extension`)

## Usage

1. Ensure Wormhole server is running
2. Visit any GitHub repository or PR page
3. Click **Terminal** to open the project in your terminal
4. Click **Cursor** to open the project in Cursor/VS Code

## Icons

To add custom icons, place `icon48.png` (48x48) and `icon128.png` (128x128) in this directory.

