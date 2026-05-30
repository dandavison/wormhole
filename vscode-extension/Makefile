WORMHOLE_EDITOR ?= code

ifeq ($(WORMHOLE_EDITOR),cursor)
EXTENSIONS_DIR := $(HOME)/.cursor/extensions
else ifeq ($(WORMHOLE_EDITOR),code-insiders)
EXTENSIONS_DIR := $(HOME)/.vscode-insiders/extensions
else
EXTENSIONS_DIR := $(HOME)/.vscode/extensions
endif

install: clean build uninstall
	$(WORMHOLE_EDITOR) --install-extension wormhole-*.vsix --force
	$(WORMHOLE_EDITOR) --list-extensions --show-versions | grep wormhole

clean:
	@rm -f *.vsix

build:
	npm install
	npm run compile
	yes | npx vsce package

uninstall:
	$(WORMHOLE_EDITOR) --uninstall-extension dandavison.wormhole || true

test:
	npm run compile
	node out/extension.test.js

dev-link:
	ln -sfn $(CURDIR) $(EXTENSIONS_DIR)/dandavison.wormhole

.PHONY: install clean build uninstall test dev-link
