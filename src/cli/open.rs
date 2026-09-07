//! Resolve a target to something wormhole can open. The target is usually typed
//! by hand, but may be a fragment of text clicked in the terminal, hence the
//! tolerance for the shapes debuggers and compilers print.
//!
//! `no_create` is for the latter case: it suppresses everything that would
//! bring a task into existence, so that text naming nothing that already exists
//! is an error rather than a new worktree.

use std::path::{Path, PathBuf};

use super::util::*;
use super::{is_conversation_file, task};

pub(super) fn run(
    client: &Client,
    target: &str,
    land_in: Option<String>,
    home_project: Option<String>,
    dry_run: bool,
    no_create: bool,
) -> Result<(), String> {
    if task::is_create_ref(target) && !no_create {
        return task::task_create(client, target, home_project, land_in, dry_run);
    }
    let cwd = std::env::current_dir().map_err(|e| format!("Failed to read cwd: {}", e))?;
    match parse(target, &cwd).ok_or_else(|| format!("No wormhole target in {:?}", target))? {
        Target::Conversation(path) => {
            client.post(&format!("/conversations/resume/{}", path.display()))?;
        }
        Target::File { path, line } => {
            let query = build_query(&Some("editor".to_string()), &line);
            client.get(&format!("/file/{}{}", path.display(), query))?;
        }
        Target::Directory(path) => {
            let query = build_switch_query(&land_in, &None, &None, &None);
            client.get(&format!("/project/switch/{}{}", path.display(), query))?;
        }
        Target::Project(name) => {
            if no_create && !is_known_project(client, &name)? {
                return Err(format!("Unknown project '{}'", name));
            }
            let query = build_switch_query(&land_in, &None, &None, &None);
            client.get(&format!("/project/switch/{}{}", name, query))?;
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
enum Target {
    Conversation(PathBuf),
    File { path: PathBuf, line: Option<usize> },
    Directory(PathBuf),
    Project(String),
}

fn parse(text: &str, cwd: &Path) -> Option<Target> {
    let text = text.trim();
    for (path, line) in path_candidates(text) {
        let resolved = resolve(cwd, path);
        if resolved.is_file() {
            let path = resolved.canonicalize().unwrap_or(resolved);
            return Some(match is_conversation_file(&path) {
                true => Target::Conversation(path),
                false => Target::File { path, line },
            });
        }
        if line.is_none() && resolved.is_dir() {
            return Some(Target::Directory(
                resolved.canonicalize().unwrap_or(resolved),
            ));
        }
    }
    is_project_identifier(text).then(|| Target::Project(text.to_string()))
}

/// Ways the text might denote a path, longest path first. Covers `path`,
/// `path:line`, `path:line:column`, `path(line)` (pdb) and `File "path", line
/// N` (Python traceback).
fn path_candidates(text: &str) -> Vec<(&str, Option<usize>)> {
    if let Some(candidate) = python_traceback(text).or_else(|| parenthesized_line(text)) {
        return vec![candidate];
    }
    let mut candidates = vec![(text, None)];
    if let Some((head, trailing)) = numeric_suffix(text) {
        candidates.push((head, Some(trailing)));
        if let Some((head, line)) = numeric_suffix(head) {
            candidates.push((head, Some(line))); // `trailing` was a column
        }
    }
    candidates
}

fn python_traceback(text: &str) -> Option<(&str, Option<usize>)> {
    let rest = text.strip_prefix("File \"")?;
    let (path, rest) = rest.split_once('"')?;
    let line = rest.strip_prefix(", line ")?.parse().ok()?;
    Some((path, Some(line)))
}

fn parenthesized_line(text: &str) -> Option<(&str, Option<usize>)> {
    let (path, line) = text.strip_suffix(')')?.rsplit_once('(')?;
    Some((path, Some(line.parse().ok()?)))
}

fn numeric_suffix(text: &str) -> Option<(&str, usize)> {
    let (head, tail) = text.rsplit_once(':')?;
    Some((head, tail.parse().ok()?))
}

fn resolve(cwd: &Path, path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => cwd.join(path),
    }
}

/// A project name (`repo`), or a task identifier (`repo:branch`, or `:branch`
/// to search every repo for the branch). The repo part being a bare name is
/// what distinguishes `repo:branch` from `path:line`.
fn is_project_identifier(text: &str) -> bool {
    let Some((repo, branch)) = text.split_once(':') else {
        return is_bare_name(text);
    };
    !branch.is_empty()
        && !branch.contains(char::is_whitespace)
        && (repo.is_empty() || is_bare_name(repo))
}

fn is_bare_name(text: &str) -> bool {
    text.starts_with(|c: char| c.is_ascii_alphanumeric())
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

fn is_known_project(client: &Client, name: &str) -> Result<bool, String> {
    let response = client.get("/project/worktrees")?;
    let projects: serde_json::Value = serde_json::from_str(&response).map_err(|e| e.to_string())?;
    let projects = projects.as_array().ok_or("Expected an array of projects")?;
    Ok(projects.iter().any(|project| {
        [
            project.get("project_key"),
            project.get("repo").filter(|_| !name.contains(':')),
        ]
        .iter()
        .flatten()
        .any(|value| value.as_str() == Some(name))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        dir: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join("src")).unwrap();
            std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
            std::fs::write(dir.path().join("app.py"), "").unwrap();
            Self { dir }
        }

        fn parse(&self, text: &str) -> Option<Target> {
            super::parse(text, self.dir.path())
        }

        fn file(&self, relative_path: &str, line: Option<usize>) -> Option<Target> {
            Some(Target::File {
                path: self.dir.path().join(relative_path).canonicalize().unwrap(),
                line,
            })
        }
    }

    #[test]
    fn relative_path_with_line() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse("src/main.rs:91"),
            fixture.file("src/main.rs", Some(91))
        );
    }

    #[test]
    fn relative_path_without_line() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse("src/main.rs"),
            fixture.file("src/main.rs", None)
        );
    }

    #[test]
    fn path_with_line_and_column() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse("src/main.rs:91:5"),
            fixture.file("src/main.rs", Some(91))
        );
    }

    #[test]
    fn absolute_path() {
        let fixture = Fixture::new();
        let absolute = fixture.dir.path().join("app.py");
        assert_eq!(
            fixture.parse(&format!("{}:3", absolute.display())),
            fixture.file("app.py", Some(3))
        );
    }

    #[test]
    fn python_traceback_frame() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse("File \"app.py\", line 42"),
            fixture.file("app.py", Some(42))
        );
    }

    #[test]
    fn pdb_frame() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse("app.py(42)"),
            fixture.file("app.py", Some(42))
        );
    }

    #[test]
    fn directory() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse("src"),
            Some(Target::Directory(
                fixture.dir.path().join("src").canonicalize().unwrap()
            ))
        );
    }

    #[test]
    fn task_identifier() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse("wormhole:dan/branch"),
            Some(Target::Project("wormhole:dan/branch".to_string()))
        );
    }

    #[test]
    fn branch_only_task_identifier() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse(":dan/branch"),
            Some(Target::Project(":dan/branch".to_string()))
        );
    }

    #[test]
    fn bare_project_name() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse("wormhole"),
            Some(Target::Project("wormhole".to_string()))
        );
    }

    #[test]
    fn nonexistent_path_is_not_a_project() {
        let fixture = Fixture::new();
        assert_eq!(fixture.parse("src/nope.rs:1"), None);
    }

    #[test]
    fn prose_is_not_a_target() {
        let fixture = Fixture::new();
        assert_eq!(fixture.parse("see the docs"), None);
    }
}
