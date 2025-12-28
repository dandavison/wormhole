See [Wormhole](https://github.com/dandavison/wormhole).

### To create MacOS application bundle in XCode

1. Select "Product" menu => "Archive", wait for build to complete
2. Select "Distribute App"
3. Select "Custom"
4. Select "Copy App"
5. `rm -r dist/Wormhole`
6. Save to `dist` directory with name `Wormhole`, so that `dist/Wormhole/Wormhole.app` is created
