serve: build
	./target/release/wormhole serve

build:
	cargo build --release

test:
	cargo nextest run --test test_integration --fail-fast --no-capture

.PHONY: test serve serve-tmux build