import SwiftUI

struct ContentView: View {
    @StateObject var model = ProjectsModel()

    var body: some View {
        ZStack {
            Color(red: 0.02, green: 0.02, blue: 0.04)
                .ignoresSafeArea()
            ProjectInput(text: self.$model.currentText, projects: self.model.projects, projectsModel: model)
                .frame(width: 340)
                .padding(.horizontal, 16)
                .padding(.vertical, 12)
        }
        .background(WindowAccessor())
    }
}

// Helper to customize the window
struct WindowAccessor: NSViewRepresentable {
    func makeNSView(context: Context) -> NSView {
        let view = NSView()
        DispatchQueue.main.async {
            if let window = view.window {
                window.isOpaque = false
                window.backgroundColor = NSColor(red: 0.02, green: 0.02, blue: 0.04, alpha: 1.0)
                window.hasShadow = true
                // Make corners rounded
                window.contentView?.wantsLayer = true
                window.contentView?.layer?.cornerRadius = 10
                window.contentView?.layer?.masksToBounds = true
                // Hide traffic light buttons
                window.standardWindowButton(.closeButton)?.isHidden = true
                window.standardWindowButton(.miniaturizeButton)?.isHidden = true
                window.standardWindowButton(.zoomButton)?.isHidden = true
                window.isMovableByWindowBackground = true
            }
        }
        return view
    }

    func updateNSView(_ nsView: NSView, context: Context) {}
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
