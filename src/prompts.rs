use std::collections::HashMap;
use std::path::PathBuf;

use crate::project::Project;

fn prompts_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("prompts")
}

pub fn review_comments(project: &Project) -> Option<String> {
    let url = project.kv.get("review_pr_url")?;
    let (repo, pr) = parse_github_pr_url(url)?;
    let vars = HashMap::from([("repo", repo.as_str()), ("pr", pr.as_str())]);
    let template = std::fs::read_to_string(prompts_dir().join("review-comments.md"))
        .map_err(|e| eprintln!("Failed to read prompt template: {}", e))
        .ok()?;
    Some(render(&template, &vars))
}

fn render(template: &str, vars: &HashMap<&str, &str>) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{{{}}}}}", k), v);
    }
    out
}

fn parse_github_pr_url(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://github.com/")?;
    let parts: Vec<&str> = rest.splitn(4, '/').collect();
    if parts.len() >= 4 && parts[2] == "pull" {
        Some((format!("{}/{}", parts[0], parts[1]), parts[3].to_string()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render() {
        let vars = HashMap::from([("name", "world"), ("n", "42")]);
        assert_eq!(render("hello {{name}} #{{n}}", &vars), "hello world #42");
    }

    #[test]
    fn test_parse_github_pr_url() {
        let (repo, pr) =
            parse_github_pr_url("https://github.com/temporalio/temporal/pull/9333").unwrap();
        assert_eq!(repo, "temporalio/temporal");
        assert_eq!(pr, "9333");
    }

    #[test]
    fn test_parse_github_pr_url_invalid() {
        assert!(parse_github_pr_url("https://github.com/temporalio/temporal").is_none());
        assert!(parse_github_pr_url("https://example.com/pull/1").is_none());
    }
}
