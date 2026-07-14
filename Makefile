build:
	cargo build --release

gui:
	$(MAKE) -C gui clean dist

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

.PHONY: gui test serve serve-tmux build reload integration-test-ui-ask-for-permission-to-run extension-test vscode-extension vscode-extension-test
