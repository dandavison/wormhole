serve:
	cargo build --release
	sudo TMUX=$$TMUX ./target/aarch64-apple-darwin/release/wormhole
