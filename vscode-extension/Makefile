install: clean build uninstall
	code --install-extension wormhole-*.vsix --force
	code --list-extensions --show-versions | grep wormhole

clean:
	@rm -f *.vsix

build:
	npm install
	npm run compile
	yes | npx vsce package

uninstall:
	code --uninstall-extension dandavison.wormhole || true

test:
	npm run compile
	node out/extension.test.js

.PHONY: install clean build uninstall test dev-link
