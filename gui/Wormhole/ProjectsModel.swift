import Foundation
import Combine

struct ProjectsResponse: Codable {
    let current: [String]
    let available: [String]
}

final class ProjectsModel: ObservableObject {
    var currentProjects: [String] = []
    var availableProjects: [String] = []

    @Published var showingAvailable: Bool = false
    @Published var currentText: String = ""
    @Published var projects: [Project<String>] = []
    @Published var currentProject: String?

    private var cancellables: Set<AnyCancellable> = []

    var activeProjectList: [String] {
        showingAvailable ? availableProjects : currentProjects
    }

    func fetchProjects() async throws -> ProjectsResponse {
        let url = URL(string: "http://localhost:7117/list-projects/")!
        let (data, _) = try await URLSession.shared.data(from: url)
        return try JSONDecoder().decode(ProjectsResponse.self, from: data)
    }

    func toggleMode() {
        showingAvailable.toggle()
        updateProjectsList()
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

                self.$showingAvailable
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
                print("Error while fetching projects: \(error)")
            }
        }
    }
}
