import Foundation
import Combine

final class ProjectsModel: ObservableObject {
    var projectNames: [String]

    @Published var currentText: String = ""
    @Published var projects: [Project<String>] = []
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
        self.projectNames = []
        Task {
            do {
                self.projectNames = try await fetchProjects()
                // self.currentText = projects[0]
                self.$currentText
                    .debounce(for: 0.3, scheduler: RunLoop.main)
                    .removeDuplicates()
                    .map { text -> [Project<String>] in
                        let text = text.lowercased()
                        let projects = text.isEmpty ? self.projectNames : self.projectNames.lazy.filter({ $0.lowercased().contains(text) })
                        return projects.map { word -> Project<String> in
                            Project(text: word, value: word)
                        }
                    }
                    .receive(on: DispatchQueue.main)
                    .assign(to: \ProjectsModel.projects, on: self)
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
