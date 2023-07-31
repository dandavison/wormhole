//
//  ProjectsView.swift
//  ProjectsDemo
//
//  Created by Stephan Michels on 12.12.20.
//

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

struct ProjectGroupView<V: Equatable>: View {
    var projectGroup: ProjectGroup<V>
    var showDivider: Bool
    @ObservedObject var model: ProjectSelectorModel<V>
    
    var body: some View {
        let projectGroup = self.projectGroup
        let model = self.model
        
        return VStack(alignment: .leading) {
            if self.showDivider {
                Divider()
                    .padding(.top, 7)
            }
            if let title = projectGroup.title {
                Text(title)
                    .foregroundColor(.gray)
                    .font(.system(size: 12, weight: .bold).monospaced())
            }
            VStack(spacing: 0) {
                ForEach(Array(projectGroup.projects.enumerated()), id: \.0)  { (_, project) in
                    ProjectView(project: project, model: model)
                }
            }
        }
    }
}

struct ProjectPopup<V: Equatable>: View {
    @ObservedObject var model: ProjectSelectorModel<V>
    
    var body: some View {
        let model = self.model
        let projectGroups = model.projectGroups
        
        return VStack(spacing: 0) {
            ForEach(Array(projectGroups.enumerated()), id: \.0)  { (projectGroupIndex, projectGroup) in
                ProjectGroupView(projectGroup: projectGroup, showDivider: projectGroupIndex > 0, model: model)
            }
        }
        .padding(10)
    }
}

struct ProjectsView_Previews: PreviewProvider {
    static var previews: some View {
        let project1 = Project(text: "Eight", value: "Eight")
        let project2 = Project(text: "Elder", value: "Elder")
        let group = ProjectGroup(title: "English", projects: [project1, project2])
        let model = ProjectSelectorModel<String>()
        model.projectGroups = [group]
        model.selectedProject = project2
        
        return ProjectPopup(model: model)
    }
}
