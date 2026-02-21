install: clean build uninstall
	cursor --install-extension wormhole-*.vsix --force
	cursor --list-extensions --show-versions | grep wormhole

clean:
	@rm -f *.vsix

build:
	npm install
	npm run compile
	yes | npx vsce package

uninstall:
	cursor --uninstall-extension dandavison.wormhole || true

test:
	npm run compile
	node out/extension.test.js

dev-link:
	ln -sfn $(CURDIR) ~/.cursor/extensions/dandavison.wormhole

.PHONY: install clean build uninstall test dev-link
