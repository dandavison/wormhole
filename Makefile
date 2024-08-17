serve: build
	./target/release/wormhole

serve-tmux: build
	TMUX=$$TMUX ./target/release/wormhole

build:
	cargo build --release
