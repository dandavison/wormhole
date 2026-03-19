use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use uuid::Uuid;

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

#[derive(Debug, Clone, Copy, PartialEq)]
enum TranscriptSource {
    Cursor,
    ClaudeCode,
}

/// A discovered transcript file with its owning project.
struct TranscriptFile {
    project_key: String,
    transcript_id: String,
    path: PathBuf,
    source: TranscriptSource,
}

pub fn parse_cursor_jsonl(path: &Path) -> Result<Vec<Message>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    parse_cursor_jsonl_str(&content)
}

pub fn parse_cursor_txt(path: &Path) -> Result<Vec<Message>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
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
            messages.push(Message {
                role: r,
                text: cleaned,
            });
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

pub fn render_conversation(
    project: &str,
    date: &str,
    session_uuid: &str,
    messages: &[Message],
) -> String {
    let mut out = format!("# {} | {} | {}\n", project, date, session_uuid);
    for msg in messages {
        let heading = match msg.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
        };
        out.push_str(&format!("\n## {}\n\n{}\n", heading, msg.text));
    }
    out
}

/// Parse the first line of a synced conversation file to extract (project_key, session_uuid).
pub fn parse_conversation_header(path: &Path) -> Option<(String, String)> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    let mut first_line = String::new();
    std::io::BufRead::read_line(&mut reader, &mut first_line).ok()?;
    parse_header_line(&first_line)
}

fn parse_header_line(line: &str) -> Option<(String, String)> {
    let line = line.trim().strip_prefix("# ")?;
    let parts: Vec<&str> = line.split(" | ").collect();
    if parts.len() >= 3 {
        Some((parts[0].to_string(), parts[2].to_string()))
    } else {
        None
    }
}

/// Discover Cursor transcript files for a set of projects.
/// Each project is (project_key, repo_path).
fn discover_cursor_transcripts(projects: &[(String, PathBuf)]) -> Vec<TranscriptFile> {
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
        if let Some((_, project_key)) = encoded
            .iter()
            .find(|(prefix, _)| dir_name.starts_with(prefix.as_str()))
        {
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
        if path
            .extension()
            .map_or(false, |e| e == "jsonl" || e == "txt")
        {
            if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                out.push(TranscriptFile {
                    project_key: project_key.to_string(),
                    transcript_id: id.to_string(),
                    path,
                    source: TranscriptSource::Cursor,
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
                    source: TranscriptSource::Cursor,
                });
            }
        }
    }
}

/// Discover Claude Code transcript files for a set of projects.
fn discover_claude_code_transcripts(projects: &[(String, PathBuf)]) -> Vec<TranscriptFile> {
    let cc_projects_dir = claude_code_projects_dir();
    if !cc_projects_dir.is_dir() {
        return Vec::new();
    }

    // Canonicalize project paths for matching, sorted longest-first
    let mut canonical: Vec<(String, String)> = projects
        .iter()
        .filter_map(|(key, path)| {
            std::fs::canonicalize(path)
                .ok()
                .map(|p| (p.to_string_lossy().to_string(), key.clone()))
        })
        .collect();
    canonical.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
    canonical.dedup_by(|a, b| a.0 == b.0);

    let mut result = Vec::new();
    let entries = match std::fs::read_dir(&cc_projects_dir) {
        Ok(e) => e,
        Err(_) => return result,
    };

    for entry in entries.flatten() {
        result.extend(discover_cc_from_jsonl_files(&entry.path(), &canonical));
    }
    result
}

