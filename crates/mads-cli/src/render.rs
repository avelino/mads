use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::Write,
    path::PathBuf,
    time::Duration,
};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use mads_core::{
    events::{Event, EventEnvelope},
    google::Issue,
};
use tokio::sync::mpsc::UnboundedReceiver;

const TEXT_LIMIT: usize = 200;
const ISSUE_LIMIT: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    Auto,
    Pretty,
    Plain,
    Json,
    Github,
}

pub fn resolve(format: Format, github_actions: bool, is_terminal: bool) -> Format {
    match format {
        Format::Auto if github_actions => Format::Github,
        Format::Auto if is_terminal => Format::Pretty,
        Format::Auto => Format::Plain,
        explicit => explicit,
    }
}

fn clock(ts: &str) -> &str {
    ts.get(11..19).unwrap_or(ts)
}

fn one_line(text: &str, limit: usize) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= limit {
        return flat;
    }
    let cut: String = flat.chars().take(limit).collect();
    format!("{cut}…")
}

fn cost(c: Option<f64>) -> String {
    c.map_or("n/a".to_string(), |v| format!("${v:.4}"))
}

fn issue_lines(t: &str, kind: &str, issues: &[Issue]) -> Vec<String> {
    let mut lines: Vec<String> = issues
        .iter()
        .take(ISSUE_LIMIT)
        .map(|i| format!("{t} [validate] {} {}: {}", i.code, i.path, i.message))
        .collect();
    if issues.len() > ISSUE_LIMIT {
        lines.push(format!(
            "{t} [validate] and {} more {kind}",
            issues.len() - ISSUE_LIMIT
        ));
    }
    lines
}

pub fn plain_lines(env: &EventEnvelope) -> Vec<String> {
    let t = clock(&env.ts);
    match &env.event {
        Event::RunStarted {
            run_id,
            provider,
            model,
            ..
        } => {
            let model = model.as_deref().unwrap_or("default model");
            vec![format!("{t} [run] {run_id} started ({provider}, {model})")]
        }
        Event::MissionStarted { mission, attempt } => {
            vec![format!("{t} [{mission}] started (attempt {attempt})")]
        }
        Event::AgentText { mission, text } => {
            vec![format!("{t} [{mission}] {}", one_line(text, TEXT_LIMIT))]
        }
        Event::ToolCalled {
            mission, summary, ..
        } => vec![format!("{t} [{mission}] > {summary}")],
        Event::ToolFinished {
            mission,
            ok,
            summary,
            ..
        } => {
            vec![format!(
                "{t} [{mission}] < {} {summary}",
                if *ok { "ok" } else { "error" }
            )]
        }
        Event::Usage {
            mission,
            input_tokens,
            output_tokens,
            cost_usd,
        } => {
            let extra = cost_usd.map(|c| format!(", ${c:.4}")).unwrap_or_default();
            vec![format!(
                "{t} [{mission}] tokens {input_tokens} in, {output_tokens} out{extra}"
            )]
        }
        Event::MissionFinished {
            mission, ok: true, ..
        } => vec![format!("{t} [{mission}] finished")],
        Event::MissionFinished {
            mission, reason, ..
        } => {
            vec![format!(
                "{t} [{mission}] failed: {}",
                reason.as_deref().unwrap_or("unknown")
            )]
        }
        Event::Step { name, detail } => vec![format!("{t} [run] {name}: {detail}")],
        Event::Validation { errors, warnings } => {
            let mut lines = vec![format!(
                "{t} [validate] {} errors, {} warnings",
                errors.len(),
                warnings.len()
            )];
            lines.extend(issue_lines(t, "errors", errors));
            lines.extend(issue_lines(t, "warnings", warnings));
            lines
        }
        Event::UrlChecked { url, status, .. } => {
            let status = status.map_or("error".to_string(), |s| s.to_string());
            vec![format!("{t} [url] {status} {url}")]
        }
        Event::ArtifactWritten { path } => vec![format!("{t} [out] {path}")],
        Event::RunFinished {
            ok,
            exit_code,
            totals,
        } => {
            let missions = if totals.failed_missions > 0 {
                format!(
                    "{} of {} missions failed",
                    totals.failed_missions, totals.missions
                )
            } else {
                format!("{} missions finished", totals.missions)
            };
            vec![format!(
                "{t} [run] {} (exit {exit_code}): {} in, {} out tokens, cost {}, {missions}",
                if *ok { "done" } else { "failed" },
                totals.input_tokens,
                totals.output_tokens,
                cost(totals.cost_usd)
            )]
        }
    }
}

