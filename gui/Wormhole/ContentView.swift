//
//  ContentView.swift
//  ProjectsDemo
//
//  Created by Stephan Michels on 16.09.20.
//

import SwiftUI

struct ContentView: View {
    @StateObject var model = ProjectsModel()
    
    var body: some View {
        ProjectInput(text: self.$model.currentText,
                        projectGroups: self.model.projectGroups)
        .frame(width: 300)
    }
}

struct ContentView_Previews: PreviewProvider {
    static var previews: some View {
        ContentView()
    }
}