/// Discover sessions by scanning JSONL files in a CC project directory.
fn discover_cc_from_jsonl_files(
    dir: &Path,
    canonical: &[(String, String)],
) -> Vec<TranscriptFile> {
    let mut result = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return result,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let session_id = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s.to_string(),
            None => continue,
        };
        if let Some(meta) = read_cc_jsonl_metadata(&path) {
            if meta.is_sidechain {
                continue;
            }
            if meta.message_count < 2 {
                continue;
            }
            let project_path = match meta.cwd {
                Some(p) => p,
                None => continue,
            };
            if let Some(key) = match_project_path(&project_path, canonical) {
                result.push(TranscriptFile {
                    project_key: key,
                    transcript_id: session_id,
                    path,
                    source: TranscriptSource::ClaudeCode,
                });
            }
        }
    }
    result
}

struct CcJsonlMetadata {
    cwd: Option<String>,
    is_sidechain: bool,
    message_count: u64,
}

/// Read metadata from the first lines of a CC JSONL file without parsing the whole file.
fn read_cc_jsonl_metadata(path: &Path) -> Option<CcJsonlMetadata> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    let mut cwd = None;
    let mut is_sidechain = false;
    let mut message_count: u64 = 0;
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let record: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let msg_type = record.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match msg_type {
            "user" | "assistant" => {
                message_count += 1;
                if cwd.is_none() {
                    cwd = record
                        .get("cwd")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                }
                if record.get("isSidechain").and_then(|v| v.as_bool()) == Some(true) {
                    is_sidechain = true;
                }
                // Once we have cwd and enough messages, stop reading
                if cwd.is_some() && message_count >= 2 {
                    break;
                }
            }
            _ => {}
        }
    }
    Some(CcJsonlMetadata {
        cwd,
        is_sidechain,
        message_count,
    })
}

/// Match a project path against known wormhole projects (longest prefix wins).
fn match_project_path(project_path: &str, canonical: &[(String, String)]) -> Option<String> {
    let canon_pp = std::fs::canonicalize(project_path)
        .unwrap_or_else(|_| PathBuf::from(project_path))
        .to_string_lossy()
        .to_string();
    canonical
        .iter()
        .find(|(prefix, _)| canon_pp.starts_with(prefix.as_str()))
        .map(|(_, key)| key.clone())
}

fn claude_code_projects_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".claude/projects")
}

/// Sync: materialize clean text files from Cursor and Claude Code transcripts.
/// Returns the output directory and counts.
pub fn sync(
    projects: &[(String, PathBuf)],
    project_filter: Option<&[&str]>,
    since: Option<SystemTime>,
) -> SyncResult {
    let output_dir = conversations_dir();
    let mut transcripts = discover_cursor_transcripts(projects);
    transcripts.extend(discover_claude_code_transcripts(projects));

    let mut synced = 0;
    let mut skipped = 0;
    for t in &transcripts {
        if let Some(filters) = project_filter {
            if !filters.iter().any(|f| *f == t.project_key) {
                continue;
            }
        }
        if let Some(cutoff) = since {
            let mtime = std::fs::metadata(&t.path)
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            if mtime < cutoff {
                continue;
            }
        }

        let messages = match t.source {
            TranscriptSource::Cursor => {
                if t.path.extension().is_some_and(|e| e == "jsonl") {
                    parse_cursor_jsonl(&t.path).ok()
                } else {
                    parse_cursor_txt(&t.path).ok()
                }
            }
            TranscriptSource::ClaudeCode => parse_claude_code_jsonl(&t.path).ok(),
        };
        let messages = match messages {
            Some(m) if !m.is_empty() => m,
            _ => continue,
        };

        // Determine the CC session UUID (eagerly convert Cursor transcripts)
        let session_uuid = match t.source {
            TranscriptSource::ClaudeCode => t.transcript_id.clone(),
            TranscriptSource::Cursor => {
                // Find the project dir for conversion
                let project_dir = projects
                    .iter()
                    .find(|(k, _)| *k == t.project_key)
                    .map(|(_, p)| p.clone());
                match project_dir {
                    Some(dir) => {
                        match convert_to_claude_code(&t.transcript_id, &messages, &dir, None) {
                            Ok((uuid, _)) => uuid.to_string(),
                            Err(_) => continue,
                        }
                    }
                    None => continue,
                }
            }
        };

        let date = file_date(&t.path);
        let short_id = &session_uuid[..8.min(session_uuid.len())];
        let out_dir = output_dir.join(&t.project_key);
        let out_file = out_dir.join(format!("{}-{}.md", date, short_id));

        if out_file.exists() && !source_newer(&t.path, &out_file) {
            skipped += 1;
            continue;
        }

        let rendered = render_conversation(&t.project_key, &date, &session_uuid, &messages);
        let _ = std::fs::create_dir_all(&out_dir);
        if std::fs::write(&out_file, &rendered).is_ok() {
            synced += 1;
        }
    }

    let dir = match project_filter {
        Some(filters) if filters.len() == 1 => {
            output_dir.join(filters[0]).to_string_lossy().to_string()
        }
        _ => output_dir.to_string_lossy().to_string(),
    };
    SyncResult {
        output_dir: dir,
        synced,
        skipped,
    }
}

