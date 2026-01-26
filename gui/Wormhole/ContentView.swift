import SwiftUI

struct ContentView: View {
    @StateObject var model = ProjectsModel()

    var body: some View {
        ProjectInput(text: self.$model.currentText, projects: self.model.projects, projectsModel: model)
            .frame(width: 740)
            .padding(8)
            .background(
                UnevenRoundedRectangle(topLeadingRadius: 10, bottomLeadingRadius: 0, bottomTrailingRadius: 0, topTrailingRadius: 10)
                    .fill(Color(red: 0.02, green: 0.02, blue: 0.04))
            )
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
                window.backgroundColor = .clear
                window.hasShadow = false
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
