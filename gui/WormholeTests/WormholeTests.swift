//
//  WormholeTests.swift
//  WormholeTests
//
//  Created by Dan Davison on 7/30/23.
//

import XCTest
@testable import Wormhole

final class WormholeTests: XCTestCase {

    @MainActor
    func testAvailableModeIncludesTasks() {
        let model = ProjectsModel(fetchOnStart: false)
        model.allProjects = [
            ProjectInfo(project_key: "temporal:feature-branch"),
            ProjectInfo(project_key: "wormhole"),
        ]
        model.mode = .available

        model.updateProjectsList()

        XCTAssertEqual(
            model.projects.map(\.value),
            ["temporal:feature-branch", "wormhole"]
        )
    }

    func testAllSelectableProjectsCombinesWorkspacesAndTasks() {
        let response = ProjectsResponse(
            current: [
                ProjectInfo(project_key: "temporal:feature-branch"),
                ProjectInfo(project_key: "wormhole"),
            ],
            available: [
                "temporal",
                "wormhole",
            ]
        )

        XCTAssertEqual(
            response.allSelectableProjects().map(\.project_key),
            ["temporal", "temporal:feature-branch", "wormhole"]
        )
    }
}