/// Parse a duration string like "2w", "3d", "1m" into a SystemTime cutoff.
pub fn parse_since(s: &str) -> Option<SystemTime> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let (num_str, unit) = s.split_at(s.len() - 1);
    let n: u64 = num_str.parse().ok()?;
    let days = match unit {
        "d" => n,
        "w" => n * 7,
        "m" => n * 30,
        _ => return None,
    };
    let duration = std::time::Duration::from_secs(days * 86400);
    SystemTime::now().checked_sub(duration)
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

// Namespace UUID for deterministic session IDs from Cursor transcript IDs
const WORMHOLE_UUID_NAMESPACE: Uuid = Uuid::from_bytes([
    0x77, 0x6f, 0x72, 0x6d, 0x68, 0x6f, 0x6c, 0x65, 0x63, 0x6f, 0x6e, 0x76, 0x73, 0x79, 0x6e, 0x63,
]);

/// Convert a Cursor transcript to Claude Code JSONL format.
/// Returns the session ID and the path to the written file.
pub fn convert_to_claude_code(
    cursor_transcript_id: &str,
    messages: &[Message],
    project_dir: &Path,
    branch: Option<&str>,
) -> Result<(Uuid, PathBuf), String> {
    let session_id = Uuid::new_v5(&WORMHOLE_UUID_NAMESPACE, cursor_transcript_id.as_bytes());
    let cc_project_dir = claude_code_project_dir(project_dir);
    std::fs::create_dir_all(&cc_project_dir)
        .map_err(|e| format!("create dir {}: {}", cc_project_dir.display(), e))?;

    let cc_file = cc_project_dir.join(format!("{}.jsonl", session_id));
    if cc_file.exists() {
        return Ok((session_id, cc_file));
    }

    let timestamp_base = "2026-01-01T00:00:00.000Z";
    let cwd = project_dir.to_string_lossy().to_string();
    let branch = branch.unwrap_or("main");

    let mut lines = Vec::new();

    // Queue operation: dequeue
    lines.push(serde_json::json!({
        "type": "queue-operation",
        "operation": "dequeue",
        "timestamp": timestamp_base,
        "sessionId": session_id.to_string(),
    }));

    let mut prev_uuid: Option<Uuid> = None;
    for (i, msg) in messages.iter().enumerate() {
        let msg_uuid = Uuid::new_v5(&session_id, format!("msg-{}", i).as_bytes());
        let timestamp = format!("2026-01-01T00:00:{:02}.000Z", (i + 1).min(59));

        match msg.role {
            Role::User => {
                lines.push(serde_json::json!({
                    "type": "user",
                    "parentUuid": prev_uuid.map(|u| u.to_string()),
                    "isSidechain": false,
                    "userType": "external",
                    "cwd": cwd,
                    "sessionId": session_id.to_string(),
                    "version": "2.1.58",
                    "gitBranch": branch,
                    "message": {
                        "role": "user",
                        "content": msg.text,
                    },
                    "uuid": msg_uuid.to_string(),
                    "timestamp": timestamp,
                    "permissionMode": "default",
                }));
            }
            Role::Assistant => {
                lines.push(serde_json::json!({
                    "type": "assistant",
                    "parentUuid": prev_uuid.map(|u| u.to_string()),
                    "isSidechain": false,
                    "userType": "external",
                    "cwd": cwd,
                    "sessionId": session_id.to_string(),
                    "version": "2.1.58",
                    "gitBranch": branch,
                    "message": {
                        "role": "assistant",
                        "content": [{ "type": "text", "text": msg.text }],
                        "model": "claude-sonnet-4-20250514",
                        "type": "message",
                        "id": format!("msg_converted_{}", i),
                        "stop_reason": null,
                        "stop_sequence": null,
                        "usage": { "input_tokens": 100, "output_tokens": 50 },
                    },
                    "uuid": msg_uuid.to_string(),
                    "timestamp": timestamp,
                }));
            }
        }
        prev_uuid = Some(msg_uuid);
    }

    let content: String = lines
        .iter()
        .map(|l| serde_json::to_string(l).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&cc_file, content).map_err(|e| format!("write {}: {}", cc_file.display(), e))?;

    // Update sessions-index.json
    let first_prompt = messages
        .iter()
        .find(|m| m.role == Role::User)
        .map(|m| {
            let s = &m.text;
            if s.len() > 80 {
                &s[..80]
            } else {
                s
            }
        })
        .unwrap_or("")
        .to_string();

    update_sessions_index(
        &cc_project_dir,
        &session_id,
        &cc_file,
        &first_prompt,
        messages.len(),
        project_dir,
        branch,
    )?;

    Ok((session_id, cc_file))
}

