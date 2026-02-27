# Conversation Search Feature

## Motivation

Finding past agent conversations requires manual browsing. We materialize clean plain text from Cursor agent transcripts for rgi search. On Enter, the Cursor conversation is converted to Claude Code JSONL format and resumed in the Claude Code VS Code extension panel.

## End-to-End Flow

```
rgi pattern $(wormhole conversations sync)
```

1. `wormhole conversations sync` materializes clean text from Cursor transcripts, prints output dir
2. rgi searches with rg, shows results with bat preview
3. User hits Enter on a match
4. rgi calls `wormhole open <file>:<line>` (via RGI_EDITOR=wormhole)
5. wormhole detects the file is under `~/.wormhole/conversations/`, extracts the Cursor transcript UUID from the filename
6. wormhole reads the original Cursor transcript and converts it to Claude Code JSONL, writing to `~/.claude/projects/<key>/`
7. wormhole determines the wormhole project and sends `claude-code/resume` intent with the new session UUID
8. The wormhole VS Code extension calls `vscode.commands.executeCommand("claude-vscode.editor.open", sessionId)`
9. Claude Code panel opens with the conversation history, ready for a new prompt

## Usage

```bash
rgi pattern $(wormhole conversations sync)                      # interactive search
rgi $(wormhole conversations sync --project wormhole)           # scoped to a project
rg "worktree" $(wormhole conversations sync)                    # non-interactive
```

## Output Directory Structure

```
~/.wormhole/conversations/
  wormhole/
    2026-02-25-8afac8bb.txt
    2026-02-24-0b80b8c3.txt
  cli/
    2026-02-21-ea2a7ded.txt
  temporal/
    2026-02-20-f4f28d4a.txt
```

Naming: `<date>-<short-uuid>.txt` where the short UUID is the first 8 chars of the Cursor transcript UUID. Grouped by wormhole project name.

## Plain Text File Format

```
# wormhole | 2026-02-25 | 8afac8bb

## User

Good evening. Would you like to discuss a possible feature?

## Assistant

Yes, let me research this area...

## User

What about using rgi?

## Assistant

That's a great idea...
```

- Markdown headers for bat preview rendering
- First line: metadata (project, date, short transcript id)
- User/assistant text only; `<user_query>` tags, system prompts, tool calls, and XML context injection stripped

## Data Source: Cursor Agent Transcripts

- Path: `~/.cursor/projects/<cursor-project-key>/agent-transcripts/<uuid>/<uuid>.jsonl` (newer) or `<uuid>.jsonl` / `<uuid>.txt` (older)
- Cursor project key: derived from workspace file path, slashes replaced by dashes
- JSONL: `{"role":"user"|"assistant","message":{"content":[{"type":"text","text":"..."}]}}`
- TXT: `user:\n<user_query>...\nA:\n[Thinking]...\n...`
- No timestamps in content; use file mtime for date

## Project-to-Transcript Mapping

Wormhole project -> Cursor transcript directories:

1. From wormhole project, get the `.code-workspace` file path (e.g. `/Users/dan/src/wormhole/.git/wormhole/workspaces/wormhole.code-workspace`)
2. Encode as Cursor project key: replace `/` with `-`, strip leading `/` (e.g. `Users-dan-src-wormhole-git-wormhole-workspaces-wormhole-code-workspace`)
3. Cursor transcripts at: `~/.cursor/projects/<key>/agent-transcripts/`
4. Also check the bare project dir path encoding (e.g. `Users-dan-src-wormhole`)

## Cursor-to-Claude-Code Conversion

When resume is requested (on Enter), convert a Cursor transcript to Claude Code JSONL:

- Generate a deterministic session UUID via UUID v5 (namespace + Cursor transcript UUID), so repeated resume of the same conversation reuses the same CC session
- Generate per-message UUIDs (also deterministic)
- Fabricate timestamps from file mtime (evenly spaced or single timestamp)
- Set `cwd` to wormhole project dir
- Set `gitBranch` from wormhole project data
- Strip `<user_query>` wrappers, extract text from content arrays
- Write to `~/.claude/projects/<cc-key>/<new-session-uuid>.jsonl`
- CC project key: project dir path with `/` replaced by `-`, prefixed with `-`

## Resume Path

### `wormhole open` handling

When `wormhole open` receives a path under `~/.wormhole/conversations/`:

1. Parse project name from directory, Cursor transcript UUID from filename
2. Read the original Cursor transcript from `~/.cursor/projects/.../agent-transcripts/`
3. Convert to Claude Code JSONL, write to `~/.claude/projects/<key>/`
4. Switch to the project (focus editor window)
5. Send `claude-code/resume` intent to that project's editor via the wormhole message channel, with the new session UUID

### Wormhole VS Code extension

Handler for `claude-code/resume` intent:

```typescript
'claude-code/resume': async (_projectKey, _port, params) => {
  const sessionId = params?.sessionId as string | undefined;
  if (!sessionId) return;
  const ext = vscode.extensions.getExtension('anthropic.claude-code');
  if (!ext) return;
  if (!ext.isActive) await ext.activate();
  await vscode.commands.executeCommand('claude-vscode.editor.open', sessionId);
},
```

This activates the Claude Code extension if needed, then calls its registered command, which opens a panel and passes `--resume <sessionId>` to the claude CLI.

## Key Design Decisions

1. **Compose, don't build UI**: rgi handles all interaction; wormhole provides data and the resume bridge.
2. **Cursor as source, Claude Code as resume target**: All existing conversations are in Cursor; Claude Code's extension has the resume API.
3. **On-demand conversion**: Conversion happens at resume time (Enter), not during sync. Sync only materializes clean text for search.
4. **Deterministic session UUIDs**: UUID v5 from Cursor UUID avoids duplicates when resuming the same conversation multiple times.
5. **`sync` prints the path**: Enables `$(wormhole conversations sync)` shell composition.
6. **Resume via existing message channel**: The wormhole extension already handles intents; adding one more is minimal.

## Future

- Add Claude Code native transcript parsing (search CC conversations too, resume directly without conversion)
- If Cursor adds a resume API, support it as an alternative resume target
