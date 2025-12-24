serve: build
	./target/release/wormhole

build:
	cargo build --release

test:
	cargo test --test test_integration -- --test-threads=1 --nocapture

.PHONY: test serve serve-tmux build