fn escape(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

pub fn github_lines(env: &EventEnvelope) -> Vec<String> {
    let mut lines = plain_lines(env);
    if let Event::Validation { errors, warnings } = &env.event {
        for (kind, list) in [("error", errors), ("warning", warnings)] {
            for i in list {
                lines.push(format!(
                    "::{kind} title={}::{}",
                    i.code,
                    escape(&format!("{}: {}", i.path, i.message))
                ));
            }
        }
    }
    lines
}

pub fn json_line(env: &EventEnvelope) -> String {
    serde_json::to_string(env).unwrap_or_default()
}

struct Pretty {
    multi: MultiProgress,
    bars: HashMap<String, ProgressBar>,
}

impl Pretty {
    fn new() -> Self {
        Self {
            multi: MultiProgress::new(),
            bars: HashMap::new(),
        }
    }

    fn handle(&mut self, env: &EventEnvelope) {
        match &env.event {
            Event::MissionStarted { mission, .. } => {
                let bar = self.multi.add(ProgressBar::new_spinner());
                bar.set_style(
                    ProgressStyle::with_template("{spinner} {prefix:<32} {msg}")
                        .unwrap_or_else(|_| ProgressStyle::default_spinner()),
                );
                bar.set_prefix(mission.clone());
                bar.enable_steady_tick(Duration::from_millis(100));
                self.bars.insert(mission.clone(), bar);
            }
            Event::ToolCalled {
                mission, summary, ..
            } => self.message(mission, summary.clone()),
            Event::ToolFinished {
                mission,
                ok,
                summary,
                ..
            } => {
                self.message(
                    mission,
                    format!("{} {summary}", if *ok { "ok" } else { "error" }),
                );
            }
            Event::MissionFinished {
                mission,
                ok,
                reason,
            } => {
                let line = if *ok {
                    "finished".to_string()
                } else {
                    format!("failed: {}", reason.as_deref().unwrap_or("unknown"))
                };
                match self.bars.get(mission) {
                    Some(bar) if *ok => bar.finish_with_message(line),
                    Some(bar) => bar.abandon_with_message(line),
                    None => {}
                }
            }
            Event::AgentText { .. } | Event::Usage { .. } => {}
            _ => plain_lines(env).iter().for_each(|l| {
                let _ = self.multi.println(l);
            }),
        }
    }

    fn message(&self, mission: &str, msg: String) {
        if let Some(bar) = self.bars.get(mission) {
            bar.set_message(msg);
        }
    }
}

/// Prints events in the chosen format and records every one of them in `<run-dir>/events.ndjson`.
pub struct Sink {
    format: Format,
    ndjson: Option<File>,
    run_dir: Option<PathBuf>,
    pretty: Option<Pretty>,
}

impl Sink {
    pub fn new(format: Format) -> Self {
        let pretty = (format == Format::Pretty).then(Pretty::new);
        Self {
            format,
            ndjson: None,
            run_dir: None,
            pretty,
        }
    }

    fn print(lines: &[String]) {
        let mut out = std::io::stdout().lock();
        for l in lines {
            let _ = writeln!(out, "{l}");
        }
    }

    pub fn handle(&mut self, env: &EventEnvelope) {
        // `init` has no run directory: it writes only business.toml, catalog.csv and research.md.
        if let Event::RunStarted { run_dir, .. } = &env.event
            && !run_dir.is_empty()
        {
            let dir = PathBuf::from(run_dir);
            self.ndjson = OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join("events.ndjson"))
                .ok();
            self.run_dir = Some(dir);
        }
        if let Some(f) = self.ndjson.as_mut() {
            let _ = writeln!(f, "{}", json_line(env));
        }
        match self.format {
            Format::Json => Self::print(&[json_line(env)]),
            Format::Plain | Format::Auto => Self::print(&plain_lines(env)),
            Format::Github => {
                Self::print(&github_lines(env));
                if matches!(env.event, Event::RunFinished { .. }) {
                    self.append_step_summary();
                }
            }
            Format::Pretty => {
                if let Some(p) = self.pretty.as_mut() {
                    p.handle(env);
                }
            }
        }
    }

    fn append_step_summary(&self) {
        let (Some(dir), Some(target)) = (&self.run_dir, std::env::var_os("GITHUB_STEP_SUMMARY"))
        else {
            return;
        };
        let Ok(report) = std::fs::read_to_string(dir.join("report.md")) else {
            return;
        };
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(target) {
            let _ = writeln!(f, "{report}");
        }
    }
}

