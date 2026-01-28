build:
	cargo build --release

integration-test:
	cargo nextest run --test test_integration --fail-fast --no-capture

extension-test:
	cd web/chrome-extension && npm install && npm test

reload: build
	./target/release/wormhole server start
	$(MAKE) -C gui clean dist

.PHONY: test serve serve-tmux build reload integration-test extension-test