use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub output_dir: String,
    pub synced: usize,
    pub skipped: usize,
}

/// A discovered transcript file with its owning project.
struct TranscriptFile {
    project_key: String,
    transcript_id: String,
    path: PathBuf,
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

/// Discover Cursor transcript files for a set of projects.
/// Each project is (project_key, repo_path).
pub fn discover_cursor_transcripts(projects: &[(String, PathBuf)]) -> Vec<TranscriptFile> {
    let cursor_projects_dir = cursor_projects_dir();
    if !cursor_projects_dir.is_dir() {
        return Vec::new();
    }

    // Build encoded prefixes sorted longest-first for most-specific matching
    let mut encoded: Vec<(String, &str)> = projects
        .iter()
        .map(|(key, path)| (encode_path_for_cursor(path), key.as_str()))
        .collect();
    encoded.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    let mut result = Vec::new();
    let entries = match std::fs::read_dir(&cursor_projects_dir) {
        Ok(e) => e,
        Err(_) => return result,
    };

    for entry in entries.flatten() {
        let dir_name = entry.file_name();
        let dir_name = dir_name.to_string_lossy();
        let transcripts_dir = entry.path().join("agent-transcripts");
        if !transcripts_dir.is_dir() {
            continue;
        }
        // Find best matching project (longest encoded prefix)
        if let Some((_, project_key)) = encoded.iter().find(|(prefix, _)| dir_name.starts_with(prefix.as_str())) {
            discover_transcripts_in(&transcripts_dir, project_key, &mut result);
        }
    }
    result
}

fn cursor_projects_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".cursor/projects")
}

fn encode_path_for_cursor(path: &Path) -> String {
    let s = path.to_string_lossy();
    let s = s.strip_prefix('/').unwrap_or(&s);
    let mut result = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_alphanumeric() {
            result.push(c);
            prev_dash = false;
        } else if !prev_dash {
            result.push('-');
            prev_dash = true;
        }
    }
    result.trim_end_matches('-').to_string()
}

fn discover_transcripts_in(dir: &Path, project_key: &str, out: &mut Vec<TranscriptFile>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().map_or(false, |e| e == "jsonl" || e == "txt") {
            if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                out.push(TranscriptFile {
                    project_key: project_key.to_string(),
                    transcript_id: id.to_string(),
                    path,
                });
            }
        } else if path.is_dir() {
            // Newer format: <uuid>/<uuid>.jsonl
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let jsonl = path.join(format!("{}.jsonl", name));
            if jsonl.is_file() {
                out.push(TranscriptFile {
                    project_key: project_key.to_string(),
                    transcript_id: name.to_string(),
                    path: jsonl,
                });
            }
        }
    }
}

/// Sync: materialize clean text files from Cursor transcripts.
/// Returns the output directory and counts.
pub fn sync(projects: &[(String, PathBuf)], project_filter: Option<&str>) -> SyncResult {
    let output_dir = conversations_dir();
    let transcripts = discover_cursor_transcripts(projects);

    let mut synced = 0;
    let mut skipped = 0;
    for t in &transcripts {
        if let Some(filter) = project_filter {
            if t.project_key != filter {
                continue;
            }
        }
        let date = file_date(&t.path);
        let short_id = &t.transcript_id[..8.min(t.transcript_id.len())];
        let out_dir = output_dir.join(&t.project_key);
        let out_file = out_dir.join(format!("{}-{}.txt", date, short_id));

        if out_file.exists() && !source_newer(&t.path, &out_file) {
            skipped += 1;
            continue;
        }

        let messages = if t.path.extension().map_or(false, |e| e == "jsonl") {
            match parse_cursor_jsonl(&t.path) {
                Ok(m) => m,
                Err(_) => continue,
            }
        } else {
            match parse_cursor_txt(&t.path) {
                Ok(m) => m,
                Err(_) => continue,
            }
        };

        if messages.is_empty() {
            continue;
        }

        let rendered = render_conversation(&t.project_key, &date, short_id, &messages);
        let _ = std::fs::create_dir_all(&out_dir);
        if std::fs::write(&out_file, &rendered).is_ok() {
            synced += 1;
        }
    }

    let dir = match project_filter {
        Some(p) => output_dir.join(p).to_string_lossy().to_string(),
        None => output_dir.to_string_lossy().to_string(),
    };
    SyncResult {
        output_dir: dir,
        synced,
        skipped,
    }
}

pub fn conversations_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".wormhole/conversations")
}

fn file_date(path: &Path) -> String {
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let secs = mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    // Simple date calculation from epoch days
    let (y, m, d) = epoch_days_to_ymd(days);
    format!("{:04}-{:02}-{:02}", y, m, d)
}

fn epoch_days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Civil calendar from epoch days (algorithm from Howard Hinnant)
    let z = days as i64 + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as u64, m, d)
}

fn source_newer(source: &Path, dest: &Path) -> bool {
    let source_mtime = std::fs::metadata(source)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let dest_mtime = std::fs::metadata(dest)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    source_mtime > dest_mtime
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
    fn test_encode_path_for_cursor() {
        assert_eq!(
            encode_path_for_cursor(Path::new("/Users/dan/src/wormhole")),
            "Users-dan-src-wormhole"
        );
        assert_eq!(
            encode_path_for_cursor(Path::new("/Users/dan/src/wormhole/.git/wormhole/workspaces/wormhole.code-workspace")),
            "Users-dan-src-wormhole-git-wormhole-workspaces-wormhole-code-workspace"
        );
        // : in task names becomes -
        assert_eq!(
            encode_path_for_cursor(Path::new("/Users/dan/src/wormhole/.git/wormhole/workspaces/wormhole:features.code-workspace")),
            "Users-dan-src-wormhole-git-wormhole-workspaces-wormhole-features-code-workspace"
        );
    }

    #[test]
    fn test_epoch_days_to_ymd() {
        // 2026-02-25 is day 20509 since epoch
        let (y, m, d) = epoch_days_to_ymd(20509);
        assert_eq!((y, m, d), (2026, 2, 25));
    }

    #[test]
    fn test_sync_materializes_files() {
        let tmp = tempfile::tempdir().unwrap();
        let cursor_dir = tmp.path().join(".cursor/projects/Users-test-myproject/agent-transcripts");
        std::fs::create_dir_all(&cursor_dir).unwrap();

        let jsonl = r#"{"role":"user","message":{"content":[{"type":"text","text":"<user_query>\nHello\n</user_query>"}]}}
{"role":"assistant","message":{"content":[{"type":"text","text":"World"}]}}"#;
        let id = "abcdef01-1234-5678-9abc-def012345678";
        let transcript_dir = cursor_dir.join(id);
        std::fs::create_dir_all(&transcript_dir).unwrap();
        std::fs::write(transcript_dir.join(format!("{}.jsonl", id)), jsonl).unwrap();

        let mut transcripts = Vec::new();
        discover_transcripts_in(&cursor_dir, "myproject", &mut transcripts);
        assert_eq!(transcripts.len(), 1);
        assert_eq!(transcripts[0].transcript_id, id);
        assert_eq!(transcripts[0].project_key, "myproject");
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
