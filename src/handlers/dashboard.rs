use hyper::{Body, Response};

use crate::projects;

pub fn dashboard() -> Response<Body> {
    use crate::project::Project;

    let window_names = crate::tmux::window_names();
    let (mut tasks, current_key): (Vec<Project>, Option<String>) = {
        let projects = projects::lock();
        let tasks = projects
            .all()
            .into_iter()
            .filter(|p| {
                p.is_task() && (p.kv.contains_key("jira_key") || p.is_active(&window_names))
            })
            .cloned()
            .collect();
        let current = projects.current().map(|p| p.store_key().to_string());
        (tasks, current)
    };
    tasks.sort_by_key(|t| status_sort_order(t.cached.jira.as_ref().map(|j| j.status.as_str())));
    let jira_instance = std::env::var("JIRA_INSTANCE").ok();

    let cards_html: String = tasks
        .iter()
        .map(|task| {
            let is_current = current_key.as_ref() == Some(&task.store_key().to_string());
            render_task_card(task, jira_instance.as_deref(), is_current)
        })
        .collect();

    let template = include_str!("dashboard.html");
    let html = template.replace("{{CARDS}}", &cards_html);

    Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(Body::from(html))
        .unwrap()
}

fn render_task_card(
    task: &crate::project::Project,
    jira_instance: Option<&str>,
    is_current: bool,
) -> String {
    let task_key = task.store_key().to_string();
    let branch_html = task
        .branch
        .as_ref()
        .map(|b| {
            format!(
                r#" <span class="card-branch" data-key="{}">{}</span>"#,
                html_escape(&task_key),
                html_escape(b.as_str())
            )
        })
        .unwrap_or_default();
    let repo_branch = format!(
        r#"<span class="card-repo">{}</span>{}"#,
        html_escape(task.repo_name.as_str()),
        branch_html
    );

    let summary = task
        .cached
        .jira
        .as_ref()
        .map(|j| html_escape(&j.summary))
        .unwrap_or_default();

    let status_html = task
        .cached
        .jira
        .as_ref()
        .map(|j| {
            format!(
                r#"<span class="card-status">{} {}</span>"#,
                j.status_emoji(),
                html_escape(&j.status)
            )
        })
        .unwrap_or_default();

    let pr_html = if let Some(ref pr) = task.cached.pr {
        let comments = pr
            .comments_display()
            .map(|c| format!(" [{}]", html_escape(&c)))
            .unwrap_or_default();
        format!(
            r#"<span class="meta-item"><a href="{}" target="_blank">{}</a>{}</span>"#,
            pr.url,
            html_escape(&pr.display()),
            comments
        )
    } else {
        String::new()
    };

    let jira_html = task
        .cached
        .jira
        .as_ref()
        .and_then(|j| {
            jira_instance.map(|i| {
                format!(
                    r#"<span class="meta-item"><a href="https://{}.atlassian.net/browse/{}" target="_blank">{}</a></span>"#,
                    i,
                    html_escape(&j.key),
                    html_escape(&j.key)
                )
            })
        })
        .unwrap_or_default();

    let sprint_html = task
        .cached
        .jira
        .as_ref()
        .and_then(|j| {
            j.sprint.as_ref().map(|name| {
                match (jira_instance, j.sprint_id) {
                    (Some(inst), Some(sprint_id)) => {
                        let url = format!(
                            "https://{}.atlassian.net/secure/GHGoToBoard.jspa?sprintId={}",
                            inst, sprint_id
                        );
                        format!(
                            r#"<span class="meta-item card-sprint"><a href="{}" target="_blank">{}</a></span>"#,
                            url,
                            html_escape(name)
                        )
                    }
                    _ => format!(
                        r#"<span class="meta-item card-sprint">{}</span>"#,
                        html_escape(name)
                    ),
                }
            })
        })
        .unwrap_or_default();

    let path = task.working_tree();

    let card_md_path = path.join(".task/card.md");
    let card_content_html = if card_md_path.exists() {
        std::fs::read_to_string(&card_md_path)
            .ok()
            .filter(|s| !s.is_empty())
            .map(|md| {
                format!(
                    r#"<div class="card-content">{}</div>"#,
                    render_markdown(&md)
                )
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

    let iframe_html = render_iframe(task);

    let status_attr = task
        .cached
        .jira
        .as_ref()
        .map(|j| status_data_attr(&j.status))
        .unwrap_or_default();

    let assignee_html = task
        .cached
        .jira
        .as_ref()
        .map(|j| render_assignee_warning(j.assignee.as_deref(), j.assignee_email.as_deref()))
        .unwrap_or_default();

    let task_id = task.store_key().to_string();
    let current_class = if is_current { " current" } else { "" };

    format!(
        r#"<div class="card{}" data-task="{}"{}>
<div class="card-header">{}<span class="card-summary">{}</span>{}</div>
<div class="card-meta">{}{}{}{}</div>
{}{}
</div>"#,
        current_class,
        html_escape(&task_id),
        status_attr,
        repo_branch,
        summary,
        status_html,
        jira_html,
        sprint_html,
        pr_html,
        assignee_html,
        card_content_html,
        iframe_html
    )
}

fn render_iframe(task: &crate::project::Project) -> String {
    let path = task.working_tree();
    let terminal_icon = include_str!("icons/terminal.b64");
    let cursor_icon = include_str!("icons/cursor.b64");
    let claude_btn = claude_md_button(&path);

    // Terminal and Cursor buttons are always available — they don't depend on serve-web.
    let mut actions = format!(
        concat!(
            r#"<button class="btn btn-icon btn-terminal" title="Terminal">"#,
            r#"<img src="data:image/png;base64,{}" alt="Terminal"></button>"#,
            r#"<button class="btn btn-icon btn-cursor" title="Cursor">"#,
            r#"<img src="data:image/png;base64,{}" alt="Cursor"></button>"#,
        ),
        terminal_icon.trim(),
        cursor_icon.trim(),
    );

    // VSCode button + iframe only appear when serve-web starts successfully.
    let mut iframe_html = String::new();
    if let Ok(port) = crate::serve_web::manager().get_or_start(task.repo_name.as_str(), &path) {
        let folder_encoded = super::url_encode(&path.to_string_lossy());
        let vscode_icon = include_str!("icons/vscode.b64");
        actions.push_str(&format!(
            concat!(
                r#"<button class="btn btn-icon btn-vscode" title="VSCode">"#,
                r#"<img src="data:image/png;base64,{}" alt="VSCode"></button>"#,
            ),
            vscode_icon.trim(),
        ));
        iframe_html = format!(
            concat!(
                "\n",
                r#"<div class="iframe-container">"#,
                r#"<iframe data-src="http://localhost:{}/?folder={}"></iframe></div>"#,
            ),
            port, folder_encoded,
        );
    }

    actions.push_str(&claude_btn);
    if !iframe_html.is_empty() {
        actions.push_str(r#"<button class="btn btn-maximize">Maximize</button>"#);
    }

    format!(
        r#"<div class="card-actions">{}</div>{}"#,
        actions, iframe_html
    )
}

fn claude_md_button(path: &std::path::Path) -> String {
    let claude_md = path.join("CLAUDE.md");
    if claude_md.exists() {
        let file_url = format!("/file/{}?land-in=editor", claude_md.to_string_lossy());
        format!(
            r#"<button class="btn btn-icon btn-claude" title="CLAUDE.md" data-url="{}">&#x1F4D6;</button>"#,
            html_escape(&file_url)
        )
    } else {
        String::new()
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn status_data_attr(status: &str) -> String {
    match status.to_lowercase().as_str() {
        "done" | "closed" | "resolved" => r#" data-status="done""#.to_string(),
        _ => String::new(),
    }
}

fn render_assignee_warning(assignee: Option<&str>, assignee_email: Option<&str>) -> String {
    let my_email = std::env::var("JIRA_EMAIL").ok();
    let is_mine = assignee_email.is_some() && assignee_email == my_email.as_deref();
    if is_mine {
        return String::new();
    }
    let display = assignee.unwrap_or("Unassigned");
    format!(
        r#"<span class="meta-item assignee-warning">⚠️ {}</span>"#,
        html_escape(display)
    )
}

fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let parser = Parser::new_ext(md, Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES);
    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);
    // Rewrite absolute image paths so the browser fetches them via /asset/.
    rewrite_img_src(&mut html_output);
    html_output
}

/// Rewrite `<img src="/absolute/path">` to `<img src="/asset/absolute/path">`
/// so local images in card.md are served through the asset endpoint.
fn rewrite_img_src(html: &mut String) {
    let needle = "src=\"/";
    let mut result = String::with_capacity(html.len());
    let mut rest = html.as_str();
    while let Some(idx) = rest.find(needle) {
        let after = &rest[idx + needle.len()..]; // text after src="/
                                                 // Don't rewrite URLs that are already routed (e.g. src="/asset/...")
        if after.starts_with("asset/") {
            result.push_str(&rest[..idx + needle.len()]);
            rest = after;
            continue;
        }
        result.push_str(&rest[..idx]);
        result.push_str("src=\"/asset/");
        rest = after;
    }
    result.push_str(rest);
    *html = result;
}

fn status_sort_order(status: Option<&str>) -> u8 {
    match status.map(|s| s.to_lowercase()).as_deref() {
        Some("done") | Some("closed") | Some("resolved") => 0,
        Some("in review") => 1,
        Some("in progress") => 2,
        Some("to do") => 3,
        _ => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rewrite_img_src_absolute_path() {
        let mut html = r#"<img src="/Users/me/pic.png" alt="pic">"#.to_string();
        rewrite_img_src(&mut html);
        assert_eq!(html, r#"<img src="/asset/Users/me/pic.png" alt="pic">"#);
    }

    #[test]
    fn test_rewrite_img_src_already_asset() {
        let mut html = r#"<img src="/asset/Users/me/pic.png">"#.to_string();
        rewrite_img_src(&mut html);
        assert_eq!(html, r#"<img src="/asset/Users/me/pic.png">"#);
    }

    #[test]
    fn test_rewrite_img_src_no_images() {
        let mut html = "<p>hello world</p>".to_string();
        rewrite_img_src(&mut html);
        assert_eq!(html, "<p>hello world</p>");
    }

    #[test]
    fn test_rewrite_img_src_multiple_images() {
        let mut html = r#"<img src="/a/b.png"><img src="/c/d.jpg">"#.to_string();
        rewrite_img_src(&mut html);
        assert_eq!(
            html,
            r#"<img src="/asset/a/b.png"><img src="/asset/c/d.jpg">"#
        );
    }

    #[test]
    fn test_rewrite_img_src_relative_untouched() {
        let mut html = r#"<img src="relative/pic.png">"#.to_string();
        rewrite_img_src(&mut html);
        assert_eq!(html, r#"<img src="relative/pic.png">"#);
    }
}
