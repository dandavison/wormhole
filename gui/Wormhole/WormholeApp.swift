//
//  WormholeApp.swift
//  Wormhole
//

import SwiftUI
import AppKit

extension Notification.Name {
    static let commandKeyReleased = Notification.Name("commandKeyReleased")
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
    var wasCommandPressed = false

    func applicationDidFinishLaunching(_ notification: Notification) {
        wasCommandPressed = NSEvent.modifierFlags.contains(.command)

        localEventMonitor = NSEvent.addLocalMonitorForEvents(matching: .flagsChanged) { [weak self] event in
            let commandPressed = event.modifierFlags.contains(.command)
            if let self = self, self.wasCommandPressed && !commandPressed {
                NotificationCenter.default.post(name: .commandKeyReleased, object: nil)
            }
            self?.wasCommandPressed = commandPressed
            return event
        }
    }

    func applicationWillTerminate(_ notification: Notification) {
        if let monitor = localEventMonitor {
            NSEvent.removeMonitor(monitor)
        }
    }
}