fn claude_code_project_dir(project_dir: &Path) -> PathBuf {
    let encoded = encode_path_for_claude_code(project_dir);
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".claude/projects")
        .join(encoded)
}

fn encode_path_for_claude_code(path: &Path) -> String {
    let s = path.to_string_lossy();
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '/' | '.' => result.push('-'),
            _ => result.push(c),
        }
    }
    result
}

fn update_sessions_index(
    cc_project_dir: &Path,
    session_id: &Uuid,
    cc_file: &Path,
    first_prompt: &str,
    message_count: usize,
    project_dir: &Path,
    branch: &str,
) -> Result<(), String> {
    let index_path = cc_project_dir.join("sessions-index.json");

    let mut index: serde_json::Value = if index_path.exists() {
        let content =
            std::fs::read_to_string(&index_path).map_err(|e| format!("read index: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("parse index: {}", e))?
    } else {
        serde_json::json!({
            "version": 1,
            "entries": [],
            "originalPath": project_dir.to_string_lossy(),
        })
    };

    let entries = index
        .get_mut("entries")
        .and_then(|e| e.as_array_mut())
        .ok_or("invalid sessions-index.json")?;

    // Don't add duplicate
    let sid = session_id.to_string();
    if entries
        .iter()
        .any(|e| e.get("sessionId").and_then(|s| s.as_str()) == Some(&sid))
    {
        return Ok(());
    }

    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    entries.push(serde_json::json!({
        "sessionId": sid,
        "fullPath": cc_file.to_string_lossy(),
        "fileMtime": now as u64,
        "firstPrompt": first_prompt,
        "messageCount": message_count,
        "created": "2026-01-01T00:00:00.000Z",
        "modified": "2026-01-01T00:00:00.000Z",
        "gitBranch": branch,
        "projectPath": project_dir.to_string_lossy(),
        "isSidechain": false,
    }));

    std::fs::write(&index_path, serde_json::to_string_pretty(&index).unwrap())
        .map_err(|e| format!("write index: {}", e))?;
    Ok(())
}

pub fn parse_claude_code_jsonl(path: &Path) -> Result<Vec<Message>, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;
    parse_claude_code_jsonl_str(&content)
}

fn parse_claude_code_jsonl_str(content: &str) -> Result<Vec<Message>, String> {
    let mut messages = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let record: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let msg_type = record.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match msg_type {
            "user" => {
                let text = extract_cc_user_text(&record);
                let text = strip_user_query_tags(&text).trim().to_string();
                if !text.is_empty() {
                    messages.push(Message {
                        role: Role::User,
                        text,
                    });
                }
            }
            "assistant" => {
                let text = extract_cc_assistant_text(&record);
                let text = text.trim().to_string();
                if !text.is_empty() {
                    messages.push(Message {
                        role: Role::Assistant,
                        text,
                    });
                }
            }
            _ => {}
        }
    }
    Ok(messages)
}

