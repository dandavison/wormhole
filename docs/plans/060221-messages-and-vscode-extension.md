---
name: VSCode extension message channel
overview: Add a per-project message routing system to the wormhole server and a minimal VSCode/Cursor extension that consumes messages. Use this to replace Hammerspoon-based editor close, and write a headed integration test that verifies close works in zen mode.
todos:
  - id: server-messages
    content: Add src/messages.rs with consumer registry (TTL-based tracking), per-consumer queues (drain-on-poll), and server-side routing
    status: completed
  - id: server-endpoints
    content: Add GET /project/messages/<name>?role=<role> (long-poll) and POST /project/messages/<name> (publish with target) endpoints
    status: completed
  - id: editor-close
    content: Modify Editor::close to publish editor/close targeted at role=editor (no Hammerspoon fallback)
    status: completed
  - id: vscode-extension
    content: Create web/vscode-extension/ with long-poll client (role=editor) and intent-to-VSCode translation layer
    status: completed
  - id: extension-makefile
    content: Add Makefile for vscode extension (build, install, dev symlink) and top-level Makefile targets
    status: completed
  - id: test-harness
    content: Add publish_message helper to test harness
    status: completed
  - id: integration-test
    content: Write test_close_zen_mode_window headed integration test
    status: completed
isProject: false
---

# Per-project message routing and VSCode extension

## Problems to solve

1. **Editor close in zen mode**: Wormhole cannot reliably close Cursor windows when Cursor is in zen mode with `window.nativeFullScreen: false` and `zenMode.fullScreen: true`. The current approach uses Hammerspoon `hs.window:close()` from outside the editor, which fails in this configuration. The fix is to close the window via Cursor's own API, which requires a VSCode extension.
2. **No VSCode extension exists**: Wormhole has a Chrome extension and Hammerspoon integration, but no VSCode/Cursor extension. Many editor operations (close, toggle zen mode, open files) would be more reliable via the editor's own API than via external window manipulation. An extension creates a channel for the server to request editor actions.
3. **Hammerspoon is inherently local and editor-specific**: The `hammerspoon::close_window` and `hammerspoon::launch_or_focus` functions work by externally manipulating windows. This violates the remote-capable architecture principle (all workspace operations should go through the HTTP API). A message-based approach works over the network.
4. **Integration test coverage for zen mode close**: There are headed integration tests for closing editor windows (see `test_close_project` in [tests/test_integration.rs](tests/test_integration.rs) and `test_close_cursor_window` in [tests/test_harness.rs](tests/test_harness.rs)), but none that exercise close in zen mode. We need a failing test that demonstrates the bug, then passes once the extension-based close is working.

## Architecture overview

The wormhole server gains a message routing system. Consumers (VSCode extension, future CLI tools) register by long-polling with a declared role. The server routes messages to consumers based on their role. Messages are JSON-RPC 2.0 notifications carrying wormhole-defined intents (not editor-specific commands). Each editor extension owns a translation layer from intents to its native API.

This is modeled on LSP and MCP: both use JSON-RPC 2.0 with protocol-defined method names that clients translate to native APIs. Our transport is HTTP long-poll (fire-and-forget notifications only). The message format is designed so that adding bidirectional request/response later (e.g., via SSE+POST or WebSocket) is a transport change only.

## Protocol

Messages use JSON-RPC 2.0 notification format (no `id`, no response expected). The `method` field is a wormhole-defined intent using `/`-separated naming (LSP convention):

```json
{"jsonrpc": "2.0", "method": "editor/close"}
{"jsonrpc": "2.0", "method": "editor/toggleZenMode"}
{"jsonrpc": "2.0", "method": "editor/openFile", "params": {"path": "/foo/bar.rs", "line": 42}}
```

The server publishes intents; editor extensions translate to native APIs. The server is agnostic to which editor consumes the messages.

## Server: consumer registry and message routing

**New module** [src/messages.rs](src/messages.rs):

**Consumer registry**: Consumers register by long-polling with a declared role. The server tracks connected consumers as `(project_key, role, queue, last_seen)` tuples. A consumer is considered "connected" if it has polled within a TTL window (e.g. 5 seconds). This avoids a false-negative gap between the extension receiving a response and issuing its next poll request. Well-known roles initially: `editor`. Future roles: `cli:<command-name>`, etc.

**Per-consumer queues**: Each registered consumer gets its own message queue (`Vec`). The queue is drained on poll -- since each consumer has a dedicated queue, there is exactly one reader and no need for sequence numbers or GC. The `last_seen` timestamp is updated on each poll.

**Routing**: When publishing, the publisher specifies a target:

- `publish(project_key, target=Role("editor"), notification)` -- copies message into queues of consumers with `role=editor` in that project (where consumer is within TTL)
- `publish(project_key, target=Broadcast, notification)` -- copies into all active consumer queues in that project

**State query**: `has_consumer(project_key, role) -> bool` -- returns true if any consumer with the given role in the given project has polled within the TTL window.

**Concurrency**: Follow the `lazy_static! + Mutex + guard wrapper` pattern used by [src/batch.rs](src/batch.rs) and [src/projects.rs](src/projects.rs). Use a `tokio::sync::watch` channel to wake long-polling consumers when new messages arrive (same `poll_until` pattern as batch progress polling in [src/handlers/project.rs](src/handlers/project.rs)).

**New endpoints** routed in [src/wormhole.rs](src/wormhole.rs):

