mod claude;
mod codex;
mod gemini;

use std::{collections::VecDeque, path::Path, process::Stdio, sync::Arc};

use async_trait::async_trait;
use mads_core::{
    agent::{Driver, DriverCtx, MissionOutcome, MissionReport, MissionSpec, describe_call},
    events::{Event, EventSink},
    tools::{ToolHost, ToolOutput, ToolSpec},
    usage::Usage,
};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, BufReader},
    process::Command,
};

pub use claude::Claude;
pub use codex::Codex;
pub use gemini::Gemini;

use crate::mcp::McpEndpoint;

const STDERR_LINES_KEPT: usize = 10;

/// Everything an agent CLI needs to run one mission.
pub struct CliContext<'a> {
    pub system: &'a str,
    pub user: &'a str,
    pub mcp_url: &'a str,
    pub token: &'a str,
    /// Private directory for this mission's config files.
    pub dir: &'a Path,
    pub model: Option<&'a str>,
    pub max_turns: usize,
    /// True when `ANTHROPIC_API_KEY` is set (lets claude run in bare mode).
    pub api_key_set: bool,
    /// Opens the CLI's own web search, and nothing else, next to the mads tools.
    pub web_search: bool,
}

/// The process to start: program, arguments, extra environment and the files to write first
/// (name relative to `CliContext::dir`, content).
pub struct CliInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub files: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CliOutput {
    Text(String),
    Usage(Usage),
    Failure(String),
}

/// What differs between agent CLIs: how to invoke them and how to read their JSON streams.
pub trait CliAgent: Send + Sync {
    fn name(&self) -> &'static str;
    /// Default executable name.
    fn binary(&self) -> &'static str;
    /// Environment variable that overrides the executable path.
    fn binary_env(&self) -> &'static str;
    fn min_version(&self) -> (u32, u32, u32);
    fn build(&self, ctx: &CliContext<'_>) -> CliInvocation;
    /// Unknown or malformed lines return nothing: a newer CLI must not break the run.
    fn parse_line(&self, line: &str) -> Vec<CliOutput>;
}

/// The executable to run: the override variable when set, else the default name.
pub fn program_for(agent: &dyn CliAgent) -> String {
    std::env::var(agent.binary_env())
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| agent.binary().to_string())
}

/// First `x.y.z` found in a `--version` banner.
pub fn parse_version(text: &str) -> Option<(u32, u32, u32)> {
    text.split_whitespace().find_map(|token| {
        let core = token.trim_start_matches('v').split(['-', '+']).next()?;
        let mut parts = core.split('.');
        let version = (
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
            parts.next()?.parse().ok()?,
        );
        parts.next().is_none().then_some(version)
    })
}

/// Fails early, with a message the user can act on, when the CLI is missing or too old.
pub async fn check_cli(program: &str, name: &str, min: (u32, u32, u32)) -> Result<(), String> {
    let out = match Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .await
    {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!(
                "{name} not found ({program}): install it or point the matching MADS_*_BIN variable at it"
            ));
        }
        Err(e) => return Err(format!("cannot run {program}: {e}")),
    };
    let banner =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    let Some(found) = parse_version(&banner) else {
        return Err(format!(
            "cannot read the {name} version from: {}",
            banner.trim()
        ));
    };
    if found < min {
        let f = |v: (u32, u32, u32)| format!("{}.{}.{}", v.0, v.1, v.2);
        return Err(format!(
            "{name} {} is too old: mads needs {} or newer",
            f(found),
            f(min)
        ));
    }
    Ok(())
}

/// Emits `ToolCalled` and `ToolFinished` around every call the agent makes over MCP, so the
/// progress looks the same whatever drives the mission.
struct EventedTools {
    inner: Arc<dyn ToolHost>,
    events: EventSink,
    mission: String,
}

#[async_trait]
impl ToolHost for EventedTools {
    fn specs(&self) -> Vec<ToolSpec> {
        self.inner.specs()
    }

