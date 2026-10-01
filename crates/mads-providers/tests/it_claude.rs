//! Real run against the `claude` CLI. Costs a few cents and needs a logged-in `claude`.
//! `MADS_IT=1 cargo test -p mads-providers --test it_claude -- --ignored --nocapture`
#![allow(clippy::unwrap_used)]

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use mads_core::{
    agent::{Driver, DriverCtx, MissionOutcome, MissionSpec, TokenBudget},
    events::{Event, EventSink},
    tools::{ToolHost, ToolOutput, ToolSpec},
};
use mads_providers::{Claude, CliDriver};
use serde_json::{Value, json};

struct Tools {
    finished: AtomicBool,
}

#[async_trait]
impl ToolHost for Tools {
    fn specs(&self) -> Vec<ToolSpec> {
        vec![ToolSpec {
            name: "finish".into(),
            description: "Call this tool to finish the task.".into(),
            input_schema: json!({"type": "object", "properties": {}}),
        }]
    }

    async fn call(&self, name: &str, _args: Value) -> ToolOutput {
        if name == "finish" {
            self.finished.store(true, Ordering::SeqCst);
            return ToolOutput::ok(json!({"done": true}), &[], "finished");
        }
        ToolOutput::fail("UNKNOWN_TOOL", name)
    }

    fn finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }
}

#[tokio::test]
#[ignore = "spends tokens on the real claude CLI"]
async fn claude_reaches_our_tools_through_the_mcp_bridge() {
    if std::env::var("MADS_IT").is_err() {
        return;
    }
    let tools = Arc::new(Tools {
        finished: AtomicBool::new(false),
    });
    let (events, mut rx) = EventSink::channel();
    let transcripts = tempfile::tempdir().unwrap();
    let ctx = DriverCtx {
        events,
        max_turns: 6,
        budget: Arc::new(TokenBudget::new(0)),
        transcripts: Some(transcripts.path().to_path_buf()),
    };
    let mission = MissionSpec {
        id: "it".into(),
        system: "You are a test agent. Use only the tools you are given.".into(),
        user: "Call the finish tool now.".into(),
        web_search: false,
    };
    let report = CliDriver::new(Claude)
        .run_mission(&mission, tools.clone(), &ctx)
        .await;

    let mut seen = Vec::new();
    while let Ok(e) = rx.try_recv() {
        seen.push(e.event);
    }
    println!(
        "outcome: {:?}\nusage: {:?}\nevents: {seen:#?}",
        report.outcome, report.usage
    );
    if let Ok(raw) = std::fs::read_to_string(transcripts.path().join("it.cli.jsonl")) {
        for line in raw.lines() {
            let v: Value = serde_json::from_str(line).unwrap_or(Value::Null);
            if v["type"] == "system" && v["subtype"] == "init" {
                println!("INIT mcp_servers={} tools={}", v["mcp_servers"], v["tools"]);
            }
        }
    }
    assert_eq!(report.outcome, MissionOutcome::Finished);
    assert!(tools.finished());
    assert!(
        seen.iter()
            .any(|e| matches!(e, Event::ToolCalled { tool, .. } if tool == "finish"))
    );
    assert!(report.usage.input_tokens > 0, "claude reports usage");
}
