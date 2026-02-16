use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    Cursor,
    Claude,
    CursorInteractive,
    ClaudeInteractive,
}

impl Agent {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "cursor" => Some(Agent::Cursor),
            "claude" => Some(Agent::Claude),
            "cursor-interactive" => Some(Agent::CursorInteractive),
            "claude-interactive" => Some(Agent::ClaudeInteractive),
            _ => None,
        }
    }

    pub fn is_interactive(self) -> bool {
        matches!(self, Agent::CursorInteractive | Agent::ClaudeInteractive)
    }

    pub fn name(self) -> &'static str {
        match self {
            Agent::Cursor | Agent::CursorInteractive => "cursor",
            Agent::Claude | Agent::ClaudeInteractive => "claude",
        }
    }

    pub fn command(self, prompt: &str) -> Vec<String> {
        match self {
            Agent::Claude => claude_command(prompt),
            Agent::Cursor => cursor_command(prompt),
            Agent::ClaudeInteractive => claude_interactive_command(prompt),
            Agent::CursorInteractive => cursor_interactive_command(prompt),
        }
    }
}

static AGENT: OnceLock<Agent> = OnceLock::new();

pub fn default_agent() -> Agent {
    *AGENT.get_or_init(|| match std::env::var("WORMHOLE_AGENT").ok().as_deref() {
        Some("claude") => Agent::Claude,
        Some("claude-interactive") => Agent::ClaudeInteractive,
        Some("cursor-interactive") => Agent::CursorInteractive,
        Some("cursor") | Some("") | None => Agent::Cursor,
        Some(other) => {
            eprintln!("Unknown WORMHOLE_AGENT={:?}, defaulting to cursor", other);
            Agent::Cursor
        }
    })
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

pub fn claude_interactive_command(prompt: &str) -> Vec<String> {
    vec![
        "claude".into(),
        "--dangerously-skip-permissions".into(),
        "--allowedTools=Bash".into(),
        prompt.into(),
    ]
}

pub fn cursor_interactive_command(prompt: &str) -> Vec<String> {
    vec![
        "cursor".into(),
        "agent".into(),
        "--yolo".into(),
        prompt.into(),
    ]
}
