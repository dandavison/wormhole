build:
	cargo build --release

gui:
	$(MAKE) -C gui clean dist

# Unit tests only: no server, no tmux, no window focus. The fast, always-safe
# target — use this by default, including from an agent.
test-unit:
	cargo nextest run -E 'not kind(test)'

# All tests, including the integration tests that spin up a server + tmux.
# WORMHOLE_EDITOR=none forces the server headless (config::editor() -> None), so
# no editor launches and no terminal/editor focus is grabbed. Safe but slow.
test:
	cargo build
	WORMHOLE_TEST=1 WORMHOLE_EDITOR=none cargo nextest run --fail-fast

integration-test-ui-ask-for-permission-to-run:
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

.PHONY: gui test test-unit serve serve-tmux build reload integration-test-ui-ask-for-permission-to-run extension-test vscode-extension vscode-extension-test
