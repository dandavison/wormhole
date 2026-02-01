use crate::config;
use crate::editor::Editor;
use crate::git;
use crate::project_path::ProjectPath;
use crate::wormhole::Application;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RepoName(String);

impl RepoName {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for RepoName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BranchName(String);

impl BranchName {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProjectKey {
    pub repo: RepoName,
    pub branch: Option<BranchName>,
}

impl ProjectKey {
    pub fn project(repo: impl Into<String>) -> Self {
        Self {
            repo: RepoName::new(repo),
            branch: None,
        }
    }

    pub fn task(repo: impl Into<String>, branch: impl Into<String>) -> Self {
        Self {
            repo: RepoName::new(repo),
            branch: Some(BranchName::new(branch)),
        }
    }

    pub fn parse(s: &str) -> Self {
        match s.split_once(':') {
            Some((repo, branch)) => Self::task(repo, branch),
            None => Self::project(s),
        }
    }
}

impl fmt::Display for ProjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.branch {
            Some(branch) => write!(f, "{}:{}", self.repo, branch),
            None => write!(f, "{}", self.repo),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Project {
    pub repo_name: RepoName,
    pub repo_path: PathBuf,
    pub kv: HashMap<String, String>,
    pub last_application: Option<Application>,
    pub branch: Option<BranchName>,
    pub github_pr: Option<u64>,
    pub github_repo: Option<String>,
    pub cached_jira: Option<crate::jira::IssueStatus>,
    pub cached_pr: Option<crate::github::PrStatus>,
}

impl Project {
    pub fn is_task(&self) -> bool {
        self.branch.is_some()
    }

    pub fn store_key(&self) -> ProjectKey {
        match &self.branch {
            Some(branch) => ProjectKey::task(self.repo_name.as_str(), branch.as_str()),
            None => ProjectKey::project(self.repo_name.as_str()),
        }
    }

    pub fn worktree_path(&self) -> Option<PathBuf> {
        self.branch.as_ref().map(|branch| {
            git::worktree_base_path(&self.repo_path)
                .join(git::encode_branch_for_path(branch.as_str()))
        })
    }

    pub fn working_tree(&self) -> PathBuf {
        self.worktree_path()
            .unwrap_or_else(|| self.repo_path.clone())
    }

    pub fn is_open(&self) -> bool {
        config::TERMINAL.exists(self)
    }

    pub fn as_project_path(&self) -> ProjectPath {
        ProjectPath {
            project: (*self).clone(),
            relative_path: None,
        }
    }

    pub fn root(&self) -> ProjectPath {
        ProjectPath {
            project: (*self).clone(),
            relative_path: Some(("".into(), None)),
        }
    }

    pub fn editor(&self) -> &'static Editor {
        if self.repo_name.as_str() == "mathematics" {
            &Editor::Emacs
        } else {
            config::editor()
        }
    }
}
