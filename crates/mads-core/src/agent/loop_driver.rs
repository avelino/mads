use std::{sync::Arc, time::Duration};

use async_trait::async_trait;

use super::{
    ChatError, ChatModel, ChatRequest, ChatResponse, Driver, DriverCtx, Message, MissionOutcome,
    MissionReport, MissionSpec, ToolResult, describe_call,
};
use crate::{events::Event, tools::ToolHost, usage::Usage};

const NUDGE: &str = "Continue using the tools. Call finish when the work is done.";
const MAX_NUDGES: u32 = 2;
const MAX_ATTEMPTS: u32 = 3;

/// Drives a `ChatModel` through the tool loop of one mission.
pub struct LoopDriver {
    model: Arc<dyn ChatModel>,
    backoff: Duration,
}

impl LoopDriver {
    pub fn new(model: Arc<dyn ChatModel>) -> Self {
        Self {
            model,
            backoff: Duration::from_secs(1),
        }
    }

    pub fn with_backoff(mut self, backoff: Duration) -> Self {
        self.backoff = backoff;
        self
    }

    /// Transient errors wait `backoff`, then `2 * backoff`, then give up on the third failure.
    async fn complete(&self, req: &ChatRequest) -> Result<ChatResponse, String> {
        let mut wait = self.backoff;
        for attempt in 1..=MAX_ATTEMPTS {
            match self.model.complete(req).await {
                Ok(r) => return Ok(r),
                Err(ChatError::Fatal(m)) => return Err(m),
                Err(ChatError::Transient(m)) if attempt == MAX_ATTEMPTS => return Err(m),
                Err(ChatError::Transient(_)) => {
                    tokio::time::sleep(wait).await;
                    wait *= 2;
                }
            }
        }
        Err("unreachable".into())
    }

    async fn run_tools(
        &self,
        mission: &MissionSpec,
        calls: &[super::ToolCall],
        tools: &dyn ToolHost,
        ctx: &DriverCtx,
    ) -> Vec<ToolResult> {
        let mut results = Vec::with_capacity(calls.len());
        for call in calls {
            let summary = describe_call(&call.name, &call.arguments);
            ctx.events.emit(Event::ToolCalled {
                mission: mission.id.clone(),
                tool: call.name.clone(),
                summary,
            });
            let out = tools.call(&call.name, call.arguments.clone()).await;
            ctx.events.emit(Event::ToolFinished {
                mission: mission.id.clone(),
                tool: call.name.clone(),
                ok: !out.is_error,
                summary: out.summary.clone(),
            });
            results.push(ToolResult {
                call_id: call.id.clone(),
                content: out.text(),
                is_error: out.is_error,
            });
        }
        results
    }
}

