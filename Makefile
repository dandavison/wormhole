serve: build
	./target/release/wormhole serve

build:
	cargo build --release

test:
	@terminal-notifier -message "wormhole tests running" -title "wormhole tests" -group wormhole-tests >/dev/null 2>&1 || true
	@cargo nextest run --test test_integration --fail-fast --no-capture; status=$$?; \
		terminal-notifier -remove wormhole-tests >/dev/null 2>&1 || true; \
		exit $$status

.PHONY: test serve serve-tmux build