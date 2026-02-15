import SwiftUI

struct ProjectView<V: Equatable>: View {
    var project: Project<V>
    @ObservedObject var model: ProjectSelectorModel<V>

    var body: some View {
        let project = self.project
        let model = self.model
        // Highlight if explicitly selected OR if it's the only match
        let isHighlighted = model.selectedProject == project || (model.projects.count == 1 && model.projects.first == project)

        return Text(project.text)
            .id(project.text)
            .frame(maxWidth: .infinity, alignment: .leading)
            .foregroundColor(isHighlighted ? .green : .cyan)
            .padding(EdgeInsets(top: 4, leading: 6, bottom: 4, trailing: 6))
            .background(ZStack {
                Color.black.ignoresSafeArea()
                RoundedRectangle(cornerRadius: 5)
                    .fill(Color.black)
                    .border(isHighlighted ? Color.green : Color.clear)
                }
            )
            .onTapGesture {
                model.confirmProject(project, modifier: false)
            }
    }
}

struct ProjectPopup<V: Equatable>: View {
    @ObservedObject var model: ProjectSelectorModel<V>

    var body: some View {
        let model = self.model
        let projects = model.projects

        return VStack(spacing: 0) {
            ForEach(projects.indices, id: \.self)  { idx in
                ProjectView(project: projects[idx], model: model)
            }
        }
        .padding(10)
    }
}
