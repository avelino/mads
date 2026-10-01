use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use async_trait::async_trait;
use serde::Deserialize;

use super::{
    ChatError, ChatModel, ChatRequest, ChatResponse, Driver, DriverCtx, LoopDriver, MissionOutcome,
    MissionReport, MissionSpec,
};
use crate::{tools::ToolHost, usage::Usage};

type Queue = Arc<Mutex<VecDeque<Result<ChatResponse, ChatError>>>>;

/// Plays back recorded responses and remembers every request it received.
pub struct ScriptedChatModel {
    label: String,
    queue: Queue,
    requests: Mutex<Vec<ChatRequest>>,
}

impl ScriptedChatModel {
    pub fn new(label: &str, script: Vec<Result<ChatResponse, ChatError>>) -> Self {
        Self::shared(label, Arc::new(Mutex::new(script.into())))
    }

    fn shared(label: &str, queue: Queue) -> Self {
        Self {
            label: label.into(),
            queue,
            requests: Mutex::new(Vec::new()),
        }
    }

    pub fn requests(&self) -> Vec<ChatRequest> {
        self.requests.lock().map(|r| r.clone()).unwrap_or_default()
    }
}

#[async_trait]
impl ChatModel for ScriptedChatModel {
    fn label(&self) -> String {
        self.label.clone()
    }

    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, ChatError> {
        if let Ok(mut r) = self.requests.lock() {
            r.push(req.clone());
        }
        let next = self.queue.lock().ok().and_then(|mut q| q.pop_front());
        next.unwrap_or_else(|| Err(ChatError::Fatal("replay script exhausted".into())))
    }
}

#[derive(Deserialize)]
struct ScriptFile {
    missions: HashMap<String, Vec<ChatResponse>>,
}

/// Replays a JSON script (`{"missions": {"plan": [...], "campaign:<slug>": [...]}}`) without any provider.
pub struct ScriptedDriver {
    scripts: HashMap<String, Queue>,
}

impl ScriptedDriver {
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        let file: ScriptFile = serde_json::from_str(text)?;
        let scripts = file
            .missions
            .into_iter()
            .map(|(id, responses)| {
                (
                    id,
                    Arc::new(Mutex::new(responses.into_iter().map(Ok).collect())),
                )
            })
            .collect();
        Ok(Self { scripts })
    }

    pub fn has_script(&self, mission_id: &str) -> bool {
        self.scripts.contains_key(mission_id)
    }
}

#[async_trait]
impl Driver for ScriptedDriver {
    async fn run_mission(
        &self,
        mission: &MissionSpec,
        tools: Arc<dyn ToolHost>,
        ctx: &DriverCtx,
    ) -> MissionReport {
        let Some(queue) = self.scripts.get(&mission.id) else {
            let outcome = MissionOutcome::Failed(format!("no script for mission {}", mission.id));
            return MissionReport {
                outcome,
                usage: Usage::default(),
                turns: 0,
            };
        };
        let model = Arc::new(ScriptedChatModel::shared("replay", queue.clone()));
        LoopDriver::new(model)
            .with_backoff(Duration::ZERO)
            .run_mission(mission, tools, ctx)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::{
        agent::*,
        events::EventSink,
        tools::{ToolHost, ToolOutput},
    };

    #[test]
    fn script_file_parses_missions() {
        let json = json!({"missions": {"plan": [{"text": "hi", "tool_calls": [], "usage": {"input_tokens": 1, "output_tokens": 2, "cost_usd": null}}]}});
        let d = ScriptedDriver::from_json(&json.to_string()).unwrap();
        assert!(d.has_script("plan"));
        assert!(!d.has_script("campaign:x"));
    }

    #[test]
    fn invalid_script_is_an_error() {
        assert!(ScriptedDriver::from_json("{").is_err());
    }

    #[tokio::test]
    async fn driver_runs_the_script_of_the_mission_id() {
        struct Done;
        #[async_trait::async_trait]
        impl ToolHost for Done {
            fn specs(&self) -> Vec<crate::tools::ToolSpec> {
                vec![]
            }
            async fn call(&self, _: &str, _: serde_json::Value) -> ToolOutput {
                ToolOutput::ok(json!({}), &[], "ok")
            }
            fn finished(&self) -> bool {
                true
            }
        }
        let script = json!({"missions": {"m": [{"text": null, "tool_calls": [{"id": "1", "name": "finish", "arguments": {}}], "usage": {"input_tokens": 3, "output_tokens": 4, "cost_usd": null}}]}});
        let driver = ScriptedDriver::from_json(&script.to_string()).unwrap();
        let (events, _rx) = EventSink::channel();
        let ctx = DriverCtx {
            events,
            max_turns: 5,
            budget: Arc::new(TokenBudget::new(0)),
            transcripts: None,
        };
        let mission = MissionSpec {
            id: "m".into(),
            system: String::new(),
            user: "u".into(),
            web_search: false,
        };
        let report = driver.run_mission(&mission, Arc::new(Done), &ctx).await;
        assert_eq!(report.outcome, MissionOutcome::Finished);
        assert_eq!(report.usage.input_tokens, 3);
        let missing = MissionSpec {
            id: "other".into(),
            system: String::new(),
            user: "u".into(),
            web_search: false,
        };
        let report = driver.run_mission(&missing, Arc::new(Done), &ctx).await;
        assert!(matches!(report.outcome, MissionOutcome::Failed(ref m) if m.contains("no script")));
    }
}
