import Foundation
import Combine

struct ProjectInfo: Codable {
    let name: String
    let home_project: String?
    let branch: String?

    var isTask: Bool { home_project != nil }

    var displayName: String {
        if let home = home_project, let branch = branch {
            return "\(home) \(name) \(branch)"
        }
        return name
    }
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
                $0.name.lowercased().contains(text) || ($0.branch?.lowercased().contains(text) ?? false)
            }
            self.projects = filtered.map { Project(text: $0.displayName, value: $0.name) }
        case .available:
            let filtered = text.isEmpty ? availableProjects : availableProjects.filter {
                $0.lowercased().contains(text)
            }
            self.projects = filtered.map { Project(text: $0, value: $0) }
        }
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
