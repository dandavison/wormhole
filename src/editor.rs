use crate::{config, project_path::ProjectPath, util::execute_command, wormhole::WindowAction};

#[allow(dead_code)]
#[derive(Debug)]
pub enum Editor {
    IntelliJ,
    VSCode,
    VSCodeInsiders,
    PyCharm,
    PyCharmCE,
}
use Editor::*;

impl Editor {
    pub fn application_name(&self) -> &'static str {
        match self {
            VSCodeInsiders => "Code - Insiders",
            VSCode => "Code",
            PyCharm => "PyCharm",
            PyCharmCE => "PyCharm",
            IntelliJ => "IntelliJ",
        }
    }
    pub fn macos_application_bundle_name(&self) -> &'static str {
        match self {
            VSCodeInsiders => "Visual Studio Code - Insiders",
            VSCode => "Visual Studio Code",
            PyCharm => "PyCharm",
            PyCharmCE => "PyCharm CE",
            IntelliJ => "IntelliJ IDEA",
        }
    }
}

pub fn open_path(path: &ProjectPath, window_action: WindowAction) -> Result<(), String> {
    let project_path = path.absolute_path();
    match window_action {
        WindowAction::Raise => {
            execute_command(
                config::EDITOR.application_name(),
                // HACK: VSCode-specific
                ["-g", project_path.to_str().unwrap()],
                &path.project.path,
            );
        }
        WindowAction::Focus => {
            execute_command(
                "open",
                [
                    "-g",
                    "-a",
                    config::EDITOR.macos_application_bundle_name(),
                    project_path.to_str().unwrap(),
                ],
                &path.project.path,
            );
        }
    }
    Ok(())
}
