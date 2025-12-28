import SwiftUI

struct ContentView: View {
    @StateObject var model = ProjectsModel()

    var body: some View {
        ZStack {
            Color.black.ignoresSafeArea()
            ProjectInput(text: self.$model.currentText, projects: self.model.projects, projectsModel: model)
            .frame(width: 300)
        }
    }
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