pub async fn pump(mut rx: UnboundedReceiver<EventEnvelope>, format: Format) {
    let mut sink = Sink::new(format);
    while let Some(env) = rx.recv().await {
        sink.handle(&env);
    }
}

#[cfg(test)]
mod tests {
    use mads_core::{
        events::{Event, EventEnvelope, Totals},
        google::Issue,
    };

    use super::*;

    fn env(event: Event) -> EventEnvelope {
        EventEnvelope {
            seq: 1,
            ts: "2026-10-01T10:12:13.456Z".into(),
            event,
        }
    }

    #[test]
    fn auto_picks_github_then_pretty_then_plain() {
        assert_eq!(resolve(Format::Auto, true, true), Format::Github);
        assert_eq!(resolve(Format::Auto, false, true), Format::Pretty);
        assert_eq!(resolve(Format::Auto, false, false), Format::Plain);
        assert_eq!(
            resolve(Format::Json, true, true),
            Format::Json,
            "explicit format wins"
        );
        assert_eq!(resolve(Format::Plain, true, true), Format::Plain);
    }

    #[test]
    fn plain_lines_start_with_the_clock_time() {
        let lines = plain_lines(&env(Event::ToolCalled {
            mission: "plan".into(),
            tool: "get_business".into(),
            summary: "get_business".into(),
        }));
        assert_eq!(lines, ["10:12:13 [plan] > get_business"]);
    }

    #[test]
    fn plain_covers_the_mission_lifecycle() {
        let line = |e| plain_lines(&env(e)).join("|");
        assert_eq!(
            line(Event::MissionStarted {
                mission: "plan".into(),
                attempt: 2
            }),
            "10:12:13 [plan] started (attempt 2)"
        );
        assert_eq!(
            line(Event::MissionFinished {
                mission: "plan".into(),
                ok: true,
                reason: None
            }),
            "10:12:13 [plan] finished"
        );
        assert_eq!(
            line(Event::MissionFinished {
                mission: "campaign:x".into(),
                ok: false,
                reason: Some("max turns".into())
            }),
            "10:12:13 [campaign:x] failed: max turns"
        );
        assert_eq!(
            line(Event::ToolFinished {
                mission: "plan".into(),
                tool: "finish".into(),
                ok: false,
                summary: "finish: plan incomplete".into()
            }),
            "10:12:13 [plan] < error finish: plan incomplete"
        );
        assert_eq!(
            line(Event::Usage {
                mission: "plan".into(),
                input_tokens: 1200,
                output_tokens: 300,
                cost_usd: None
            }),
            "10:12:13 [plan] tokens 1200 in, 300 out"
        );
        assert!(
            line(Event::Usage {
                mission: "plan".into(),
                input_tokens: 1,
                output_tokens: 2,
                cost_usd: Some(0.5)
            })
            .ends_with("$0.5000")
        );
    }