    async fn call(&self, name: &str, args: Value) -> ToolOutput {
        let summary = describe_call(name, &args);
        self.events.emit(Event::ToolCalled {
            mission: self.mission.clone(),
            tool: name.into(),
            summary,
        });
        let out = self.inner.call(name, args).await;
        self.events.emit(Event::ToolFinished {
            mission: self.mission.clone(),
            tool: name.into(),
            ok: !out.is_error,
            summary: out.summary.clone(),
        });
        out
    }

    fn finished(&self) -> bool {
        self.inner.finished()
    }
}

/// Runs a mission by starting an agent CLI that reaches the tools through an MCP endpoint.
pub struct CliDriver<A: CliAgent> {
    agent: A,
    model: Option<String>,
}

impl<A: CliAgent> CliDriver<A> {
    pub fn new(agent: A) -> Self {
        Self { agent, model: None }
    }

    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model;
        self
    }

    #[cfg(test)]
    pub(crate) fn agent(&self) -> &A {
        &self.agent
    }
}

fn failed(message: impl Into<String>, usage: Usage) -> MissionReport {
    MissionReport {
        outcome: MissionOutcome::Failed(message.into()),
        usage,
        turns: 0,
    }
}

#[cfg(unix)]
fn write_private(path: &Path, content: &str) -> std::io::Result<()> {
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(content.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &Path, content: &str) -> std::io::Result<()> {
    std::fs::write(path, content)
}

/// Keeps the last lines of a stream: the tail of stderr is what explains a crash.
async fn tail_of(stream: impl AsyncRead + Unpin) -> String {
    let mut lines = BufReader::new(stream).lines();
    let mut kept: VecDeque<String> = VecDeque::new();
    while let Ok(Some(line)) = lines.next_line().await {
        if kept.len() == STDERR_LINES_KEPT {
            kept.pop_front();
        }
        kept.push_back(line);
    }
    kept.into_iter().collect::<Vec<_>>().join(" | ")
}

#[async_trait]
impl<A: CliAgent> Driver for CliDriver<A> {
    fn web_search(&self) -> bool {
        true
    }

    async fn run_mission(
        &self,
        mission: &MissionSpec,
        tools: Arc<dyn ToolHost>,
        ctx: &DriverCtx,
    ) -> MissionReport {
        let hosted: Arc<dyn ToolHost> = Arc::new(EventedTools {
            inner: tools.clone(),
            events: ctx.events.clone(),
            mission: mission.id.clone(),
        });
        let endpoint = match McpEndpoint::start(hosted).await {
            Ok(e) => e,
            Err(e) => {
                return failed(
                    format!("cannot start the MCP endpoint: {e}"),
                    Usage::default(),
                );
            }
        };
        let dir = match tempfile::Builder::new().prefix("mads-").tempdir() {
            Ok(d) => d,
            Err(e) => {
                return failed(
                    format!("cannot create a temp directory: {e}"),
                    Usage::default(),
                );
            }
        };
        let cli = CliContext {
            system: &mission.system,
            user: &mission.user,
            mcp_url: endpoint.url(),
            token: endpoint.token(),
            dir: dir.path(),
            model: self.model.as_deref(),
            max_turns: ctx.max_turns,
            api_key_set: std::env::var_os("ANTHROPIC_API_KEY").is_some_and(|v| !v.is_empty()),
            web_search: mission.web_search,
        };
        let inv = self.agent.build(&cli);
        for (name, content) in &inv.files {
            if let Err(e) = write_private(&dir.path().join(name), content) {
                return failed(format!("cannot write {name}: {e}"), Usage::default());
            }
        }

        let spawned = Command::new(&inv.program)
            .args(&inv.args)
            .envs(inv.env.iter().cloned())
            .current_dir(dir.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn();
        let mut child = match spawned {
            Ok(c) => c,
            Err(e) => {
                return failed(
                    format!("cannot start {}: {e}", inv.program),
                    Usage::default(),
                );
            }
        };
        let (Some(stdout), Some(stderr)) = (child.stdout.take(), child.stderr.take()) else {
            return failed("agent output is not piped", Usage::default());
        };
        let stderr_tail = tokio::spawn(tail_of(stderr));

        let mut lines = BufReader::new(stdout).lines();
        let (mut usage, mut failure, mut over_budget) = (Usage::default(), None, false);
        while let Ok(Some(line)) = lines.next_line().await {
            ctx.append_transcript(&mission.id, "cli.jsonl", &line);
            for output in self.agent.parse_line(&line) {
                match output {
                    CliOutput::Text(text) => ctx.events.emit(Event::AgentText {
                        mission: mission.id.clone(),
                        text,
                    }),
                    CliOutput::Failure(message) => failure = Some(message),
                    CliOutput::Usage(u) => {
                        usage.add(&u);
                        ctx.events.emit(Event::Usage {
                            mission: mission.id.clone(),
                            input_tokens: u.input_tokens,
                            output_tokens: u.output_tokens,
                            cost_usd: u.cost_usd,
                        });
                        over_budget |= ctx.budget.add(&u);
                    }
                }
            }
            if over_budget {
                break;
            }
        }
        if over_budget {
            let _ = child.kill().await;
            return MissionReport {
                outcome: MissionOutcome::BudgetExceeded,
                usage,
                turns: 0,
            };
        }
        let status = child.wait().await;
        let tail = stderr_tail.await.unwrap_or_default();
        if tools.finished() {
            return MissionReport {
                outcome: MissionOutcome::Finished,
                usage,
                turns: 0,
            };
        }
        let reason = failure.unwrap_or_else(|| match status {
            Ok(s) => format!(
                "{} exited with {s} before calling finish: {tail}",
                self.agent.name()
            ),
            Err(e) => format!("cannot wait for {}: {e}", self.agent.name()),
        });
        failed(reason, usage)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use mads_core::{
        agent::{Driver, DriverCtx, MissionOutcome, MissionSpec, TokenBudget},
        events::{Event, EventSink},
        tools::{ToolHost, ToolOutput, ToolSpec},
        usage::Usage,
    };
    use serde_json::{Value, json};

    use super::*;

    const SCRIPT: &str = r#"#!/bin/sh
CFG="$1"; MODE="$2"; PIDFILE="$3"
finish_via_mcp() {
  URL=$(sed -n 's/.*"url": *"\([^"]*\)".*/\1/p' "$CFG")
  TOKEN=$(sed -n 's/.*Bearer \([0-9a-f]*\)".*/\1/p' "$CFG")
  H1="Content-Type: application/json"; H2="Accept: application/json, text/event-stream"; A="Authorization: Bearer $TOKEN"
  INIT=$(curl -s -i -X POST "$URL" -H "$A" -H "$H1" -H "$H2" -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"fake","version":"0"}}}')
  SID=$(printf '%s' "$INIT" | tr -d '\r' | sed -n 's/^[Mm][Cc][Pp]-[Ss]ession-[Ii][Dd]: *//p')
  curl -s -X POST "$URL" -H "$A" -H "$H1" -H "$H2" -H "Mcp-Session-Id: $SID" -d '{"jsonrpc":"2.0","method":"notifications/initialized"}' >/dev/null
  curl -s -X POST "$URL" -H "$A" -H "$H1" -H "$H2" -H "Mcp-Session-Id: $SID" -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"finish","arguments":{}}}' >/dev/null
}
case "$MODE" in
  finish) finish_via_mcp; echo "done"; echo "USAGE 10 5 0.01" ;;
  perm) echo "perm:$(ls -l "$CFG" | cut -c1-10)"; finish_via_mcp ;;
  exit3) echo "boom detail" >&2; exit 3 ;;
  failline) echo "FAIL quota exceeded"; exit 1 ;;
  sleep) echo $$ > "$PIDFILE"; sleep 30 ;;
  bigusage) echo $$ > "$PIDFILE"; echo "USAGE 1000000 0 0"; sleep 30 ;;
