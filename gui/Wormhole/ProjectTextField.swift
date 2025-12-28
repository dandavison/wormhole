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

        let searchFieldCell = searchField.cell!
        searchFieldCell.lineBreakMode = .byWordWrapping

        context.coordinator.searchField = searchField

        return searchField
    }

    func updateNSView(_ searchField: NSSearchField, context: Context) {
        let model = self.model
        let text = self.text

        let coordinator = context.coordinator
        coordinator.model = model

        coordinator.updatingSelectedRange = true
        defer {
            coordinator.updatingSelectedRange = false
        }

        if let selectedProject = model.selectedProject {
            let projectText = selectedProject.text

            if searchField.stringValue != projectText {
                searchField.stringValue = projectText
            }

            if let fieldEditor = searchField.window?.fieldEditor(false, for: searchField) {
                if model.projectConfirmed {
                    let range = NSRange(projectText.startIndex..<projectText.endIndex, in: fieldEditor.string)
                    if fieldEditor.selectedRange != range {
                        fieldEditor.selectedRange = range
                    }
                } else if projectText.hasPrefix(text) {
                    let range = NSRange(projectText.index(projectText.startIndex, offsetBy: text.count)..<projectText.index(projectText.startIndex, offsetBy: projectText.count), in: fieldEditor.string)
                    if fieldEditor.selectedRange != range {
                        fieldEditor.selectedRange = range
                    }
                }
            }
        } else {
            if searchField.stringValue != self.text {
                searchField.stringValue = self.text
            }
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

            if let event = NSApp.currentEvent, event.modifierFlags.contains(.shift), commandSelector == #selector(NSResponder.insertNewline(_:)) {
                if let project = self.model.selectedProject {
                    self.model.confirmProject(project, modifier: false)
                } else if self.model.projects.count == 1, let only = self.model.projects.first {
                    self.model.confirmProject(only, modifier: false)
                }

                return true
            }

            if commandSelector == #selector(NSResponder.insertNewline(_:)) {
                if let project = self.model.selectedProject {
                    self.model.confirmProject(project, modifier: true)
                } else if self.model.projects.count == 1, let only = self.model.projects.first {
                    // Auto-select when there's exactly one match
                    self.model.confirmProject(only, modifier: true)
                }

                return true
            }

            // Tab toggles between current and available projects
            if commandSelector == #selector(NSResponder.insertTab(_:)) {
                self.projectsModel.toggleMode()
                return true
            }

            return false
        }
    }
}