    #[test]
    fn agent_text_is_one_line_and_truncated() {
        let long = format!("first line\nsecond {}", "x".repeat(300));
        let lines = plain_lines(&env(Event::AgentText {
            mission: "plan".into(),
            text: long,
        }));
        assert_eq!(lines.len(), 1);
        assert!(!lines[0].contains('\n'));
        assert!(
            lines[0].chars().count() <= "10:12:13 [plan] ".chars().count() + 200 + 1,
            "{}",
            lines[0].chars().count()
        );
        assert!(lines[0].ends_with('…'));
    }

    #[test]
    fn validation_prints_a_summary_and_each_issue_up_to_a_cap() {
        let errors: Vec<Issue> = (0..25)
            .map(|i| Issue::error("E01", format!("p[{i}]"), "too long"))
            .collect();
        let lines = plain_lines(&env(Event::Validation {
            errors,
            warnings: vec![Issue::warning("W01", "w", "third party")],
        }));
        assert_eq!(lines[0], "10:12:13 [validate] 25 errors, 1 warnings");
        assert!(lines.iter().any(|l| l.contains("E01 p[0]: too long")));
        assert!(lines.iter().any(|l| l.contains("and 5 more errors")));
        assert!(lines.iter().any(|l| l.contains("W01 w: third party")));
    }

    #[test]
    fn url_and_run_events() {
        let line = |e| plain_lines(&env(e)).join("|");
        assert_eq!(
            line(Event::UrlChecked {
                url: "https://a.com".into(),
                status: Some(404),
                ok: false
            }),
            "10:12:13 [url] 404 https://a.com"
        );
        assert_eq!(
            line(Event::UrlChecked {
                url: "https://a.com".into(),
                status: None,
                ok: false
            }),
            "10:12:13 [url] error https://a.com"
        );
        assert_eq!(
            line(Event::ArtifactWritten {
                path: "out/x.csv".into()
            }),
            "10:12:13 [out] out/x.csv"
        );
        let done = line(Event::RunFinished {
            ok: false,
            exit_code: 3,
            totals: Totals {
                input_tokens: 10,
                output_tokens: 5,
                cost_usd: None,
                missions: 3,
                failed_missions: 1,
            },
        });
        assert!(
            done.contains("failed")
                && done.contains("exit 3")
                && done.contains("10 in")
                && done.contains("1 of 3 missions failed"),
            "{done}"
        );
    }

    #[test]
    fn github_adds_annotations_for_validation_issues() {
        let e = env(Event::Validation {
            errors: vec![Issue::error("E01", "campaigns[0].h", "31 chars,\nlimit 30")],
            warnings: vec![Issue::warning("W01", "w", "100% third party")],
        });
        let lines = github_lines(&e);
        assert!(
            lines.contains(&"::error title=E01::campaigns[0].h: 31 chars,%0Alimit 30".to_string()),
            "{lines:?}"
        );
        assert!(
            lines.contains(&"::warning title=W01::w: 100%25 third party".to_string()),
            "{lines:?}"
        );
        assert!(
            lines[0].starts_with("10:12:13 [validate]"),
            "plain lines still come first"
        );
    }

    #[test]
    fn github_for_other_events_is_just_plain() {
        let e = env(Event::Step {
            name: "export".into(),
            detail: "writing".into(),
        });
        assert_eq!(github_lines(&e), plain_lines(&e));
    }

    #[test]
    fn json_is_the_envelope_on_one_line() {
        let e = env(Event::Step {
            name: "a".into(),
            detail: String::new(),
        });
        let line = json_line(&e);
        assert!(!line.contains('\n'));
        let back: EventEnvelope = serde_json::from_str(&line).unwrap();
        assert_eq!(back, e);
    }
}
