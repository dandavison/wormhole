//! The member branches of a project.
//!
//! A project is identified by a single branch, and its worktree lives at a
//! path derived from that branch. Work on one project often spans several
//! branches (a stack of PRs, a follow-up, a rebased copy). The
//! `project_branches` table in `wormhole.toml` declares which further
//! branches belong to a project instead of becoming projects of their own:
//!
//! ```toml
//! [project_branches]
//! # The key is a glob over project keys, so this entry applies to every
//! # project, extending its own branch.
//! "*" = ["{project}-*", "{project}/*"]
//! # This one applies to one project: literal names, globs, or /regexes/.
//! "wormhole:multi-branch-tasks" = ["mbt-fixups", "mbt/*", "/^stack-[0-9]+$/"]
//! ```
//!
//! A pattern is a glob unless delimited by `/`, in which case the text
//! between the delimiters is a regex; both must match the whole branch name.
//! `{project}` stands for the project's own branch. A project's patterns are
//! the union of those of every entry whose key glob matches its `repo:branch`
//! key.
//!
//! A branch naming a project of its own is that project whatever the table
//! says. Membership is otherwise exclusive, and never decided by contest: a
//! branch claimed by two projects belongs to neither, and the operation that
//! asked for it fails.

use std::path::Path;

use glob::Pattern;
use regex::Regex;

use crate::git;
use crate::util::error;

const PROJECT_PLACEHOLDER: &str = "{project}";

#[derive(Debug, Clone, Default)]
pub struct ProjectBranches {
    entries: Vec<Entry>,
}

#[derive(Debug, Clone)]
struct Entry {
    key: Pattern,
    patterns: Vec<BranchPattern>,
}

#[derive(Debug, Clone)]
enum BranchPattern {
    Glob(String),
    Regex(String),
}

impl ProjectBranches {
    /// Keep the well-formed entries and patterns, reporting and dropping the rest.
    pub fn new(entries: impl IntoIterator<Item = (String, Vec<String>)>) -> Self {
        let entries = entries
            .into_iter()
            .filter_map(|(key, patterns)| {
                let compiled = match Pattern::new(&key) {
                    Ok(compiled) => compiled,
                    Err(e) => {
                        error(&format!("Invalid project glob {key:?}: {e}"));
                        return None;
                    }
                };
                Some(Entry {
                    key: compiled,
                    patterns: patterns
                        .into_iter()
                        .filter_map(BranchPattern::new)
                        .collect(),
                })
            })
            .collect();
        Self { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.iter().all(|entry| entry.patterns.is_empty())
    }

    /// Whether `branch` is a member branch of the project `repo:project_branch`.
    /// A project's own branch is its identity, not a member branch.
    pub fn links(&self, repo: &str, project_branch: &str, branch: &str) -> bool {
        if project_branch == branch {
            return false;
        }
        let key = format!("{repo}:{project_branch}");
        self.entries
            .iter()
            .filter(|entry| entry.key.matches(&key))
            .flat_map(|entry| &entry.patterns)
            .any(|pattern| pattern.matches(project_branch, branch))
    }

    /// Of `project_branches` in `repo`, the branch of the project whose member
    /// branches include `branch`. A branch naming a project of its own is that
    /// project, so it has no owner to look for.
    pub fn owning_project_branch<'a>(
        &self,
        project_branches: impl IntoIterator<Item = &'a str>,
        repo: &str,
        branch: &str,
    ) -> Result<Option<&'a str>, String> {
        let mut owners = Vec::new();
        for candidate in project_branches {
            if candidate == branch {
                return Ok(None);
            }
            if self.links(repo, candidate, branch) {
                owners.push(candidate);
            }
        }
        one_owner(repo, branch, owners)
    }

    /// The branch identifying the project whose worktree is at `worktree_path`
    /// with `checked_out` checked out.
    ///
    /// Worktrees live at `<worktree_dir>/<repo>/<encoded-branch>/<repo>`, so
    /// normally the directory encodes the checked-out branch and that branch
    /// is the identity. When it doesn't, the worktree was created for another
    /// branch. That branch remains the project's identity if the checked-out
    /// branch is a member branch of it, whether or not it still exists
    /// locally: the base of a stack is routinely merged and deleted while work
    /// continues in its worktree. Otherwise the checked-out branch is the
    /// identity, as it always was (and `doctor conform` relocates the worktree
    /// accordingly).
    ///
    /// `local_branches` is consulted only in the mismatch case, since listing
    /// them costs a git call. It disambiguates the directory name, whose
    /// encoding is lossy (`/` becomes `--`); absent a matching local branch
    /// the name is decoded on the assumption that `--` came from `/`.
    pub fn project_branch_for_worktree(
        &self,
        repo: &str,
        worktree_path: &Path,
        checked_out: &str,
        local_branches: impl FnOnce() -> Vec<String>,
    ) -> Result<String, String> {
        let encoded_dir = worktree_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());
        let Some(encoded_dir) = encoded_dir else {
            return Ok(checked_out.to_string());
        };
        if git::encode_branch_for_path(checked_out) == encoded_dir || self.is_empty() {
            return Ok(checked_out.to_string());
        }
        let mut candidates: Vec<String> = local_branches()
            .into_iter()
            .filter(|b| git::encode_branch_for_path(b) == encoded_dir)
            .collect();
        if candidates.is_empty() {
            candidates.push(encoded_dir.replace("--", "/"));
        }
        let owners = candidates
            .iter()
            .map(String::as_str)
            .filter(|b| self.links(repo, b, checked_out))
            .collect();
        Ok(one_owner(repo, checked_out, owners)?
            .unwrap_or(checked_out)
            .to_string())
    }
}

