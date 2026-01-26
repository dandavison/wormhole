build:
	cargo build --release

test:
	cargo nextest run --test test_integration --fail-fast --no-capture

reload: build
	./target/release/wormhole server start
	$(MAKE) -C gui clean dist

.PHONY: test serve serve-tmux build reload