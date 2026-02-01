import Foundation
import Combine

struct ProjectInfo: Codable {
    let project_key: String

    var isTask: Bool { project_key.contains(":") }

    var name: String {
        if let idx = project_key.firstIndex(of: ":") {
            return String(project_key[..<idx])
        }
        return project_key
    }

    var branch: String? {
        if let idx = project_key.firstIndex(of: ":") {
            return String(project_key[project_key.index(after: idx)...])
        }
        return nil
    }

    var repo: String { name }
}

struct ProjectsResponse: Codable {
    let current: [ProjectInfo]
    let available: [String]
}

enum SelectorMode: Int, CaseIterable {
    case current = 0
    case available = 1

    func next() -> SelectorMode {
        let allCases = SelectorMode.allCases
        let nextIndex = (self.rawValue + 1) % allCases.count
        return allCases[nextIndex]
    }
}

final class ProjectsModel: ObservableObject {
    var currentProjects: [ProjectInfo] = []
    var availableProjects: [String] = []

    @Published var mode: SelectorMode = .current
    @Published var currentText: String = ""
    @Published var projects: [Project<String>] = []
    @Published var currentProject: String?

    private var cancellables: Set<AnyCancellable> = []

    func fetchProjects() async throws -> ProjectsResponse {
        let url = URL(string: "http://localhost:7117/project/list")!
        let (data, _) = try await URLSession.shared.data(from: url)
        return try JSONDecoder().decode(ProjectsResponse.self, from: data)
    }

    func toggleMode() {
        DispatchQueue.main.async {
            self.mode = self.mode.next()
            self.updateProjectsList()
        }
    }

    private func updateProjectsList() {
        let text = currentText.lowercased()
        switch mode {
        case .current:
            let filtered = text.isEmpty ? currentProjects : currentProjects.filter {
                $0.name.lowercased().contains(text) ||
                ($0.branch?.lowercased().contains(text) ?? false)
            }
            self.projects = formatAligned(filtered, allProjects: currentProjects)
        case .available:
            let filtered = text.isEmpty ? availableProjects : availableProjects.filter {
                $0.lowercased().contains(text)
            }
            self.projects = filtered.map { Project(text: $0, value: $0) }
        }
    }

    private func formatAligned(_ projects: [ProjectInfo], allProjects: [ProjectInfo]) -> [Project<String>] {
        let allTasks = allProjects.filter { $0.isTask }
        let repoWidth = min(allTasks.map { $0.repo.count }.max() ?? 0, 16)
        let branchWidth = 60

        return projects.map { p in
            if let branch = p.branch {
                let col1 = truncateOrPad(p.repo, width: repoWidth)
                let col2 = truncate(branch, width: branchWidth)
                return Project(text: "\(col1)  \(col2)", value: p.project_key)
            }
            return Project(text: p.name, value: p.project_key)
        }
    }

    private func truncateOrPad(_ s: String, width: Int) -> String {
        if s.count > width {
            return String(s.prefix(width - 1)) + "…"
        }
        return s.padding(toLength: width, withPad: " ", startingAt: 0)
    }

    private func truncate(_ s: String, width: Int) -> String {
        if s.count > width {
            return String(s.prefix(width - 1)) + "…"
        }
        return s
    }

    init() {
        Task {
            do {
                let response = try await fetchProjects()

                await MainActor.run {
                    self.currentProjects = response.current
                    self.availableProjects = response.available
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
