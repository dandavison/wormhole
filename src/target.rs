//! What a piece of text names, and whether wormhole can open it.
//!
//! The text is usually typed by hand, but may be a fragment clicked in the
//! terminal, hence the tolerance for the shapes debuggers and compilers print.
//! Those shapes carry relative paths, so a caller supplies the directories to
//! try them against, most likely first.

use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq)]
pub enum Target {
    Conversation(PathBuf),
    File { path: PathBuf, line: Option<usize> },
    Directory(PathBuf),
    Project(String),
}

pub fn parse(text: &str, dirs: &[PathBuf]) -> Option<Target> {
    let text = text.trim();
    for (path, line) in path_candidates(text) {
        for resolved in dirs.iter().map(|dir| resolve(dir, path)) {
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
    }
    is_project_identifier(text).then(|| Target::Project(text.to_string()))
}

/// Ways the text might denote a path, longest path first. Covers `path`,
/// `path:line`, `path:line:column`, `path:line-line`, `path(line)` (pdb) and
/// `File "path", line N` (Python traceback).
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

/// Split off a trailing `:line`, or `:line-line` as a line range is written,
/// which is opened at its first line.
fn numeric_suffix(text: &str) -> Option<(&str, usize)> {
    let (head, tail) = text.rsplit_once(':')?;
    let start = match tail.split_once('-') {
        Some((start, end)) => end.parse::<usize>().ok().map(|_| start)?,
        None => tail,
    };
    Some((head, start.parse().ok()?))
}

fn resolve(dir: &Path, path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest),
        None => dir.join(path),
    }
}

/// A project name (`repo`), or a task identifier (`repo:branch`, or `:branch`
/// to search every repo for the branch). The repo part being a bare name is
/// what distinguishes `repo:branch` from `path:line`: a repo is a directory
/// name, so it carries no dot, whereas the text a compiler prints is a
/// filename with an extension.
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
            .all(|c| c.is_ascii_alphanumeric() || "_-".contains(c))
}

fn is_conversation_file(path: &Path) -> bool {
    let conversations_dir = std::fs::canonicalize(crate::conversations::conversations_dir())
        .unwrap_or_else(|_| crate::conversations::conversations_dir());
    let abs_path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    abs_path.starts_with(&conversations_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        dir: tempfile::TempDir,
        other: tempfile::TempDir,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(dir.path().join("src")).unwrap();
            std::fs::write(dir.path().join("src/main.rs"), "").unwrap();
            std::fs::write(dir.path().join("app.py"), "").unwrap();
            let other = tempfile::tempdir().unwrap();
            std::fs::write(other.path().join("only-here.rs"), "").unwrap();
            Self { dir, other }
        }

        /// Parse against the fixture directory alone.
        fn parse(&self, text: &str) -> Option<Target> {
            super::parse(text, &[self.dir.path().to_path_buf()])
        }

        /// Parse against the other directory first, as a second pane would.
        fn parse_in_either(&self, text: &str) -> Option<Target> {
            super::parse(
                text,
                &[
                    self.other.path().to_path_buf(),
                    self.dir.path().to_path_buf(),
                ],
            )
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
    fn path_with_line_range() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse("src/main.rs:91-95"),
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
    fn relative_path_is_tried_against_every_directory() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse_in_either("src/main.rs:91"),
            fixture.file("src/main.rs", Some(91))
        );
    }

    #[test]
    fn first_directory_holding_the_path_wins() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse_in_either("only-here.rs"),
            Some(Target::File {
                path: fixture
                    .other
                    .path()
                    .join("only-here.rs")
                    .canonicalize()
                    .unwrap(),
                line: None,
            })
        );
    }

    #[test]
    fn a_filename_with_a_line_range_is_not_a_task_identifier() {
        let fixture = Fixture::new();
        assert_eq!(fixture.parse("operator_commands.go:245-249."), None);
    }

    #[test]
    fn a_filename_with_a_line_is_not_a_task_identifier() {
        let fixture = Fixture::new();
        assert_eq!(fixture.parse("operator_commands.go:245"), None);
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
            fixture.parse("temporal:dan--fix"),
            Some(Target::Project("temporal:dan--fix".to_string()))
        );
    }

    #[test]
    fn branch_only_task_identifier() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse(":dan--fix"),
            Some(Target::Project(":dan--fix".to_string()))
        );
    }

    #[test]
    fn bare_project_name() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.parse("temporal"),
            Some(Target::Project("temporal".to_string()))
        );
    }

    #[test]
    fn prose_is_not_a_target() {
        let fixture = Fixture::new();
        assert_eq!(fixture.parse("the quick brown fox"), None);
    }
}
