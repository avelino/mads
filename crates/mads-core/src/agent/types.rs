use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    events::EventSink,
    tools::{ToolHost, ToolSpec},
    usage::Usage,
};

#[derive(Debug, Clone, PartialEq)]
pub struct ChatRequest {
    pub system: String,
    pub messages: Vec<Message>,
    pub tools: Vec<ToolSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Message {
    User(String),
    Assistant {
        text: Option<String>,
        tool_calls: Vec<ToolCall>,
    },
    ToolResults(Vec<ToolResult>),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
    /// Opaque provider data that must come back with the call (Gemini thought signatures).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolResult {
    pub call_id: String,
    pub content: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
    #[serde(default)]
    pub usage: Usage,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChatError {
    /// Rate limits, 5xx and timeouts: worth retrying.
    #[error("{0}")]
    Transient(String),
    #[error("{0}")]
    Fatal(String),
}

#[async_trait]
pub trait ChatModel: Send + Sync {
    /// `provider/model`, for logs and the report.
    fn label(&self) -> String;
    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, ChatError>;
}

/// One unit of agent work: a system prompt, the first user message and an id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionSpec {
    pub id: String,
    pub system: String,
    pub user: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissionOutcome {
    Finished,
    Failed(String),
    BudgetExceeded,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MissionReport {
    pub outcome: MissionOutcome,
    pub usage: Usage,
    pub turns: u32,
}

/// Run-wide token ceiling shared by every mission. Zero means unlimited.
#[derive(Debug)]
pub struct TokenBudget {
    max: u64,
    used: AtomicU64,
}

impl TokenBudget {
    pub fn new(max: u64) -> Self {
        Self {
            max,
            used: AtomicU64::new(0),
        }
    }

    /// Adds the usage and reports whether the ceiling is now exceeded.
    pub fn add(&self, usage: &Usage) -> bool {
        self.used.fetch_add(usage.total_tokens(), Ordering::SeqCst);
        self.exceeded()
    }

    pub fn exceeded(&self) -> bool {
        self.max > 0 && self.used.load(Ordering::SeqCst) > self.max
    }

    pub fn used(&self) -> u64 {
        self.used.load(Ordering::SeqCst)
    }
}

pub struct DriverCtx {
    pub events: EventSink,
    pub max_turns: usize,
    pub budget: Arc<TokenBudget>,
    /// Directory for per-mission transcripts. `None` records nothing.
    pub transcripts: Option<PathBuf>,
}

/// `campaign:x` becomes `campaign-x.<extension>`: a colon breaks artifact uploads on some platforms.
pub fn transcript_path(dir: &Path, mission_id: &str, extension: &str) -> PathBuf {
    dir.join(format!("{}.{extension}", mission_id.replace(':', "-")))
}

impl DriverCtx {
    /// Appends one line to the mission transcript. Best effort: a full disk must not fail a mission.
    pub fn append_transcript(&self, mission_id: &str, extension: &str, line: &str) {
        let Some(dir) = &self.transcripts else { return };
        let path = transcript_path(dir, mission_id, extension);
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(f, "{line}");
        }
    }

    pub fn record(&self, mission_id: &str, message: &Message) {
        if self.transcripts.is_some()
            && let Ok(line) = serde_json::to_string(message)
        {
            self.append_transcript(mission_id, "jsonl", &line);
        }
    }
}

/// Runs a whole mission against a tool host. API providers loop over `ChatModel`;
/// agent CLIs reach the tools through the MCP bridge.
#[async_trait]
pub trait Driver: Send + Sync {
    async fn run_mission(
        &self,
        mission: &MissionSpec,
        tools: Arc<dyn ToolHost>,
        ctx: &DriverCtx,
    ) -> MissionReport;
}

/// `tool` plus the `name` argument when there is one, for progress lines.
pub fn describe_call(tool: &str, args: &Value) -> String {
    match args.get("name").and_then(Value::as_str) {
        Some(n) => format!("{tool} {n}"),
        None => tool.to_string(),
    }
}