esac
"#;

    struct FakeAgent {
        script: PathBuf,
        mode: &'static str,
        pidfile: PathBuf,
        dir_seen: std::sync::Mutex<Option<PathBuf>>,
    }

    impl CliAgent for FakeAgent {
        fn name(&self) -> &'static str {
            "fake"
        }
        fn binary(&self) -> &'static str {
            "fake"
        }
        fn binary_env(&self) -> &'static str {
            "MADS_FAKE_BIN"
        }
        fn min_version(&self) -> (u32, u32, u32) {
            (0, 0, 0)
        }
        fn build(&self, ctx: &CliContext<'_>) -> CliInvocation {
            *self.dir_seen.lock().unwrap() = Some(ctx.dir.to_path_buf());
            let cfg = json!({"mcpServers": {"mads": {"type": "http", "url": ctx.mcp_url, "headers": {"Authorization": format!("Bearer {}", ctx.token)}}}});
            CliInvocation {
                program: self.script.display().to_string(),
                args: vec![
                    ctx.dir.join("mcp.json").display().to_string(),
                    self.mode.into(),
                    self.pidfile.display().to_string(),
                ],
                env: vec![],
                files: vec![("mcp.json".into(), cfg.to_string())],
            }
        }
        fn parse_line(&self, line: &str) -> Vec<CliOutput> {
            if let Some(rest) = line.strip_prefix("USAGE ") {
                let p: Vec<&str> = rest.split(' ').collect();
                let usage = Usage {
                    input_tokens: p[0].parse().unwrap(),
                    output_tokens: p[1].parse().unwrap(),
                    cost_usd: p[2].parse().ok(),
                };
                return vec![CliOutput::Usage(usage)];
            }
            if let Some(rest) = line.strip_prefix("FAIL ") {
                return vec![CliOutput::Failure(rest.into())];
            }
            vec![CliOutput::Text(line.into())]
        }
    }

    struct FakeTools {
        finished: AtomicBool,
    }

    #[async_trait]
    impl ToolHost for FakeTools {
        fn specs(&self) -> Vec<ToolSpec> {
            vec![ToolSpec {
                name: "finish".into(),
                description: "Finish".into(),
                input_schema: json!({"type": "object"}),
            }]
        }
        async fn call(&self, name: &str, _: Value) -> ToolOutput {
            if name == "finish" {
                self.finished.store(true, Ordering::SeqCst);
                return ToolOutput::ok(json!({}), &[], "finished");
            }
            ToolOutput::fail("UNKNOWN_TOOL", name)
        }
        fn finished(&self) -> bool {
            self.finished.load(Ordering::SeqCst)
        }
    }

    struct Harness {
        driver: CliDriver<FakeAgent>,
        tools: Arc<FakeTools>,
        rx: tokio::sync::mpsc::UnboundedReceiver<mads_core::events::EventEnvelope>,
        ctx: DriverCtx,
        _dir: tempfile::TempDir,
        pidfile: PathBuf,
        transcripts: PathBuf,
    }

    fn harness(mode: &'static str, max_tokens: u64) -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("fake-agent.sh");
        std::fs::write(&script, SCRIPT).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let pidfile = dir.path().join("pid");
        let transcripts = dir.path().join("transcripts");
        std::fs::create_dir_all(&transcripts).unwrap();
        let agent = FakeAgent {
            script,
            mode,
            pidfile: pidfile.clone(),
            dir_seen: std::sync::Mutex::new(None),
        };
        let (events, rx) = EventSink::channel();
        let ctx = DriverCtx {
            events,
            max_turns: 10,
            budget: Arc::new(TokenBudget::new(max_tokens)),
            transcripts: Some(transcripts.clone()),
        };
        Harness {
            driver: CliDriver::new(agent),
            tools: Arc::new(FakeTools {
                finished: AtomicBool::new(false),
            }),
            rx,
            ctx,
            _dir: dir,
            pidfile,
            transcripts,
        }
    }

    fn mission() -> MissionSpec {
        MissionSpec {
            id: "campaign:x".into(),
            system: "sys".into(),
            user: "go".into(),
            web_search: false,
        }
    }

    #[test]
    fn agent_clis_offer_web_search() {
        assert!(CliDriver::new(Claude).web_search());
        assert!(CliDriver::new(Codex).web_search());
        assert!(CliDriver::new(Gemini).web_search());
    }

    fn drain(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<mads_core::events::EventEnvelope>,
    ) -> Vec<Event> {
        let mut out = Vec::new();
        while let Ok(e) = rx.try_recv() {
            out.push(e.event);
        }
        out
    }

    /// A killed process that nobody has reaped yet is a zombie: it answers `kill -0` but is dead.
    fn alive(pid: &str) -> bool {
        let out = std::process::Command::new("ps")
            .args(["-o", "stat=", "-p", pid])
            .output();
        out.map(|o| {
            let stat = String::from_utf8_lossy(&o.stdout).trim().to_string();
            !stat.is_empty() && !stat.starts_with('Z')
        })
        .unwrap_or(false)
    }

    async fn pid_of(path: &Path) -> String {
        for _ in 0..50 {
            if let Ok(p) = std::fs::read_to_string(path)
                && !p.trim().is_empty()
            {
                return p.trim().to_string();
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("agent never wrote its pid");
    }

    #[tokio::test]
    async fn an_agent_that_calls_finish_over_mcp_completes_the_mission() {
        let mut h = harness("finish", 0);
        let report = h
            .driver
            .run_mission(&mission(), h.tools.clone(), &h.ctx)
            .await;
        assert_eq!(
            report.outcome,
            MissionOutcome::Finished,
            "{:?}",
            drain(&mut h.rx)
        );
        assert!(h.tools.finished());
        assert_eq!(
            (report.usage.input_tokens, report.usage.output_tokens),
            (10, 5)
        );
        let events = drain(&mut h.rx);
        assert!(events.iter().any(|e| matches!(e, Event::ToolCalled { tool, mission, .. } if tool == "finish" && mission == "campaign:x")), "{events:?}");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::ToolFinished { ok: true, .. }))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::AgentText { text, .. } if text == "done"))
        );
        assert!(events.iter().any(|e| matches!(
            e,
            Event::Usage {
                input_tokens: 10,
                ..
            }
        )));
    }

    #[tokio::test]
    async fn the_raw_cli_stream_is_kept_as_a_transcript() {
        let h = harness("finish", 0);
        h.driver
            .run_mission(&mission(), h.tools.clone(), &h.ctx)
            .await;
        let text = std::fs::read_to_string(h.transcripts.join("campaign-x.cli.jsonl")).unwrap();
        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            ["done", "USAGE 10 5 0.01"]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn config_files_are_private_and_removed_after_the_mission() {
        let mut h = harness("perm", 0);
        let report = h
            .driver
            .run_mission(&mission(), h.tools.clone(), &h.ctx)
            .await;
        assert_eq!(report.outcome, MissionOutcome::Finished);
        let events = drain(&mut h.rx);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::AgentText { text, .. } if text == "perm:-rw-------")),
            "{events:?}"
        );
        let dir = h.driver_dir();
        assert!(
            !dir.exists(),
            "temp dir with the token must be deleted: {}",
            dir.display()
        );
    }

    #[tokio::test]
    async fn the_endpoint_is_closed_when_the_mission_ends() {
        let h = harness("finish", 0);
        h.driver
            .run_mission(&mission(), h.tools.clone(), &h.ctx)
            .await;
        let cfg = h.driver_dir();
        assert!(!cfg.exists());
    }

    #[tokio::test]
    async fn an_agent_that_exits_without_finishing_fails_with_its_stderr() {
        let h = harness("exit3", 0);
        let report = h
            .driver
            .run_mission(&mission(), h.tools.clone(), &h.ctx)
            .await;
        match report.outcome {
            MissionOutcome::Failed(m) => assert!(
                m.contains("exit") && m.contains('3') && m.contains("boom detail"),
                "{m}"
            ),
            other => panic!("{other:?}"),
        }
        assert!(!h.tools.finished());
    }

    #[tokio::test]
    async fn a_reported_failure_wins_over_the_exit_code() {
        let h = harness("failline", 0);
        let report = h
            .driver
            .run_mission(&mission(), h.tools.clone(), &h.ctx)
            .await;
        assert_eq!(
            report.outcome,
            MissionOutcome::Failed("quota exceeded".into())
        );
    }

    #[tokio::test]
    async fn dropping_the_mission_kills_the_agent_process() {
        let h = harness("sleep", 0);
        let m = mission();
        let run = h.driver.run_mission(&m, h.tools.clone(), &h.ctx);
        let timed = tokio::time::timeout(Duration::from_millis(400), run).await;
        assert!(timed.is_err(), "the mission should still be running");
        let pid = pid_of(&h.pidfile).await;
        for _ in 0..200 {
            if !alive(&pid) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("agent process {pid} is still alive after the mission was dropped");
    }

    #[tokio::test]
    async fn exceeding_the_token_budget_kills_the_agent() {
        let h = harness("bigusage", 100);
        let report = h
            .driver
            .run_mission(&mission(), h.tools.clone(), &h.ctx)
            .await;
        assert_eq!(report.outcome, MissionOutcome::BudgetExceeded);
        let pid = pid_of(&h.pidfile).await;
        for _ in 0..200 {
            if !alive(&pid) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("agent process {pid} survived the budget abort");
    }

    #[test]
    fn versions_are_read_from_real_cli_banners() {
        assert_eq!(parse_version("2.1.285 (Claude Code)"), Some((2, 1, 285)));
        assert_eq!(parse_version("codex-cli 0.46.0"), Some((0, 46, 0)));
        assert_eq!(parse_version("0.8.1\n"), Some((0, 8, 1)));
        assert_eq!(parse_version("v1.2.3-beta"), Some((1, 2, 3)));
        assert_eq!(parse_version("no version here"), None);
        assert_eq!(parse_version(""), None);
    }

    #[tokio::test]
    async fn check_cli_rejects_old_missing_and_unreadable_binaries() {
        let dir = tempfile::tempdir().unwrap();
        let make = |body: &str| {
            let p = dir.path().join(format!("bin-{}", body.len()));
            std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            p.display().to_string()
        };
        assert!(
            check_cli(&make("echo '2.1.285 (x)'"), "claude", (2, 1, 259))
                .await
                .is_ok()
        );
        let old = check_cli(&make("echo '2.0.1 (x)'"), "claude", (2, 1, 259))
            .await
            .unwrap_err();
        assert!(old.contains("2.0.1") && old.contains("2.1.259"), "{old}");
        let nover = check_cli(&make("echo hello"), "claude", (2, 1, 259))
            .await
            .unwrap_err();
        assert!(nover.contains("version"), "{nover}");
        let missing = check_cli("/nonexistent/claude-bin", "claude", (2, 1, 259))
            .await
            .unwrap_err();
        assert!(missing.contains("not found"), "{missing}");
    }

    impl Harness {
        fn driver_dir(&self) -> PathBuf {
            self.driver
                .agent()
                .dir_seen
                .lock()
                .unwrap()
                .clone()
                .expect("the agent was built")
        }
    }
}
