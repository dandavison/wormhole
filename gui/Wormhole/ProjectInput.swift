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
                              behavior: .semitransient,
                              anchor: .bottomLeading,
                              windowAnchor: .topLeading,
                              windowOffset: CGPoint(x: -28, y: -12)) {
                ProjectPopup(model: model)
                    .frame(width: model.width + 16)  // Match container width (340 + 8px padding each side)
                    .background(
                        UnevenRoundedRectangle(topLeadingRadius: 0, bottomLeadingRadius: 10, bottomTrailingRadius: 10, topTrailingRadius: 0)
                            .fill(Color(red: 0.02, green: 0.02, blue: 0.04))
                    )
                    .shadow(color: Color(white: 0, opacity: 0.10),
                            radius: 5, x: 0, y: 2)
                    .padding(20)
                    .font(.system(size: 12).monospaced())
            }
    }
}
