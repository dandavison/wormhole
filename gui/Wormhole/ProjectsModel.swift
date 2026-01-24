import Foundation
import Combine

struct ProjectsResponse: Codable {
    let current: [String]
    let available: [String]
}

struct TasksResponse: Codable {
    let tasks: [TaskInfo]
}

struct TaskInfo: Codable {
    let id: String
    let home_repo: String
    let worktree_path: String
}

enum SelectorMode: Int, CaseIterable {
    case tasks = 0
    case current = 1
    case available = 2

    func next() -> SelectorMode {
        let allCases = SelectorMode.allCases
        let nextIndex = (self.rawValue + 1) % allCases.count
        return allCases[nextIndex]
    }
}

final class ProjectsModel: ObservableObject {
    var currentProjects: [String] = []
    var availableProjects: [String] = []
    var tasks: [String] = []

    @Published var mode: SelectorMode = .tasks
    @Published var currentText: String = ""
    @Published var projects: [Project<String>] = []
    @Published var currentProject: String?

    private var cancellables: Set<AnyCancellable> = []

    var activeProjectList: [String] {
        switch mode {
        case .current:
            return currentProjects
        case .available:
            return availableProjects
        case .tasks:
            return tasks
        }
    }

    var isTaskMode: Bool {
        mode == .tasks
    }

    func fetchProjects() async throws -> ProjectsResponse {
        let url = URL(string: "http://localhost:7117/projects")!
        let (data, _) = try await URLSession.shared.data(from: url)
        return try JSONDecoder().decode(ProjectsResponse.self, from: data)
    }

    func fetchTasks() async throws -> TasksResponse {
        let url = URL(string: "http://localhost:7117/tasks")!
        let (data, _) = try await URLSession.shared.data(from: url)
        return try JSONDecoder().decode(TasksResponse.self, from: data)
    }

    func toggleMode() {
        DispatchQueue.main.async {
            self.mode = self.mode.next()
            self.updateProjectsList()
        }
    }

    private func updateProjectsList() {
        let text = currentText.lowercased()
        let source = activeProjectList
        let filtered = text.isEmpty ? source : source.filter { $0.lowercased().contains(text) }
        self.projects = filtered.map { Project(text: $0, value: $0) }
    }

    init() {
        Task {
            do {
                async let projectsTask = fetchProjects()
                async let tasksTask = fetchTasks()

                let (projectsResponse, tasksResponse) = try await (projectsTask, tasksTask)

                await MainActor.run {
                    self.currentProjects = projectsResponse.current
                    self.availableProjects = projectsResponse.available
                    self.tasks = tasksResponse.tasks.map { $0.id }
                    self.updateProjectsList()
                }

                self.$currentText
                    .debounce(for: 0.1, scheduler: RunLoop.main)
                    .removeDuplicates()
                    .sink { [weak self] _ in
                        self?.updateProjectsList()
                    }
                    .store(in: &cancellables)

                self.$mode
                    .receive(on: DispatchQueue.main)
                    .sink { [weak self] _ in
                        self?.updateProjectsList()
                    }
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
                print("Error while fetching data: \(error)")
            }
        }
    }
}
