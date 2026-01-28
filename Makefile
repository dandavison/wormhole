build:
	cargo build --release

test:
	cargo nextest run --bin wormhole --fail-fast

integration-test:
	cargo nextest run --test '*' --fail-fast --no-capture

extension-test:
	cd web/chrome-extension && npm install && npm test

reload: build
	./target/release/wormhole server start
	$(MAKE) -C gui clean dist

.PHONY: test serve serve-tmux build reload integration-test extension-test