fn one_owner<'a>(
    repo: &str,
    branch: &str,
    mut owners: Vec<&'a str>,
) -> Result<Option<&'a str>, String> {
    match owners.len() {
        0 => Ok(None),
        1 => Ok(Some(owners[0])),
        _ => {
            owners.sort();
            Err(format!(
                "Branch '{branch}' is a member branch of more than one project: {}",
                owners
                    .iter()
                    .map(|b| format!("{repo}:{b}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        }
    }
}

impl BranchPattern {
    /// The pattern, if it compiles once `{project}` is filled in.
    fn new(pattern: String) -> Option<Self> {
        let compiled = match regex_body(&pattern) {
            Some(body) => Regex::new(&anchored(&body.replace(PROJECT_PLACEHOLDER, "x")))
                .map(|_| Self::Regex(body.to_string()))
                .map_err(|e| e.to_string()),
            None => Pattern::new(&pattern.replace(PROJECT_PLACEHOLDER, "x"))
                .map(|_| Self::Glob(pattern.clone()))
                .map_err(|e| e.to_string()),
        };
        compiled
            .map_err(|e| error(&format!("Invalid branch pattern {pattern:?}: {e}")))
            .ok()
    }

    fn matches(&self, project_branch: &str, branch: &str) -> bool {
        match self {
            Self::Glob(glob) => Pattern::new(&glob.replace(PROJECT_PLACEHOLDER, project_branch))
                .is_ok_and(|p| p.matches(branch)),
            Self::Regex(regex) => {
                let filled = regex.replace(PROJECT_PLACEHOLDER, &regex::escape(project_branch));
                Regex::new(&anchored(&filled)).is_ok_and(|re| re.is_match(branch))
            }
        }
    }
}

/// The regex a `/`-delimited pattern holds. A branch name can neither start
/// nor end with `/`, so no glob is mistaken for one.
fn regex_body(pattern: &str) -> Option<&str> {
    pattern.strip_prefix('/')?.strip_suffix('/')
}

fn anchored(regex: &str) -> String {
    format!("^(?:{regex})$")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn global(patterns: &[&str]) -> ProjectBranches {
        branches(&[("*", patterns)])
    }

    fn branches(entries: &[(&str, &[&str])]) -> ProjectBranches {
        ProjectBranches::new(entries.iter().map(|(key, patterns)| {
            (
                key.to_string(),
                patterns.iter().map(|p| p.to_string()).collect(),
            )
        }))
    }

    #[test]
    fn the_global_entry_extends_every_project_branch() {
        let rules = global(&["{project}-*", "{project}/*"]);
        assert!(rules.links("repo", "feature-x", "feature-x-2"));
        assert!(rules.links("repo", "feature-x", "feature-x/fixup"));
        assert!(rules.links("other", "feature-x", "feature-x-2"));
        assert!(!rules.links("repo", "feature-x", "feature-x.rebased"));
        assert!(!rules.links("repo", "feature-x", "feature-xy"));
        assert!(!rules.links("repo", "feature-x", "other"));
        assert!(!rules.links("repo", "feature-x-2", "feature-x"));
    }

    #[test]
    fn patterns_are_globs_and_match_the_whole_branch() {
        let rules = global(&["fixup-*"]);
        assert!(rules.links("repo", "feature-x", "fixup-1"));
        assert!(!rules.links("repo", "feature-x", "my-fixup-1"));
    }

    #[test]
    fn slash_delimited_patterns_are_anchored_regexes() {
        let rules = global(&["/stack-[0-9]+/"]);
        assert!(rules.links("repo", "feature-x", "stack-12"));
        assert!(!rules.links("repo", "feature-x", "pre-stack-12"));
        // Interior slashes need no escaping, and explicit anchors are allowed.
        let rules = global(&["/^dan/{project}-[0-9]+$/"]);
        assert!(rules.links("repo", "feat/x", "dan/feat/x-2"));
        assert!(!rules.links("repo", "feat/x", "dan/feat/x-2/more"));
    }

    #[test]
    fn the_project_branch_is_escaped_in_a_regex() {
        let rules = global(&["/{project}-.*/"]);
        assert!(rules.links("repo", "v1.0", "v1.0-hotfix"));
        assert!(!rules.links("repo", "v1.0", "v1x0-hotfix"));
    }

    #[test]
    fn a_project_key_glob_selects_which_projects_an_entry_applies_to() {
        let rules = branches(&[
            ("wormhole:feature-x", &["mbt-fixups", "mbt/*"]),
            ("temporal:*", &["{project}-v*"]),
        ]);
        assert!(rules.links("wormhole", "feature-x", "mbt-fixups"));
        assert!(rules.links("wormhole", "feature-x", "mbt/2"));
        assert!(!rules.links("wormhole", "other", "mbt-fixups"));
        assert!(!rules.links("temporal", "feature-x", "mbt-fixups"));
        assert!(rules.links("temporal", "feature-x", "feature-x-v2"));
        assert!(!rules.links("wormhole", "feature-x", "feature-x-v2"));
    }

    #[test]
    fn a_project_key_glob_matches_across_slashes_in_a_branch() {
        let rules = global(&["{project}-*"]);
        assert!(rules.links("repo", "dan/feature", "dan/feature-2"));
    }

    #[test]
    fn a_branch_never_links_to_itself() {
        let rules = global(&["*"]);
        assert!(!rules.links("repo", "feature-x", "feature-x"));
        assert!(rules.links("repo", "feature-x", "anything"));
    }

    #[test]
    fn invalid_entries_are_dropped() {
        let rules = branches(&[("repo:[", &["x"]), ("*", &["/(/", "{project}-*"])]);
        assert!(rules.links("repo", "feature-x", "feature-x-2"));
        assert!(!rules.links("repo", "feature-x", "x"));
    }

    #[test]
    fn no_patterns_link_nothing() {
        let rules = ProjectBranches::default();
        assert!(rules.is_empty());
        assert!(!rules.links("repo", "feature-x", "feature-x-2"));
        assert!(branches(&[("*", &[])]).is_empty());
    }

    #[test]
    fn a_branch_naming_a_project_has_no_owner_to_look_for() {
        let rules = global(&["{project}-*"]);
        // `feature` claims `feature-x` under the rule, but `feature-x` is a
        // project, so it is not a member branch of anything.
        assert_eq!(
            rules.owning_project_branch(["feature", "feature-x"], "repo", "feature-x"),
            Ok(None)
        );
        assert_eq!(
            rules.owning_project_branch(["feature"], "repo", "unrelated"),
            Ok(None)
        );
        assert_eq!(
            rules.owning_project_branch(["feature"], "repo", "feature-x"),
            Ok(Some("feature"))
        );
    }

    #[test]
    fn a_branch_claimed_by_two_projects_is_an_error() {
        let rules = global(&["{project}-*"]);
        let projects = ["feature", "feature-x"];
        let err = rules
            .owning_project_branch(projects, "repo", "feature-x-2")
            .unwrap_err();
        assert!(
            err.contains("repo:feature, repo:feature-x"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn entries_are_a_union_not_a_precedence_order() {
        let rules = branches(&[
            ("*", &["{project}-*"]),
            ("repo:feature-x", &["feature-x-2"]),
        ]);
        // The specific entry does not shield `feature-x-2` from the global
        // one, so `feature` claims it too.
        assert!(rules
            .owning_project_branch(["feature", "feature-x"], "repo", "feature-x-2")
            .is_err());
        assert_eq!(
            rules.owning_project_branch(["feature-x"], "repo", "feature-x-2"),
            Ok(Some("feature-x"))
        );
    }

    fn worktree(encoded: &str) -> PathBuf {
        PathBuf::from("/worktrees/repo").join(encoded).join("repo")
    }

    #[test]
    fn worktree_keeps_the_directory_branch_when_the_checkout_is_a_member() {
        let rules = global(&["{project}-*"]);
        let local = || vec!["feature-x".to_string(), "feature-x-2".to_string()];
        assert_eq!(
            rules.project_branch_for_worktree("repo", &worktree("feature-x"), "feature-x-2", local),
            Ok("feature-x".to_string())
        );
    }

    #[test]
    fn worktree_follows_the_checkout_when_directory_and_branch_agree() {
        let rules = global(&["{project}-*"]);
        let never = || panic!("local branches should not be listed");
        assert_eq!(
            rules.project_branch_for_worktree("repo", &worktree("feature-x"), "feature-x", never),
            Ok("feature-x".to_string())
        );
        assert_eq!(
            rules.project_branch_for_worktree("repo", &worktree("feat--x"), "feat/x", never),
            Ok("feat/x".to_string())
        );
    }

    #[test]
    fn worktree_follows_the_checkout_when_the_checkout_is_not_a_member() {
        let rules = global(&["{project}/*"]);
        let local = || vec!["feature-x".to_string(), "feature-x-2".to_string()];
        assert_eq!(
            rules.project_branch_for_worktree("repo", &worktree("feature-x"), "feature-x-2", local),
            Ok("feature-x-2".to_string())
        );
        let never = || panic!("local branches should not be listed");
        assert_eq!(
            ProjectBranches::default().project_branch_for_worktree(
                "repo",
                &worktree("feature-x"),
                "feature-x-2",
                never
            ),
            Ok("feature-x-2".to_string())
        );
    }

    #[test]
    fn worktree_keeps_the_directory_branch_even_after_it_is_deleted() {
        let rules = global(&["{project}-*"]);
        let local = || vec!["feature-x-2".to_string()];
        assert_eq!(
            rules.project_branch_for_worktree("repo", &worktree("feature-x"), "feature-x-2", local),
            Ok("feature-x".to_string())
        );
        // A deleted branch's directory name is decoded as if `--` were `/`.
        let local = || vec!["feat/x-2".to_string()];
        assert_eq!(
            rules.project_branch_for_worktree("repo", &worktree("feat--x"), "feat/x-2", local),
            Ok("feat/x".to_string())
        );
        // ...but an existing local branch spelled with a literal `--` wins.
        let local = || vec!["feat--x".to_string(), "feat--x-2".to_string()];
        assert_eq!(
            rules.project_branch_for_worktree("repo", &worktree("feat--x"), "feat--x-2", local),
            Ok("feat--x".to_string())
        );
    }

    #[test]
    fn a_worktree_whose_checkout_two_branches_claim_is_an_error() {
        // The directory name `feat--x` could have come from either branch, and
        // this pattern makes the checkout a member branch of both.
        let rules = global(&["*-2"]);
        let local = || vec!["feat/x".to_string(), "feat--x".to_string()];
        let err = rules
            .project_branch_for_worktree("repo", &worktree("feat--x"), "feat/x-2", local)
            .unwrap_err();
        assert!(
            err.contains("repo:feat--x, repo:feat/x"),
            "unexpected error: {err}"
        );
    }
}
