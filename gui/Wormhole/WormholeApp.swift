//
//  WormholeApp.swift
//  Wormhole
//

import SwiftUI
import AppKit

extension Notification.Name {
    static let modifierKeyReleased = Notification.Name("modifierKeyReleased")
}

@main
struct WormholeApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate

    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .windowStyle(.hiddenTitleBar)
    }
}

class AppDelegate: NSObject, NSApplicationDelegate {
    var localEventMonitor: Any?
    var wasOptionPressed = false

    func applicationDidFinishLaunching(_ notification: Notification) {
        wasOptionPressed = NSEvent.modifierFlags.contains(.option)

        localEventMonitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            let optionPressed = event.modifierFlags.contains(.option)
            if let self = self, self.wasOptionPressed && !optionPressed {
                NotificationCenter.default.post(name: .modifierKeyReleased, object: nil)
            }
            self?.wasOptionPressed = optionPressed
            return event
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        if let monitor = localEventMonitor {
            NSEvent.removeMonitor(monitor)
        }
    }
}
