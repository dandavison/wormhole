//
//  ProjectInput.swift
//  ProjectsDemo
//
//  Created by Stephan Michels on 13.12.20.
//

import SwiftUI

struct Project<V: Equatable>: Equatable {
    var text: String = ""
    var value: V
    
    static func ==(_ lhs: Project<V>, _ rhs: Project<V>) -> Bool {
        return lhs.value == rhs.value
    }
}

struct ProjectGroup<V: Equatable>: Equatable {
    var title: String?
    var projects: [Project<V>]
    
    static func ==(_ lhs: ProjectGroup<V>, _ rhs: ProjectGroup<V>) -> Bool {
        return lhs.projects == rhs.projects
    }
}

struct ProjectInput<V: Equatable>: View {
    @Binding var text: String
    var projectGroups: [ProjectGroup<V>]
    
    @StateObject var model = ProjectSelectorModel<V>()
    
    var body: some View {
        let model = self.model
        if model.projectGroups != self.projectGroups {
            model.projectGroups = self.projectGroups
            
            model.selectedProject = nil
        }
        model.textBinding = self.$text
        
        return ProjectTextField(text: self.$text, model: model)
            .borderlessWindow(isVisible: Binding<Bool>(get: { model.projectsVisible && !model.projectGroups.isEmpty }, set: { model.projectsVisible = $0 }),
                              behavior: .transient,
                              anchor: .bottomLeading,
                              windowAnchor: .topLeading,
                              windowOffset: CGPoint(x: -20, y: -16)) {
                ProjectPopup(model: model)
                    .frame(width: model.width)
                    .background(VisualEffectBlur(material: .popover, blendingMode: .behindWindow, cornerRadius: 8))
                .overlay(RoundedRectangle(cornerRadius: 8)
                            .stroke(lineWidth: 1)
                            .foregroundColor(Color(white: 0.6, opacity: 0.2))
                )
                .shadow(color: Color(white: 0, opacity: 0.10),
                        radius: 5, x: 0, y: 2)
                .padding(20)
            }
    }
}