- `GET /project/messages/<name>?role=<role>&wait=<secs>` -- Register as consumer with given role in the given project. Long-poll: blocks up to `wait` seconds (via `Prefer: wait=N` header or query param). Returns JSON array of JSON-RPC notification objects (drains the consumer's queue, updates `last_seen`). The extension calls this in a loop.
- `POST /project/messages/<name>` -- Publish a message. Body: `{"target": "editor", "message": {"jsonrpc": "2.0", "method": "editor/close"}}`. Target is a role name string, or `"*"` for broadcast. Used by `Editor::close` internally and by tests via HTTP.

**Modify `Editor::close`** in [src/editor.rs](src/editor.rs): Publish `{"jsonrpc": "2.0", "method": "editor/close"}` targeted at `role=editor`. No Hammerspoon fallback: if no consumer is connected, the message is published but not delivered, and the close is effectively a no-op. This replaces the Hammerspoon-based close entirely for the server side.

## VSCode/Cursor extension

Create `web/vscode-extension/` with:

- `package.json` -- Extension manifest. Publisher: `dandavison`. Activation event: `onStartupFinished`. No contributed commands or UI.
- `tsconfig.json` -- TypeScript config targeting ES2020, module commonjs (standard VSCode extension setup matching `~/src/vscode-etc/`).
- `src/extension.ts` -- On activate:
  1. Determine project name from the workspace folder name (the leaf directory of the first workspace folder). This matches how wormhole names projects -- the workspace folder is the project/worktree directory whose leaf is the repo name or branch name.
  2. Read wormhole port from `WORMHOLE_PORT` env var (default 7117).
  3. Start long-poll loop: `GET http://localhost:{port}/project/messages/{name}?role=editor&wait=30`.
  4. For each received notification, look up the `method` in the intent translation table and call `vscode.commands.executeCommand(...)`.
  5. On deactivate, abort the outstanding fetch (use `AbortController`).
- **Intent translation layer** -- the only editor-specific code:
  - `editor/close` -> `workbench.action.closeWindow`
  - `editor/toggleZenMode` -> `workbench.action.toggleZenMode`
  A future Zed or JetBrains plugin would have its own mapping table.

## Extension development workflow

- **During development**: Symlink `web/vscode-extension/` into `~/.cursor/extensions/`, run `tsc --watch`. "Reload Window" in Cursor picks up changes immediately with no packaging step.
- **Proper install**: `web/vscode-extension/Makefile` with targets mirroring `~/src/vscode-etc/Makefile`:
  - `build`: `vsce package`
  - `install`: `cursor --install-extension *.vsix --force`
  - `clean`, `uninstall`
- **Wormhole build integration**: Add extension build/install targets to the top-level [Makefile](Makefile).

## Integration test

Add `test_close_zen_mode_window` in [tests/test_integration.rs](tests/test_integration.rs):

1. Create and open a project (existing `create_project` pattern via `WormholeTest` harness)
2. Wait for the extension to connect (poll `/project/messages/<name>` consumer registry until `has_consumer` is true, or just sleep briefly)
3. Publish `editor/toggleZenMode` targeted at `role=editor` -- extension receives it, translates to `workbench.action.toggleZenMode`, Cursor enters zen mode
4. Wait briefly for zen mode to activate
5. `wormhole project close <name>` -- server publishes `editor/close` targeted at `role=editor`, extension translates to `workbench.action.closeWindow`
6. Assert `!window_exists(&name)` via Hammerspoon check -- this is the assertion that fails without the extension and passes with it

Add `publish_message(project, method, target)` helper to [tests/harness.rs](tests/harness.rs) that POSTs `{"target": "<target>", "message": {"jsonrpc": "2.0", "method": "<method>"}}` to the messages endpoint.

**Test prerequisites**: Headed tests require `WORMHOLE_TEST=1` and `WORMHOLE_EDITOR` not set to `none`. The wormhole VSCode extension must be installed in Cursor.

## Key existing code to reference

- [src/hammerspoon.rs](src/hammerspoon.rs): `close_window()` at line 47 -- the current close implementation being replaced
- [src/editor.rs](src/editor.rs): `Editor::close()` at line 103 -- where we publish the intent instead of calling Hammerspoon
- [src/batch.rs](src/batch.rs): `lazy_static! + Mutex + watch` pattern to follow for `messages.rs`
- [src/handlers/project.rs](src/handlers/project.rs): `poll_until()` at line 447 -- generic long-poll helper to reuse
- [tests/harness.rs](tests/harness.rs): `close_cursor_window()` at line 265, `window_exists()` at line 256 -- test helpers for verifying window state via Hammerspoon
- [tests/test_integration.rs](tests/test_integration.rs): `test_close_project` at line 112 -- existing close test to use as template
- `~/src/vscode-etc/Makefile`: Extension build/install pattern to follow

## What we are NOT doing

- Not adding a Hammerspoon fallback in `Editor::close` -- if the extension is not connected, close is a no-op
- Not replacing Hammerspoon for `launch_or_focus` (editor focus stays via Hammerspoon for now; the architecture supports migrating it later)
- Not adding bidirectional request/response communication (fire-and-forget notifications only; bidirectional deferred to future transport upgrade, likely SSE+POST or WebSocket)
- Not adding palette commands or UI to the extension
- Not adding a CLI message consumer (the architecture supports it, but it's not in scope)

