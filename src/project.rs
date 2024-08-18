use crate::project_path::ProjectPath;
use crate::util::{contract_user, expand_user, panic};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Project {
    pub name: String,
    pub path: PathBuf,
    pub aliases: Vec<String>,
}

impl Project {
    pub fn as_project_path(&self) -> ProjectPath {
        ProjectPath {
            project: (*self).clone(),
            relative_path: None,
        }
    }

    #[allow(dead_code)]
    pub fn root(&self) -> ProjectPath {
        ProjectPath {
            project: (*self).clone(),
            relative_path: Some(("".into(), None)),
        }
    }

    pub fn parse(line: &str) -> Self {
        let parts: Vec<&str> = line.split("->").collect();
        let path = PathBuf::from(expand_user(parts[0].trim()));
        let (name, aliases) = if parts.len() > 1 {
            let names: Vec<String> = parts[1].split(",").map(|s| s.trim().to_string()).collect();
            (names[0].clone(), names)
        } else {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| panic(&format!("invalid path: {}", path.display())));
            (name, vec![])
        };
        Self {
            name,
            path,
            aliases,
        }
    }

    pub fn format(&self) -> String {
        let mut s = contract_user(self.path.to_str().unwrap());
        if !self.aliases.is_empty() {
            s += &format!(" -> {}", self.aliases.join(", "));
        }
        s
    }

    pub fn is_terminal_only(&self) -> bool {
        self.name == "services"
    }
}