#[async_trait]
impl Driver for LoopDriver {
    async fn run_mission(
        &self,
        mission: &MissionSpec,
        tools: Arc<dyn ToolHost>,
        ctx: &DriverCtx,
    ) -> MissionReport {
        let specs = tools.specs();
        let mut messages = vec![Message::User(mission.user.clone())];
        ctx.record(&mission.id, &messages[0]);
        let (mut usage, mut turns, mut nudges) = (Usage::default(), 0u32, 0u32);
        let done = |outcome, usage: Usage, turns| MissionReport {
            outcome,
            usage,
            turns,
        };

        for turn in 1..=ctx.max_turns {
            turns = turn as u32;
            let req = ChatRequest {
                system: mission.system.clone(),
                messages: messages.clone(),
                tools: specs.clone(),
            };
            let resp = match self.complete(&req).await {
                Ok(r) => r,
                Err(m) => return done(MissionOutcome::Failed(m), usage, turns),
            };
            usage.add(&resp.usage);
            ctx.events.emit(Event::Usage {
                mission: mission.id.clone(),
                input_tokens: resp.usage.input_tokens,
                output_tokens: resp.usage.output_tokens,
                cost_usd: resp.usage.cost_usd,
            });
            if ctx.budget.add(&resp.usage) {
                return done(MissionOutcome::BudgetExceeded, usage, turns);
            }
            if let Some(text) = resp.text.as_ref().filter(|t| !t.trim().is_empty()) {
                ctx.events.emit(Event::AgentText {
                    mission: mission.id.clone(),
                    text: text.clone(),
                });
            }
            messages.push(Message::Assistant {
                text: resp.text.clone(),
                tool_calls: resp.tool_calls.clone(),
            });
            ctx.record(&mission.id, &messages[messages.len() - 1]);

            if resp.tool_calls.is_empty() {
                if tools.finished() {
                    return done(MissionOutcome::Finished, usage, turns);
                }
                nudges += 1;
                if nudges > MAX_NUDGES {
                    return done(MissionOutcome::Failed("no progress".into()), usage, turns);
                }
                messages.push(Message::User(NUDGE.into()));
                ctx.record(&mission.id, &messages[messages.len() - 1]);
                continue;
            }
            nudges = 0;
            let results = self
                .run_tools(mission, &resp.tool_calls, tools.as_ref(), ctx)
                .await;
            messages.push(Message::ToolResults(results));
            ctx.record(&mission.id, &messages[messages.len() - 1]);
            if tools.finished() {
                return done(MissionOutcome::Finished, usage, turns);
            }
        }
        done(MissionOutcome::Failed("max turns".into()), usage, turns)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use serde_json::{Value, json};

    use super::*;
    use crate::{
        agent::*,
        events::{Event, EventSink},
        tools::{ToolHost, ToolOutput, ToolSpec},
        usage::Usage,
    };

    struct FakeTools {
        finished: AtomicBool,
    }

    impl FakeTools {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                finished: AtomicBool::new(false),
            })
        }
    }

    #[async_trait]
    impl ToolHost for FakeTools {
        fn specs(&self) -> Vec<ToolSpec> {
            ["echo", "finish"]
                .iter()
                .map(|n| ToolSpec {
                    name: (*n).into(),
                    description: "d".into(),
                    input_schema: json!({"type": "object"}),
                })
                .collect()
        }

        async fn call(&self, name: &str, args: Value) -> ToolOutput {
            match name {
                "echo" => ToolOutput::ok(json!({"echo": args}), &[], "echoed"),
                "finish" => {
                    self.finished.store(true, Ordering::SeqCst);
                    ToolOutput::ok(json!({}), &[], "finished")
                }
                other => ToolOutput::fail("UNKNOWN_TOOL", other),
            }
        }

        fn finished(&self) -> bool {
            self.finished.load(Ordering::SeqCst)
        }
    }

    fn usage(i: u64, o: u64) -> Usage {
        Usage {
            input_tokens: i,
            output_tokens: o,
            cost_usd: None,
        }
    }

    fn calls(text: Option<&str>, tools: &[&str]) -> Result<ChatResponse, ChatError> {
        Ok(ChatResponse {
            text: text.map(String::from),
            tool_calls: tools
                .iter()
                .enumerate()
                .map(|(i, t)| ToolCall {
                    id: format!("c{i}"),
                    name: (*t).into(),
                    arguments: json!({"n": i}),
                    extra: None,
                })
                .collect(),
            usage: usage(10, 5),
        })
    }

    fn text(t: &str) -> Result<ChatResponse, ChatError> {
        calls(Some(t), &[])
    }

    fn mission() -> MissionSpec {
        MissionSpec {
            id: "m".into(),
            system: "sys".into(),
            user: "go".into(),
            web_search: false,
        }
    }

    struct Harness {
        driver: LoopDriver,
        model: Arc<ScriptedChatModel>,
        ctx: DriverCtx,
        rx: tokio::sync::mpsc::UnboundedReceiver<crate::events::EventEnvelope>,
    }

    fn harness(
        script: Vec<Result<ChatResponse, ChatError>>,
        max_turns: usize,
        max_tokens: u64,
    ) -> Harness {
        let model = Arc::new(ScriptedChatModel::new("scripted", script));
        let driver = LoopDriver::new(model.clone()).with_backoff(Duration::ZERO);
        let (events, rx) = EventSink::channel();
        let ctx = DriverCtx {
            events,
            max_turns,
            budget: Arc::new(TokenBudget::new(max_tokens)),
            transcripts: None,
        };
        Harness {
            driver,
            model,
            ctx,
            rx,
        }
    }

    fn drain(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<crate::events::EventEnvelope>,
    ) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(e) = rx.try_recv() {
            out.push(e.event);
        }
        out
    }

    fn kinds(events: &[Event]) -> Vec<&'static str> {
        events
            .iter()
            .map(|e| match e {
                Event::Usage { .. } => "usage",
                Event::AgentText { .. } => "text",
                Event::ToolCalled { .. } => "called",
                Event::ToolFinished { .. } => "finished",
                _ => "other",
            })
            .collect()
    }

    #[tokio::test]
    async fn finishes_when_the_finish_tool_succeeds_and_reports_usage() {
        let mut h = harness(
            vec![calls(Some("thinking"), &["echo"]), calls(None, &["finish"])],
            10,
            0,
        );
        let tools = FakeTools::new();
        let report = h.driver.run_mission(&mission(), tools, &h.ctx).await;
        assert_eq!(report.outcome, MissionOutcome::Finished);
        assert_eq!(report.turns, 2);
        assert_eq!(
            (report.usage.input_tokens, report.usage.output_tokens),
            (20, 10)
        );
        assert_eq!(
            kinds(&drain(&mut h.rx)),
            [
                "usage", "text", "called", "finished", "usage", "called", "finished"
            ]
        );
    }

    #[tokio::test]
    async fn tool_results_are_fed_back_to_the_model() {
        let h = harness(
            vec![calls(None, &["echo"]), calls(None, &["finish"])],
            10,
            0,
        );
        h.driver
            .run_mission(&mission(), FakeTools::new(), &h.ctx)
            .await;
        let requests = h.model.requests();
        assert_eq!(requests.len(), 2);
        assert_eq!(requests[0].system, "sys");
        assert_eq!(requests[0].messages, vec![Message::User("go".into())]);
        assert_eq!(requests[0].tools.len(), 2);
        match requests[1].messages.last().unwrap() {
            Message::ToolResults(r) => {
                assert_eq!(r[0].call_id, "c0");
                assert!(r[0].content.contains("echo") && !r[0].is_error);
            }
            other => panic!("expected tool results, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn text_only_turns_get_nudged_then_fail_with_no_progress() {
        let h = harness(vec![text("a"), text("b"), text("c")], 10, 0);
        let report = h
            .driver
            .run_mission(&mission(), FakeTools::new(), &h.ctx)
            .await;
        assert_eq!(report.outcome, MissionOutcome::Failed("no progress".into()));
        let requests = h.model.requests();
        assert!(
            matches!(requests[1].messages.last().unwrap(), Message::User(m) if m.contains("finish"))
        );
    }

    #[tokio::test]
    async fn a_nudge_can_recover() {
        let h = harness(vec![text("a"), calls(None, &["finish"])], 10, 0);
        assert_eq!(
            h.driver
                .run_mission(&mission(), FakeTools::new(), &h.ctx)
                .await
                .outcome,
            MissionOutcome::Finished
        );
    }

    #[tokio::test]
    async fn nudge_counter_resets_after_a_tool_turn() {
        let script = vec![
            text("a"),
            text("b"),
            calls(None, &["echo"]),
            text("c"),
            text("d"),
            calls(None, &["finish"]),
        ];
        let h = harness(script, 10, 0);
        assert_eq!(
            h.driver
                .run_mission(&mission(), FakeTools::new(), &h.ctx)
                .await
                .outcome,
            MissionOutcome::Finished
        );
    }

    #[tokio::test]
    async fn stops_at_max_turns() {
        let script = (0..10).map(|_| calls(None, &["echo"])).collect();
        let h = harness(script, 3, 0);
        let report = h
            .driver
            .run_mission(&mission(), FakeTools::new(), &h.ctx)
            .await;
        assert_eq!(report.outcome, MissionOutcome::Failed("max turns".into()));
        assert_eq!(report.turns, 3);
    }

    #[tokio::test]
    async fn transient_errors_are_retried_up_to_three_attempts() {
        let script = vec![
            Err(ChatError::Transient("429".into())),
            Err(ChatError::Transient("429".into())),
            calls(None, &["finish"]),
        ];
        let h = harness(script, 10, 0);
        assert_eq!(
            h.driver
                .run_mission(&mission(), FakeTools::new(), &h.ctx)
                .await
                .outcome,
            MissionOutcome::Finished
        );
        assert_eq!(h.model.requests().len(), 3);
    }

    #[tokio::test]
    async fn four_transient_errors_fail_the_mission() {
        let script = (0..4)
            .map(|_| Err(ChatError::Transient("503".into())))
            .collect();
        let h = harness(script, 10, 0);
        let report = h
            .driver
            .run_mission(&mission(), FakeTools::new(), &h.ctx)
            .await;
        assert!(
            matches!(report.outcome, MissionOutcome::Failed(ref m) if m.contains("503")),
            "{:?}",
            report.outcome
        );
        assert_eq!(h.model.requests().len(), 3);
    }

    #[tokio::test]
    async fn fatal_errors_fail_immediately() {
        let h = harness(
            vec![
                Err(ChatError::Fatal("bad key".into())),
                calls(None, &["finish"]),
            ],
            10,
            0,
        );
        let report = h
            .driver
            .run_mission(&mission(), FakeTools::new(), &h.ctx)
            .await;
        assert_eq!(report.outcome, MissionOutcome::Failed("bad key".into()));
        assert_eq!(h.model.requests().len(), 1);
    }

    #[tokio::test]
    async fn token_budget_aborts_with_budget_exceeded() {
        let h = harness(
            vec![
                calls(None, &["echo"]),
                calls(None, &["echo"]),
                calls(None, &["finish"]),
            ],
            10,
            20,
        );
        let report = h
            .driver
            .run_mission(&mission(), FakeTools::new(), &h.ctx)
            .await;
        assert_eq!(report.outcome, MissionOutcome::BudgetExceeded);
        assert_eq!(report.turns, 2);
    }

    #[tokio::test]
    async fn tool_errors_are_returned_to_the_model_not_raised() {
        let h = harness(
            vec![calls(None, &["nope"]), calls(None, &["finish"])],
            10,
            0,
        );
        let report = h
            .driver
            .run_mission(&mission(), FakeTools::new(), &h.ctx)
            .await;
        assert_eq!(report.outcome, MissionOutcome::Finished);
        match h.model.requests()[1].messages.last().unwrap() {
            Message::ToolResults(r) => assert!(r[0].is_error),
            other => panic!("{other:?}"),
        }
    }

    #[tokio::test]
    async fn multiple_tool_calls_in_one_turn_run_in_order() {
        let h = harness(vec![calls(None, &["echo", "finish"])], 10, 0);
        let report = h
            .driver
            .run_mission(&mission(), FakeTools::new(), &h.ctx)
            .await;
        assert_eq!(report.outcome, MissionOutcome::Finished);
        match h.model.requests().len() {
            1 => {}
            n => panic!("finish in the same turn must stop the loop, got {n} requests"),
        }
    }

    #[tokio::test]
    async fn exhausted_script_is_a_fatal_failure() {
        let h = harness(vec![], 10, 0);
        let report = h
            .driver
            .run_mission(&mission(), FakeTools::new(), &h.ctx)
            .await;
        assert!(matches!(report.outcome, MissionOutcome::Failed(ref m) if m.contains("exhausted")));
    }

    #[test]
    fn token_budget_zero_means_unlimited() {
        let b = TokenBudget::new(0);
        assert!(!b.add(&Usage {
            input_tokens: u64::MAX / 2,
            output_tokens: 0,
            cost_usd: None
        }));
        let b = TokenBudget::new(10);
        assert!(!b.add(&usage(5, 5)));
        assert!(b.add(&usage(1, 0)));
        assert!(b.exceeded());
    }

    #[tokio::test]
    async fn the_transcript_records_every_message_when_a_directory_is_given() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = harness(
            vec![calls(Some("thinking"), &["echo"]), calls(None, &["finish"])],
            10,
            0,
        );
        h.ctx.transcripts = Some(dir.path().to_path_buf());
        let mission = MissionSpec {
            id: "campaign:x".into(),
            system: "sys".into(),
            user: "go".into(),
            web_search: false,
        };
        h.driver
            .run_mission(&mission, FakeTools::new(), &h.ctx)
            .await;
        let text = std::fs::read_to_string(dir.path().join("campaign-x.jsonl")).unwrap();
        let lines: Vec<Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(
            lines.len(),
            5,
            "user, assistant, results, assistant, results: {text}"
        );
        assert!(
            lines[0].get("User").is_some()
                && lines[1].get("Assistant").is_some()
                && lines[2].get("ToolResults").is_some()
        );
    }

    #[test]
    fn transcript_paths_never_contain_a_colon() {
        let p = transcript_path(
            std::path::Path::new("t"),
            "campaign:vinellu-catalogo",
            "cli.jsonl",
        );
        assert_eq!(
            p,
            std::path::PathBuf::from("t/campaign-vinellu-catalogo.cli.jsonl")
        );
    }
}
