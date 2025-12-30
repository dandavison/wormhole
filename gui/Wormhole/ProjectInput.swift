import SwiftUI

struct Project<V: Equatable>: Equatable {
    var text: String = ""
    var value: V

    static func ==(_ lhs: Project<V>, _ rhs: Project<V>) -> Bool {
        return lhs.value == rhs.value
    }
}

struct ProjectInput<V: Equatable>: View {
    @Binding var text: String
    var projects: [Project<V>]
    @ObservedObject var projectsModel: ProjectsModel

    @StateObject var model = ProjectSelectorModel<V>()

    var body: some View {
        let model = self.model
        if model.projects != self.projects {
            model.projects = self.projects

            model.selectedProject = nil
        }
        model.textBinding = self.$text

        return ProjectTextField(text: self.$text, model: model, projectsModel: projectsModel)
            .borderlessWindow(isVisible: Binding<Bool>(get: { model.projectsVisible && !model.projects.isEmpty }, set: { model.projectsVisible = $0 }),
                              behavior: .transient,
                              anchor: .bottomLeading,
                              windowAnchor: .topLeading,
                              windowOffset: CGPoint(x: -20, y: -16)) {
                ProjectPopup(model: model)
                    .frame(width: model.width)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(Color(red: 0.08, green: 0.08, blue: 0.10))
                    )
                    .padding(20)
                    .font(.system(size: 12).monospaced())
                    .drawingGroup()
            }
    }
}
