build:
	cargo build --release

gui:
	$(MAKE) -C gui clean dist

# All tests, minus the ones that steal OS focus: WORMHOLE_EDITOR=none skips the
# editor-focus assertions so the integration tests exercise only tmux. Unit
# tests run in parallel; integration tests are serialized via the `integration`
# test-group in .config/nextest.toml.
test:
	cargo build
	WORMHOLE_TEST=1 WORMHOLE_EDITOR=none cargo nextest run --fail-fast

# The focus-stealing run: drives a real editor (Cursor) and asserts window
# focus, so it grabs your screen. Opt in explicitly.
integration-test:
	cargo build
	WORMHOLE_TEST=1 cargo nextest run --test '*' --fail-fast --no-capture

extension-test:
	cd chrome-extension && npm install && npm test

vscode-extension:
	$(MAKE) -C vscode-extension install

vscode-extension-test:
	$(MAKE) -C vscode-extension test

reload: build
	./target/release/wormhole server start

.PHONY: gui test serve serve-tmux build reload integration-test extension-test vscode-extension vscode-extension-test
