use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Config, Context, Editor, Helper};

use crate::config;

/// HTTP client for communicating with the wormhole server
pub struct Client {
    base_url: String,
}

impl Client {
    pub fn new() -> Self {
        Self {
            base_url: format!("http://127.0.0.1:{}", config::wormhole_port()),
        }
    }

    pub(super) fn get(&self, path: &str) -> Result<String, String> {
        ureq::get(&format!("{}{}", self.base_url, path))
            .call()
            .map_err(map_ureq_error)?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }

    pub(super) fn post(&self, path: &str) -> Result<String, String> {
        ureq::post(&format!("{}{}", self.base_url, path))
            .call()
            .map_err(map_ureq_error)?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }

    pub(super) fn put(&self, path: &str, body: &str) -> Result<String, String> {
        ureq::put(&format!("{}{}", self.base_url, path))
            .send_string(body)
            .map_err(map_ureq_error)?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }

    pub(super) fn delete(&self, path: &str) -> Result<String, String> {
        ureq::delete(&format!("{}{}", self.base_url, path))
            .call()
            .map_err(map_ureq_error)?
            .into_string()
            .map_err(|e| format!("Failed to read response: {}", e))
    }
}

fn map_ureq_error(e: ureq::Error) -> String {
    match e {
        ureq::Error::Status(_code, response) => response
            .into_string()
            .unwrap_or_else(|_| "Unknown error".to_string()),
        e => format!("Request failed: {}", e),
    }
}

pub(super) struct ProjectCompleter {
    projects: Vec<String>,
}
impl Helper for ProjectCompleter {}
impl Validator for ProjectCompleter {}
impl Hinter for ProjectCompleter {
    type Hint = String;
}
impl Highlighter for ProjectCompleter {}
impl Completer for ProjectCompleter {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let prefix = &line[..pos];
        let candidates: Vec<Pair> = self
            .projects
            .iter()
            .filter(|p| p.starts_with(prefix))
            .map(|p| Pair {
                display: p.clone(),
                replacement: p.clone(),
            })
            .collect();
        Ok((0, candidates))
    }
}

pub(super) fn create_project_editor(
    available_projects: Vec<String>,
) -> Result<Editor<ProjectCompleter, rustyline::history::DefaultHistory>, String> {
    let config = Config::builder()
        .auto_add_history(false)
        .completion_type(rustyline::CompletionType::List)
        .build();
    let helper = ProjectCompleter {
        projects: available_projects,
    };
    let mut rl: Editor<ProjectCompleter, rustyline::history::DefaultHistory> =
        Editor::with_config(config).map_err(|e| format!("Failed to init editor: {}", e))?;
    rl.set_helper(Some(helper));
    Ok(rl)
}

pub(super) struct BranchCompleter {
    branches: Vec<String>,
}
impl Helper for BranchCompleter {}
impl Validator for BranchCompleter {}
impl Hinter for BranchCompleter {
    type Hint = String;
}
impl Highlighter for BranchCompleter {}
impl Completer for BranchCompleter {
    type Candidate = Pair;
    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let prefix = &line[..pos];
        let candidates: Vec<Pair> = self
            .branches
            .iter()
            .filter(|b| b.starts_with(prefix))
            .map(|b| Pair {
                display: b.clone(),
                replacement: b.clone(),
            })
            .collect();
        Ok((0, candidates))
    }
}

pub(super) fn create_branch_editor(
    branches: Vec<String>,
) -> Result<Editor<BranchCompleter, rustyline::history::DefaultHistory>, String> {
    let config = Config::builder()
        .auto_add_history(false)
        .completion_type(rustyline::CompletionType::List)
        .build();
    let helper = BranchCompleter { branches };
    let mut rl: Editor<BranchCompleter, rustyline::history::DefaultHistory> =
        Editor::with_config(config).map_err(|e| format!("Failed to init editor: {}", e))?;
    rl.set_helper(Some(helper));
    Ok(rl)
}

pub(super) fn get_available_projects(client: &Client) -> Result<Vec<String>, String> {
    let response = client.get("/project/list")?;
    let parsed: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    Ok(parsed
        .get("available")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default())
}

pub(super) fn parse_path_and_line(target: &str) -> (String, Option<usize>) {
    if let Some(idx) = target.rfind(':') {
        let (path, rest) = target.split_at(idx);
        if let Ok(line) = rest[1..].parse::<usize>() {
            if std::path::Path::new(path).exists() {
                return (path.to_string(), Some(line));
            }
        }
    }
    (target.to_string(), None)
}

pub(super) fn build_query(land_in: &Option<String>, line: &Option<usize>) -> String {
    let mut params = vec![];
    if let Some(app) = land_in {
        params.push(format!("land-in={}", app));
    }
    if let Some(n) = line {
        params.push(format!("line={}", n));
    }
    if params.is_empty() {
        String::new()
    } else {
        format!("?{}", params.join("&"))
    }
}

pub(super) fn build_switch_query(
    land_in: &Option<String>,
    name: &Option<String>,
    home_project: &Option<String>,
    branch: &Option<String>,
) -> String {
    let mut params = vec!["sync=true".to_string()];
    if let Some(app) = land_in {
        params.push(format!("land-in={}", app));
    }
    if let Some(n) = name {
        params.push(format!("name={}", n));
    }
    if let Some(h) = home_project {
        params.push(format!("home-project={}", h));
    }
    if let Some(b) = branch {
        params.push(format!("branch={}", b));
    }
    format!("?{}", params.join("&"))
}

pub(super) fn to_kebab_case(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            if c.is_alphanumeric() {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() || c == '-' || c == '_' {
                Some('-')
            } else {
                None
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Returns None if we should prompt, or Some(reason) if we should auto-skip.
/// Only skips issues that have a non-draft PR (work is already submitted).
pub(super) fn should_skip_issue(has_pr: bool) -> Option<&'static str> {
    if has_pr {
        return Some("has PR");
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_kebab_case() {
        assert_eq!(to_kebab_case("Hello World"), "hello-world");
        assert_eq!(
            to_kebab_case("[Jan] Standalone activity CLI integration"),
            "jan-standalone-activity-cli-integration"
        );
        assert_eq!(
            to_kebab_case("Fix PollActivityTaskQueueResponse proto doc comments"),
            "fix-pollactivitytaskqueueresponse-proto-doc-comments"
        );
        assert_eq!(to_kebab_case("Multiple   spaces"), "multiple-spaces");
    }

    #[test]
    fn test_should_skip_issues_with_pr() {
        assert_eq!(should_skip_issue(true), Some("has PR"));
    }

    #[test]
    fn test_should_not_skip_issues_without_pr() {
        assert_eq!(should_skip_issue(false), None);
    }
}
