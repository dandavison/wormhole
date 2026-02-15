import AppKit
import SwiftUI
import Combine

// original code from https://developer.apple.com/library/archive/samplecode/CustomMenus

struct ProjectTextField<V: Equatable>: NSViewRepresentable {
    @Binding var text: String
    @ObservedObject var model: ProjectSelectorModel<V>
    @ObservedObject var projectsModel: ProjectsModel

    func makeNSView(context: Context) -> NSSearchField {
        let searchField = NSSearchField(frame: .zero)
        searchField.controlSize = .regular
        searchField.font = NSFont.monospacedSystemFont(ofSize: NSFont.systemFontSize(for: searchField.controlSize), weight: NSFont.Weight(0))
        searchField.translatesAutoresizingMaskIntoConstraints = false
        searchField.setContentCompressionResistancePriority(NSLayoutConstraint.Priority(rawValue: 1), for: .horizontal)
        searchField.setContentHuggingPriority(NSLayoutConstraint.Priority(rawValue: 1), for: .horizontal)
        searchField.delegate = context.coordinator

        // Dark theme styling
        searchField.appearance = NSAppearance(named: .darkAqua)
        searchField.textColor = NSColor(red: 0.4, green: 0.8, blue: 1.0, alpha: 1.0) // Cyan text
        searchField.backgroundColor = NSColor(red: 0.05, green: 0.05, blue: 0.08, alpha: 1.0)
        searchField.drawsBackground = true
        searchField.focusRingType = .none
        searchField.bezelStyle = .roundedBezel
        searchField.wantsLayer = true
        searchField.layer?.borderColor = NSColor(red: 0.2, green: 0.6, blue: 0.8, alpha: 0.5).cgColor
        searchField.layer?.borderWidth = 1
        searchField.layer?.cornerRadius = 6
        searchField.placeholderString = ""

        let searchFieldCell = searchField.cell as! NSSearchFieldCell
        searchFieldCell.lineBreakMode = .byWordWrapping
        searchFieldCell.searchButtonCell = nil  // Remove magnifying glass
        searchFieldCell.cancelButtonCell = nil  // Remove X button

        context.coordinator.searchField = searchField

        return searchField
    }

    func updateNSView(_ searchField: NSSearchField, context: Context) {
        let coordinator = context.coordinator
        coordinator.model = self.model

        if searchField.stringValue != self.text {
            searchField.stringValue = self.text
        }
    }

    func makeCoordinator() -> Coordinator {
        return Coordinator(text: self.$text, model: self.model, projectsModel: self.projectsModel)
    }

    class Coordinator: NSObject, NSSearchFieldDelegate, NSWindowDelegate, NSTableViewDataSource, NSTableViewDelegate {
        @Binding var text: String
        var model: ProjectSelectorModel<V>
        var projectsModel: ProjectsModel
        var didChangeSelectionSubscription: AnyCancellable?
        var frameDidChangeSubscription: AnyCancellable?
        var keyEventMonitor: Any?
        var cursorTimer: Timer?
        var updatingSelectedRange: Bool = false

        init(text: Binding<String>, model: ProjectSelectorModel<V>, projectsModel: ProjectsModel) {
            self._text = text
            self.model = model
            self.projectsModel = projectsModel

            super.init()

            self.didChangeSelectionSubscription = NotificationCenter.default.publisher(for: NSTextView.didChangeSelectionNotification)
                .sink(receiveValue: { notification in
                    guard !self.updatingSelectedRange,
                          let fieldEditor = self.searchField.window?.fieldEditor(false, for: self.searchField),
                          let textView = notification.object as? NSTextView,
                          fieldEditor === textView else {
                        return
                    }
                    self.model.chooseProject(nil)
                })

            self.keyEventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { [weak self] event in
                // F13 key code is 105
                if event.keyCode == 105 {
                    self?.projectsModel.toggleMode()
                    return nil
                }
                return event
            }

            self.cursorTimer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
                guard let self = self,
                      let window = self.searchField?.window,
                      let fieldEditor = window.fieldEditor(false, for: self.searchField) as? NSTextView else {
                    return
                }
                fieldEditor.updateInsertionPointStateAndRestartTimer(true)
            }
        }

        deinit {
            cursorTimer?.invalidate()
            if let monitor = keyEventMonitor {
                NSEvent.removeMonitor(monitor)
            }
        }

        var searchField: NSSearchField! {
            didSet {
                if let searchField = self.searchField {
                    searchField.postsFrameChangedNotifications = true
                    self.frameDidChangeSubscription = NotificationCenter.default.publisher(for: NSView.frameDidChangeNotification, object: searchField)
                        .sink(receiveValue: { (_) in
                            self.model.width = self.searchField.frame.width
                        })
                } else {
                    self.frameDidChangeSubscription = nil
                }
            }
        }

        // MARK: - NSSearchField Delegate Methods

        @objc func controlTextDidChange(_ notification: Notification) {
            let text = self.searchField.stringValue

            self.model.modifiedText(text)
        }

        func controlTextDidEndEditing(_ obj: Notification) {
            self.model.cancel()
        }

        @objc func control(_ control: NSControl, textView: NSTextView, doCommandBy commandSelector: Selector) -> Bool {
            if commandSelector == #selector(NSResponder.moveUp(_:)) {
                guard self.model.projectsVisible else {
                    return false
                }
                self.model.moveUp()
                return true
            }

            if commandSelector == #selector(NSResponder.moveDown(_:)) {
                guard self.model.projectsVisible else {
                    return false
                }
                self.model.moveDown()
                return true
            }

            if commandSelector == #selector(NSResponder.complete(_:)) ||
                commandSelector == #selector(NSResponder.cancelOperation(_:)) {
                guard self.model.projectsVisible else {
                    return false
                }
                self.model.cancel()

                return true
            }

            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                let landInTerminal = NSApp.currentEvent?.modifierFlags.contains(.shift) ?? false
                if let project = self.model.selectedProject {
                    self.model.confirmProject(project, modifier: landInTerminal)
                } else if self.model.projects.count == 1, let only = self.model.projects.first {
                    // Auto-select when there's exactly one match
                    self.model.confirmProject(only, modifier: landInTerminal)
                }

                return true
            }

            return false
        }
    }
}
