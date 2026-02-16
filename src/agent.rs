use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    Cursor,
    Claude,
}

static AGENT: OnceLock<Agent> = OnceLock::new();

pub fn agent() -> Agent {
    *AGENT.get_or_init(|| match std::env::var("WORMHOLE_AGENT").ok().as_deref() {
        Some("claude") => Agent::Claude,
        Some("cursor") | Some("") | None => Agent::Cursor,
        Some(other) => {
            eprintln!("Unknown WORMHOLE_AGENT={:?}, defaulting to cursor", other);
            Agent::Cursor
        }
    })
}

pub fn agent_command(prompt: &str) -> Vec<String> {
    match agent() {
        Agent::Claude => claude_command(prompt),
        Agent::Cursor => cursor_command(prompt),
    }
}

pub fn claude_command(prompt: &str) -> Vec<String> {
    vec![
        "claude".into(),
        "--print".into(),
        "--verbose".into(),
        "--output-format=stream-json".into(),
        "--include-partial-messages".into(),
        "--allowedTools=Bash".into(),
        prompt.into(),
    ]
}

pub fn cursor_command(prompt: &str) -> Vec<String> {
    vec![
        "cursor".into(),
        "agent".into(),
        "--print".into(),
        "--trust".into(),
        "--yolo".into(),
        "--output-format=stream-json".into(),
        "--stream-partial-output".into(),
        prompt.into(),
    ]
}

pub fn agent_name() -> &'static str {
    match agent() {
        Agent::Cursor => "cursor",
        Agent::Claude => "claude",
    }
}
