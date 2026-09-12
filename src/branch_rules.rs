//! Rules linking additional branches to a task.
//!
//! A task is identified by a single branch, and its worktree lives at a path
//! derived from that branch. Work on one task often spans several branches
//! (a stack of PRs, a follow-up, a rebased copy). The `branch_rules` list in
//! `wormhole.toml` lets such a branch belong to the existing task instead of
//! becoming a task of its own. Each rule is a regex tested against a branch
//! name, in which `{task}` stands for the task's (regex-escaped) branch name:
//!
//! ```toml
//! branch_rules = ["^{task}[-/.]"]
//! ```
//!
//! links `feature-x-2` and `feature-x/fixup` to the task `repo:feature-x`.

use std::path::Path;

use regex::Regex;

use crate::git;

const TASK_PLACEHOLDER: &str = "{task}";

#[derive(Debug, Clone, Default)]
pub struct BranchRules {
    patterns: Vec<String>,
}

impl BranchRules {
    /// Keep the well-formed patterns, reporting and dropping the rest.
    pub fn new(patterns: impl IntoIterator<Item = String>) -> Self {
        let patterns = patterns
            .into_iter()
            .filter(
                |pattern| match Regex::new(&pattern.replace(TASK_PLACEHOLDER, "x")) {
                    Ok(_) => true,
                    Err(e) => {
                        crate::util::error(&format!("Invalid branch rule {pattern:?}: {e}"));
                        false
                    }
                },
            )
            .collect();
        Self { patterns }
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Whether `branch` belongs to the task whose branch is `task_branch`. A
    /// branch never links to itself: that is identity, not a rule.
    pub fn links(&self, task_branch: &str, branch: &str) -> bool {
        if task_branch == branch {
            return false;
        }
        let escaped = regex::escape(task_branch);
        self.patterns.iter().any(|pattern| {
            Regex::new(&pattern.replace(TASK_PLACEHOLDER, &escaped))
                .map(|re| re.is_match(branch))
                .unwrap_or(false)
        })
    }

    /// Of `task_branches`, the one `branch` belongs to, if any. The branch's
    /// own task wins outright. Otherwise, among the tasks a rule links it to,
    /// the longest prefix match wins (`feature-x-2` goes to `feature-x`, not
    /// `feature`); ties go to the alphabetically first so the outcome is
    /// stable.
    pub fn owning_task_branch<'a>(
        &self,
        task_branches: impl IntoIterator<Item = &'a str>,
        branch: &str,
    ) -> Option<&'a str> {
        let mut best: Option<&'a str> = None;
        for candidate in task_branches {
            if candidate == branch {
                return Some(candidate);
            }
            if !self.links(candidate, branch) {
                continue;
            }
            let better = match best {
                None => true,
                Some(b) => {
                    (candidate.len(), std::cmp::Reverse(candidate))
                        > (b.len(), std::cmp::Reverse(b))
                }
            };
            if better {
                best = Some(candidate);
            }
        }
        best
    }

    /// The branch identifying the task whose worktree is at `worktree_path`
    /// with `checked_out` checked out.
    ///
    /// Worktrees live at `<worktree_dir>/<repo>/<encoded-branch>/<repo>`, so
    /// normally the directory encodes the checked-out branch and that branch
    /// is the identity. When it doesn't, the worktree was created for another
    /// branch. That branch remains the task's identity if a rule links the
    /// checked-out branch to it, whether or not it still exists locally: the
    /// base of a stack is routinely merged and deleted while work continues
    /// in its worktree. Otherwise the checked-out branch is the identity, as
    /// it always was (and `doctor conform` relocates the worktree
    /// accordingly).
    ///
    /// `local_branches` is consulted only in the mismatch case, since listing
    /// them costs a git call. It disambiguates the directory name, whose
    /// encoding is lossy (`/` becomes `--`); absent a matching local branch
    /// the name is decoded on the assumption that `--` came from `/`.
    pub fn task_branch_for_worktree(
        &self,
        worktree_path: &Path,
        checked_out: &str,
        local_branches: impl FnOnce() -> Vec<String>,
    ) -> String {
        let encoded_dir = worktree_path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());
        let Some(encoded_dir) = encoded_dir else {
            return checked_out.to_string();
        };
        if git::encode_branch_for_path(checked_out) == encoded_dir || self.is_empty() {
            return checked_out.to_string();
        }
        let mut existing: Vec<String> = local_branches()
            .into_iter()
            .filter(|b| git::encode_branch_for_path(b) == encoded_dir)
            .collect();
        if existing.is_empty() {
            existing.push(encoded_dir.replace("--", "/"));
        }
        let mut candidates = existing.into_iter().filter(|b| self.links(b, checked_out));
        match (candidates.next(), candidates.next()) {
            (Some(task_branch), None) => task_branch,
            _ => checked_out.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn rules(patterns: &[&str]) -> BranchRules {
        BranchRules::new(patterns.iter().map(|s| s.to_string()))
    }

    #[test]
    fn prefix_rule_links_extensions_of_the_task_branch() {
        let rules = rules(&["^{task}[-/.]"]);
        assert!(rules.links("feature-x", "feature-x-2"));
        assert!(rules.links("feature-x", "feature-x/fixup"));
        assert!(rules.links("feature-x", "feature-x.rebased"));
        assert!(!rules.links("feature-x", "feature-xy"));
        assert!(!rules.links("feature-x", "other"));
        assert!(!rules.links("feature-x-2", "feature-x"));
    }

    #[test]
    fn task_branch_is_escaped_in_the_pattern() {
        let rules = rules(&["^{task}-"]);
        assert!(rules.links("v1.0", "v1.0-hotfix"));
        assert!(!rules.links("v1.0", "v1x0-hotfix"));
        assert!(rules.links("feat/x", "feat/x-2"));
    }

    #[test]
    fn a_branch_never_links_to_itself() {
        let rules = rules(&[".*"]);
        assert!(!rules.links("feature-x", "feature-x"));
        assert!(rules.links("feature-x", "anything"));
    }

    #[test]
    fn invalid_patterns_are_dropped() {
        let rules = rules(&["^{task}(", "^{task}-"]);
        assert_eq!(rules.patterns, vec!["^{task}-".to_string()]);
    }

    #[test]
    fn no_rules_link_nothing() {
        let rules = BranchRules::default();
        assert!(rules.is_empty());
        assert!(!rules.links("feature-x", "feature-x-2"));
    }

    #[test]
    fn owning_task_prefers_identity_then_longest_prefix_then_alphabetical() {
        let rules = rules(&["^{task}-"]);
        let tasks = ["feature", "feature-x", "feature-y"];
        assert_eq!(
            rules.owning_task_branch(tasks, "feature-x"),
            Some("feature-x")
        );
        assert_eq!(
            rules.owning_task_branch(tasks, "feature-x-2"),
            Some("feature-x")
        );
        assert_eq!(
            rules.owning_task_branch(tasks, "feature-z-2"),
            Some("feature")
        );
        assert_eq!(rules.owning_task_branch(tasks, "unrelated"), None);

        let same_length = ["task-b", "task-a"];
        assert_eq!(
            rules.owning_task_branch(same_length, "task-a-1"),
            Some("task-a")
        );
        let ambiguous = BranchRules::new([".*".to_string()]);
        assert_eq!(
            ambiguous.owning_task_branch(same_length, "x"),
            Some("task-a")
        );
    }

    fn worktree(encoded: &str) -> PathBuf {
        PathBuf::from("/worktrees/repo").join(encoded).join("repo")
    }

    #[test]
    fn worktree_keeps_the_directory_branch_when_a_rule_links_the_checkout() {
        let rules = rules(&["^{task}-"]);
        let local = || vec!["feature-x".to_string(), "feature-x-2".to_string()];
        assert_eq!(
            rules.task_branch_for_worktree(&worktree("feature-x"), "feature-x-2", local),
            "feature-x"
        );
    }

    #[test]
    fn worktree_follows_the_checkout_when_directory_and_branch_agree() {
        let rules = rules(&["^{task}-"]);
        let never = || panic!("local branches should not be listed");
        assert_eq!(
            rules.task_branch_for_worktree(&worktree("feature-x"), "feature-x", never),
            "feature-x"
        );
        assert_eq!(
            rules.task_branch_for_worktree(&worktree("feat--x"), "feat/x", never),
            "feat/x"
        );
    }

    #[test]
    fn worktree_follows_the_checkout_without_a_linking_rule() {
        let rules = rules(&["^{task}/"]);
        let local = || vec!["feature-x".to_string(), "feature-x-2".to_string()];
        assert_eq!(
            rules.task_branch_for_worktree(&worktree("feature-x"), "feature-x-2", local),
            "feature-x-2"
        );
        let never = || panic!("local branches should not be listed");
        assert_eq!(
            BranchRules::default().task_branch_for_worktree(
                &worktree("feature-x"),
                "feature-x-2",
                never
            ),
            "feature-x-2"
        );
    }

    #[test]
    fn worktree_keeps_the_directory_branch_even_after_it_is_deleted() {
        let rules = rules(&["^{task}-"]);
        let local = || vec!["feature-x-2".to_string()];
        assert_eq!(
            rules.task_branch_for_worktree(&worktree("feature-x"), "feature-x-2", local),
            "feature-x"
        );
        // A deleted branch's directory name is decoded as if `--` were `/`.
        let local = || vec!["feat/x-2".to_string()];
        assert_eq!(
            rules.task_branch_for_worktree(&worktree("feat--x"), "feat/x-2", local),
            "feat/x"
        );
        // ...but an existing local branch spelled with a literal `--` wins.
        let local = || vec!["feat--x".to_string(), "feat--x-2".to_string()];
        assert_eq!(
            rules.task_branch_for_worktree(&worktree("feat--x"), "feat--x-2", local),
            "feat--x"
        );
    }
}
