serve:
	cargo build --release
	TMUX=$$TMUX ./target/release/wormhole
