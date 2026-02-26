use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug)]
pub struct Conversation {
    pub id: String,
    pub messages: Vec<Message>,
    pub source_path: PathBuf,
}

pub fn parse_cursor_jsonl(path: &Path) -> Result<Vec<Message>, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    parse_cursor_jsonl_str(&content)
}

pub fn parse_cursor_txt(path: &Path) -> Result<Vec<Message>, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    Ok(parse_cursor_txt_str(&content))
}

fn parse_cursor_jsonl_str(content: &str) -> Result<Vec<Message>, String> {
    let mut messages = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let record: CursorJsonlRecord =
            serde_json::from_str(line).map_err(|e| format!("parse JSONL: {}", e))?;
        let role = match record.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            _ => continue,
        };
        let text = extract_text(&record.message.content);
        let text = if role == Role::User {
            strip_user_query_tags(&text)
        } else {
            text
        };
        let text = text.trim().to_string();
        if !text.is_empty() {
            messages.push(Message { role, text });
        }
    }
    Ok(messages)
}

fn parse_cursor_txt_str(content: &str) -> Vec<Message> {
    let mut messages = Vec::new();
    let mut current_role: Option<Role> = None;
    let mut current_text = String::new();
    let mut in_tool_block = false;

    for line in content.lines() {
        if line == "user:" {
            flush_message(&mut messages, &mut current_role, &mut current_text);
            current_role = Some(Role::User);
            in_tool_block = false;
        } else if line == "A:" {
            flush_message(&mut messages, &mut current_role, &mut current_text);
            current_role = Some(Role::Assistant);
            in_tool_block = false;
        } else if current_role.is_some() {
            if line.starts_with("[Tool call]") || line.starts_with("[Tool result]") {
                in_tool_block = true;
                continue;
            }
            if in_tool_block {
                if line.is_empty() || !line.starts_with("  ") {
                    in_tool_block = false;
                }
                if line.starts_with("  ") {
                    continue;
                }
            }
            current_text.push_str(line);
            current_text.push('\n');
        }
    }
    flush_message(&mut messages, &mut current_role, &mut current_text);
    messages
}

fn flush_message(messages: &mut Vec<Message>, role: &mut Option<Role>, text: &mut String) {
    if let Some(r) = role.take() {
        let cleaned = if r == Role::User {
            strip_user_query_tags(text)
        } else {
            strip_thinking(text)
        };
        let cleaned = cleaned.trim().to_string();
        if !cleaned.is_empty() {
            messages.push(Message { role: r, text: cleaned });
        }
        text.clear();
    }
}

fn strip_user_query_tags(text: &str) -> String {
    let text = text.trim();
    let text = text
        .strip_prefix("<user_query>")
        .unwrap_or(text)
        .strip_suffix("</user_query>")
        .unwrap_or(text);
    text.trim().to_string()
}

fn strip_thinking(text: &str) -> String {
    let mut result = String::new();
    for line in text.lines() {
        if line.starts_with("[Thinking]") {
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }
    result
}

fn extract_text(content: &[ContentBlock]) -> String {
    let mut parts = Vec::new();
    for block in content {
        if block.r#type == "text" {
            if let Some(ref text) = block.text {
                parts.push(text.as_str());
            }
        }
    }
    parts.join("\n")
}

pub fn render_conversation(project: &str, date: &str, short_id: &str, messages: &[Message]) -> String {
    let mut out = format!("# {} | {} | {}\n", project, date, short_id);
    for msg in messages {
        let heading = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
        };
        out.push_str(&format!("\n## {}\n\n{}\n", heading, msg.text));
    }
    out
}

#[derive(Deserialize)]
struct CursorJsonlRecord {
    role: String,
    message: CursorMessage,
}

#[derive(Deserialize)]
struct CursorMessage {
    content: Vec<ContentBlock>,
}

#[derive(Deserialize)]
struct ContentBlock {
    r#type: String,
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_jsonl_basic() {
        let input = r#"{"role":"user","message":{"content":[{"type":"text","text":"<user_query>\nHello world\n</user_query>"}]}}
{"role":"assistant","message":{"content":[{"type":"text","text":"Hi there!"}]}}"#;
        let messages = parse_cursor_jsonl_str(input).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[0].text, "Hello world");
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[1].text, "Hi there!");
    }

    #[test]
    fn test_parse_jsonl_strips_system_noise() {
        let input = r#"{"role":"user","message":{"content":[{"type":"text","text":"<user_query>\nWhat is rust?\n</user_query>"}]}}
{"role":"assistant","message":{"content":[{"type":"text","text":"Rust is a systems programming language."}]}}
{"role":"user","message":{"content":[{"type":"text","text":"<user_query>\nThanks\n</user_query>"}]}}"#;
        let messages = parse_cursor_jsonl_str(input).unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text, "What is rust?");
        assert_eq!(messages[2].text, "Thanks");
    }

    #[test]
    fn test_parse_jsonl_multiline_user_query() {
        let input = r#"{"role":"user","message":{"content":[{"type":"text","text":"<user_query>\nLine one\nLine two\nLine three\n</user_query>"}]}}"#;
        let messages = parse_cursor_jsonl_str(input).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "Line one\nLine two\nLine three");
    }

    #[test]
    fn test_parse_jsonl_skips_empty_content() {
        let input = r#"{"role":"user","message":{"content":[{"type":"text","text":"<user_query>\n\n</user_query>"}]}}"#;
        let messages = parse_cursor_jsonl_str(input).unwrap();
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_parse_jsonl_non_text_blocks_ignored() {
        let input = r#"{"role":"assistant","message":{"content":[{"type":"tool_use","text":null},{"type":"text","text":"The answer is 42."}]}}"#;
        let messages = parse_cursor_jsonl_str(input).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "The answer is 42.");
    }

    #[test]
    fn test_parse_txt_basic() {
        let input = "user:\n<user_query>\nHello\n</user_query>\n\nA:\nHi there!\n";
        let messages = parse_cursor_txt_str(input);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[0].text, "Hello");
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[1].text, "Hi there!");
    }

    #[test]
    fn test_parse_txt_strips_tool_calls() {
        let input = "user:\n<user_query>\nStudy this\n</user_query>\n\nA:\n[Thinking] Let me look at this.\nHere is what I found.\n[Tool call] Read\n  path: /some/file\n\n[Tool result] Read\n\nA:\nThe file contains foo.\n";
        let messages = parse_cursor_txt_str(input);
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text, "Study this");
        assert_eq!(messages[1].text, "Here is what I found.");
        assert!(!messages[1].text.contains("[Thinking]"));
        assert!(!messages[1].text.contains("[Tool call]"));
        assert_eq!(messages[2].text, "The file contains foo.");
    }

    #[test]
    fn test_render_conversation() {
        let messages = vec![
            Message { role: Role::User, text: "Hello".to_string() },
            Message { role: Role::Assistant, text: "Hi there!".to_string() },
        ];
        let rendered = render_conversation("wormhole", "2026-02-25", "8afac8bb", &messages);
        assert!(rendered.starts_with("# wormhole | 2026-02-25 | 8afac8bb\n"));
        assert!(rendered.contains("## User\n\nHello\n"));
        assert!(rendered.contains("## Assistant\n\nHi there!\n"));
    }
}
