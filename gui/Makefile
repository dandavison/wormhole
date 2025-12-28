SCHEME = Wormhole
PROJECT = Wormhole.xcodeproj
BUILD_DIR = build
APP_NAME = Wormhole.app
DIST_DIR = dist/Wormhole

dist: build
	rm -rf $(DIST_DIR)
	mkdir -p $(DIST_DIR)
	cp -r $(BUILD_DIR)/$(APP_NAME) $(DIST_DIR)/

build:
	xcodebuild -project $(PROJECT) -scheme $(SCHEME) -configuration Release \
		-derivedDataPath $(BUILD_DIR) \
		CONFIGURATION_BUILD_DIR=$(CURDIR)/$(BUILD_DIR) > /dev/null

clean:
	rm -rf $(BUILD_DIR)
	rm -rf dist

.PHONY: build dist clean