/// Extract user text from CC JSONL. Content may be a string or an array of blocks.
/// Skip tool_result blocks (those are tool responses, not user-authored text).
fn extract_cc_user_text(record: &serde_json::Value) -> String {
    let content = match record.get("message").and_then(|m| m.get("content")) {
        Some(c) => c,
        None => return String::new(),
    };
    if let Some(s) = content.as_str() {
        return s.to_string();
    }
    let blocks = match content.as_array() {
        Some(a) => a,
        None => return String::new(),
    };
    let mut parts = Vec::new();
    for block in blocks {
        let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if btype == "text" {
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                parts.push(text);
            }
        }
    }
    parts.join("\n")
}

/// Extract assistant text from CC JSONL. Content is always an array; keep only text blocks.
fn extract_cc_assistant_text(record: &serde_json::Value) -> String {
    let blocks = match record
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array())
    {
        Some(a) => a,
        None => return String::new(),
    };
    let mut parts = Vec::new();
    for block in blocks {
        let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        if btype == "text" {
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                parts.push(text);
            }
        }
    }
    parts.join("\n")
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
            encode_path_for_cursor(Path::new(
                "/Users/dan/src/wormhole/.git/wormhole/workspaces/wormhole.code-workspace"
            )),
            "Users-dan-src-wormhole-git-wormhole-workspaces-wormhole-code-workspace"
        );
        // : in task names becomes -
        assert_eq!(
            encode_path_for_cursor(Path::new(
                "/Users/dan/src/wormhole/.git/wormhole/workspaces/wormhole:features.code-workspace"
            )),
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
        let cursor_dir = tmp
            .path()
            .join(".cursor/projects/Users-test-myproject/agent-transcripts");
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
    fn test_encode_path_for_claude_code() {
        assert_eq!(
            encode_path_for_claude_code(Path::new("/Users/dan/src/wormhole")),
            "-Users-dan-src-wormhole"
        );
        assert_eq!(
            encode_path_for_claude_code(Path::new("/Users/dan/src/wormhole/.git")),
            "-Users-dan-src-wormhole--git"
        );
    }

    #[test]
    fn test_deterministic_session_id() {
        let id1 = Uuid::new_v5(&WORMHOLE_UUID_NAMESPACE, b"abc123");
        let id2 = Uuid::new_v5(&WORMHOLE_UUID_NAMESPACE, b"abc123");
        let id3 = Uuid::new_v5(&WORMHOLE_UUID_NAMESPACE, b"xyz789");
        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_convert_to_claude_code() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("myproject");
        std::fs::create_dir_all(&project_dir).unwrap();

        let messages = vec![
            Message {
                role: Role::User,
                text: "Hello".to_string(),
            },
            Message {
                role: Role::Assistant,
                text: "Hi there!".to_string(),
            },
        ];

        // Override HOME for the test
        let cc_dir = claude_code_project_dir(&project_dir);

        let (session_id, cc_file) =
            convert_to_claude_code("test-transcript-id", &messages, &project_dir, Some("main"))
                .unwrap();

        assert!(cc_file.exists());
        let content = std::fs::read_to_string(&cc_file).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines.len(), 3); // dequeue + user + assistant

        // Verify determinism
        let (session_id2, _) =
            convert_to_claude_code("test-transcript-id", &messages, &project_dir, Some("main"))
                .unwrap();
        assert_eq!(session_id, session_id2);

        // Verify JSONL structure
        let dequeue: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(dequeue["type"], "queue-operation");
        assert_eq!(dequeue["sessionId"], session_id.to_string());

        let user_msg: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(user_msg["type"], "user");
        assert_eq!(user_msg["message"]["content"], "Hello");

        let assist_msg: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
        assert_eq!(assist_msg["type"], "assistant");

        // Check sessions-index.json was created
        let index_path = cc_dir.join("sessions-index.json");
        assert!(index_path.exists());

        // Clean up
        let _ = std::fs::remove_dir_all(&cc_dir);
    }

    #[test]
    fn test_render_conversation() {
        let messages = vec![
            Message {
                role: Role::User,
                text: "Hello".to_string(),
            },
            Message {
                role: Role::Assistant,
                text: "Hi there!".to_string(),
            },
        ];
        let uuid = "8afac8bb-1234-5678-9abc-def012345678";
        let rendered = render_conversation("wormhole", "2026-02-25", uuid, &messages);
        assert!(rendered.starts_with(&format!("# wormhole | 2026-02-25 | {}\n", uuid)));
        assert!(rendered.contains("## User\n\nHello\n"));
        assert!(rendered.contains("## Assistant\n\nHi there!\n"));
    }

    #[test]
    fn test_parse_claude_code_jsonl() {
        let input = r#"{"type":"queue-operation","operation":"dequeue","sessionId":"abc"}
{"type":"user","message":{"content":"Hello world"},"uuid":"u1","sessionId":"abc"}
{"type":"assistant","message":{"content":[{"type":"text","text":"Hi!"},{"type":"tool_use","id":"t1","name":"Read"}]},"uuid":"u2","sessionId":"abc"}
{"type":"user","message":{"content":[{"tool_use_id":"t1","type":"tool_result","content":"file contents"}]},"uuid":"u3","sessionId":"abc"}
{"type":"assistant","message":{"content":[{"type":"text","text":"Got it."}]},"uuid":"u4","sessionId":"abc"}"#;
        let messages = parse_claude_code_jsonl_str(input).unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, Role::User);
        assert_eq!(messages[0].text, "Hello world");
        assert_eq!(messages[1].role, Role::Assistant);
        assert_eq!(messages[1].text, "Hi!");
        assert_eq!(messages[2].role, Role::Assistant);
        assert_eq!(messages[2].text, "Got it.");
    }

    #[test]
    fn test_parse_claude_code_jsonl_strips_user_query() {
        let input = r#"{"type":"user","message":{"content":"<user_query>\nHello\n</user_query>"},"uuid":"u1","sessionId":"abc"}"#;
        let messages = parse_claude_code_jsonl_str(input).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "Hello");
    }

    #[test]
    fn test_parse_claude_code_jsonl_array_user_content() {
        let input = r#"{"type":"user","message":{"content":[{"type":"text","text":"Hello"},{"type":"text","text":"World"}]},"uuid":"u1","sessionId":"abc"}"#;
        let messages = parse_claude_code_jsonl_str(input).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "Hello\nWorld");
    }

    #[test]
    fn test_parse_conversation_header() {
        let uuid = "8afac8bb-1234-5678-9abc-def012345678";
        let header = format!("# wormhole | 2026-02-25 | {}\n", uuid);
        let result = parse_header_line(&header);
        assert_eq!(result, Some(("wormhole".to_string(), uuid.to_string())));
    }

    #[test]
    fn test_parse_since() {
        assert!(parse_since("2w").is_some());
        assert!(parse_since("3d").is_some());
        assert!(parse_since("1m").is_some());
        assert!(parse_since("").is_none());
        assert!(parse_since("abc").is_none());
        assert!(parse_since("2x").is_none());
    }

    #[test]
    fn test_read_cc_jsonl_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let jsonl = dir.path().join("abc123.jsonl");
        std::fs::write(
            &jsonl,
            r#"{"type":"queue-operation","operation":"enqueue","timestamp":"2026-03-19T12:00:00Z","sessionId":"abc123"}
{"type":"queue-operation","operation":"dequeue","timestamp":"2026-03-19T12:00:00Z","sessionId":"abc123"}
{"parentUuid":null,"isSidechain":false,"type":"user","uuid":"u1","timestamp":"2026-03-19T12:00:01Z","cwd":"/Users/dan/worktrees/wormhole/conversation-search/wormhole","sessionId":"abc123","message":{"role":"user","content":[{"type":"text","text":"Hello"}]}}
{"type":"assistant","uuid":"a1","timestamp":"2026-03-19T12:00:02Z","cwd":"/Users/dan/worktrees/wormhole/conversation-search/wormhole","sessionId":"abc123","message":{"role":"assistant","content":[{"type":"text","text":"Hi there"}]}}
"#,
        )
        .unwrap();
        let meta = read_cc_jsonl_metadata(&jsonl).unwrap();
        assert_eq!(
            meta.cwd.as_deref(),
            Some("/Users/dan/worktrees/wormhole/conversation-search/wormhole")
        );
        assert!(!meta.is_sidechain);
        assert_eq!(meta.message_count, 2);
    }

    #[test]
    fn test_discover_cc_from_jsonl_files() {
        let cc_dir = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();
        let project_path = std::fs::canonicalize(project_dir.path())
            .unwrap()
            .to_string_lossy()
            .to_string();

        // Write a JSONL file with cwd pointing to our project
        let jsonl = cc_dir.path().join("sess-001.jsonl");
        std::fs::write(
            &jsonl,
            format!(
                r#"{{"type":"user","isSidechain":false,"uuid":"u1","cwd":"{}","sessionId":"sess-001","message":{{"role":"user","content":[{{"type":"text","text":"Hello"}}]}}}}
{{"type":"assistant","uuid":"a1","cwd":"{}","sessionId":"sess-001","message":{{"role":"assistant","content":[{{"type":"text","text":"Hi"}}]}}}}
"#,
                project_path, project_path
            ),
        )
        .unwrap();

        let canonical = vec![(project_path.clone(), "myproject".to_string())];
        let result = discover_cc_from_jsonl_files(cc_dir.path(), &canonical);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].project_key, "myproject");
        assert_eq!(result[0].transcript_id, "sess-001");
        assert_eq!(result[0].source, TranscriptSource::ClaudeCode);
    }

    #[test]
    fn test_discover_cc_from_jsonl_skips_sidechain() {
        let cc_dir = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();
        let project_path = std::fs::canonicalize(project_dir.path())
            .unwrap()
            .to_string_lossy()
            .to_string();

        let jsonl = cc_dir.path().join("sess-002.jsonl");
        std::fs::write(
            &jsonl,
            format!(
                r#"{{"type":"user","isSidechain":true,"uuid":"u1","cwd":"{}","sessionId":"sess-002","message":{{"role":"user","content":[{{"type":"text","text":"Hello"}}]}}}}
{{"type":"assistant","uuid":"a1","cwd":"{}","sessionId":"sess-002","message":{{"role":"assistant","content":[{{"type":"text","text":"Hi"}}]}}}}
"#,
                project_path, project_path
            ),
        )
        .unwrap();

        let canonical = vec![(project_path.clone(), "myproject".to_string())];
        let result = discover_cc_from_jsonl_files(cc_dir.path(), &canonical);
        assert_eq!(result.len(), 0);
    }

}
