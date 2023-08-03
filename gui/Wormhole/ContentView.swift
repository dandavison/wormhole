import SwiftUI

struct ContentView: View {
    @StateObject var model = ProjectsModel()
    
    var body: some View {
        ProjectInput(text: self.$model.currentText, projects: self.model.projects)
        .frame(width: 300)
    }
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
