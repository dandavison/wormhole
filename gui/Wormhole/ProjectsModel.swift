//
//  ProjectsModel.swift
//  SuggestionsDemo
//
//  Created by Dan Davison on 7/30/23.
//

import Foundation
import Combine

final class ProjectsModel: ObservableObject {
    var projects: [String]

    @Published var currentText: String = ""
    @Published var projectGroups: [ProjectGroup<String>] = []
    @Published var currentProject: String?

    private var cancellables: Set<AnyCancellable> = []

    func fetchProjects() async throws -> [String] {
        let url = URL(string: "http://o/list-projects/")!
        let (data, _) = try await URLSession.shared.data(from: url)
        let str = String(data: data, encoding: .utf8)
        let projects = str?.components(separatedBy: "\n") ?? []
        return projects
    }

    init() {
        self.projects = []
        Task {
            do {
                self.projects = try await fetchProjects()
                self.$currentText
                    .removeDuplicates()
                    .map { text -> [ProjectGroup<String>] in
                        let text = text.lowercased()
                        let _projects = text.isEmpty ? self.projects : self.projects.lazy.filter({ $0.lowercased().contains(text) })
                        let projects = _projects.map { word -> Project<String> in
                            Project(text: word, value: word)
                        }
                        var projectGroups: [ProjectGroup<String>] = []
                        if !projects.isEmpty {
                            projectGroups.append(ProjectGroup<String>(title: "Projects", projects: Array(projects)))
                        }
                        return projectGroups
                    }
                    .receive(on: DispatchQueue.main)
                    .assign(to: \ProjectsModel.projectGroups, on: self)
                    .store(in: &cancellables)

                self.$currentText
                    .debounce(for: 0.3, scheduler: RunLoop.main)
                    .removeDuplicates()
                    .map { text -> String? in
                        return text
                    }
                    .receive(on: DispatchQueue.main)
                    .assign(to: \ProjectsModel.currentProject, on: self)
                    .store(in: &cancellables)
            } catch {
                print("Error while fetching projects / initializing project list")
            }
        }
    }
}
