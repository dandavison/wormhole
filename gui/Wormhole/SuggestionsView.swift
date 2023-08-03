import SwiftUI

struct ProjectView<V: Equatable>: View {
    var project: Project<V>
    @ObservedObject var model: ProjectSelectorModel<V>
    
    var body: some View {
        let project = self.project
        let model = self.model
        
        return Text(project.text)
            .id(project.text)
            .frame(maxWidth: .infinity, alignment: .leading)
            .foregroundColor(model.selectedProject == project ? .white : .primary)
            .padding(EdgeInsets(top: 4, leading: 6, bottom: 4, trailing: 6))
            .background(
                RoundedRectangle(cornerRadius: 5)
                    .foregroundColor(model.selectedProject == project ? Color.accentColor : Color.clear)
            )
            .onHover(perform: { hovering in
                if hovering {
                    model.chooseProject(project)
                } else if model.selectedProject == project {
                    model.chooseProject(nil)
                }
            })
            .onTapGesture {
                model.confirmProject(project)
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
