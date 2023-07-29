import AppKit

@main
class AppDelegate: NSObject, NSApplicationDelegate {

    var window: NSWindow!

    func applicationDidFinishLaunching(_ aNotification: Notification) {
        let viewController = SearchableListViewController()
        window = NSWindow(contentViewController: viewController)
        window.makeKeyAndOrderFront(nil)
    }
}
