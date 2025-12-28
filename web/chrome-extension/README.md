# Wormhole GitHub Redirector

Chrome extension that redirects GitHub file links to your local Wormhole server.

## What it does

When you click a GitHub link like:
- `https://github.com/owner/repo/blob/main/src/file.rs`
- `https://github.com/owner/repo/blob/main/src/file.rs#L42`

It redirects to:
- `http://localhost:7117/owner/repo/blob/main/src/file.rs`
- `http://localhost:7117/owner/repo/blob/main/src/file.rs?line=42`

Wormhole then opens the file in your local editor at the correct line.

## Installation

1. Open Chrome and go to `chrome://extensions/`
2. Enable "Developer mode" (toggle in top right)
3. Click "Load unpacked"
4. Select this directory (`web/chrome-extension`)

## Testing

1. Ensure Wormhole server is running (`wormhole serve`)
2. Click any GitHub file link (e.g., in a Google Doc, Slack, etc.)
3. The file should open in your local Cursor/VS Code

## Icons

To add custom icons, place `icon48.png` (48x48) and `icon128.png` (128x128) in this directory.

