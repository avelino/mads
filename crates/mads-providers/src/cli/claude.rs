use mads_core::usage::Usage;
use serde_json::{Value, json};

use super::{CliAgent, CliContext, CliInvocation, CliOutput, program_for};

/// Claude Code in print mode. Isolation is what keeps a mission at hundreds of tokens instead of
/// the 150k a developer's full config can add: no user settings, skills, plugins or other MCP servers.
pub struct Claude;

impl CliAgent for Claude {
    fn name(&self) -> &'static str {
        "claude"
    }

    fn binary(&self) -> &'static str {
        "claude"
    }

    fn binary_env(&self) -> &'static str {
        "MADS_CLAUDE_BIN"
    }

    /// `--permission-prompts` first shipped in 2.1.259.
    fn min_version(&self) -> (u32, u32, u32) {
        (2, 1, 259)
    }

    fn build(&self, c: &CliContext<'_>) -> CliInvocation {
        let path = |name: &str| c.dir.join(name).display().to_string();
        let (builtin, allowed) = if c.web_search {
            ("WebSearch,WebFetch", "mcp__mads__* WebSearch WebFetch")
        } else {
            ("", "mcp__mads__*")
        };
        let mut args: Vec<String> = [
            "-p",
            c.user,
            "--system-prompt-file",
            &path("system.md"),
            "--mcp-config",
            &path("mcp.json"),
            "--strict-mcp-config",
            "--setting-sources",
            "project",
            "--disable-slash-commands",
            "--tools",
            builtin,
            "--allowedTools",
            allowed,
            "--permission-prompts",
            "none",
            "--output-format",
            "stream-json",
            "--verbose",
            "--max-turns",
            &c.max_turns.to_string(),
            "--no-session-persistence",
        ]
        .map(String::from)
        .into();
        if c.api_key_set {
            args.push("--bare".into());
        }
        if let Some(model) = c.model {
            args.extend(["--model".into(), model.into()]);
        }
        let mcp = json!({"mcpServers": {"mads": {
            "type": "http",
            "url": c.mcp_url,
            "headers": {"Authorization": format!("Bearer {}", c.token)},
        }}});
        CliInvocation {
            program: program_for(self),
            args,
            // Claude defers MCP tools behind ToolSearch, which `--tools ""` removes: load them upfront.
            env: vec![("ENABLE_TOOL_SEARCH".into(), "false".into())],
            files: vec![
                ("system.md".into(), c.system.into()),
                ("mcp.json".into(), mcp.to_string()),
            ],
        }
    }

    fn parse_line(&self, line: &str) -> Vec<CliOutput> {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            return Vec::new();
        };
        match v["type"].as_str() {
            Some("assistant") => v["message"]["content"]
                .as_array()
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter(|b| b["type"] == "text")
                        .filter_map(|b| b["text"].as_str())
                        .map(|t| CliOutput::Text(t.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            Some("result") => result_outputs(&v),
            _ => Vec::new(),
        }
    }
}

/// The final `result` line has the run's aggregated usage and cost. Cache reads and writes count as input.
fn result_outputs(v: &Value) -> Vec<CliOutput> {
    let n = |key: &str| v["usage"][key].as_u64().unwrap_or(0);
    let usage = Usage {
        input_tokens: n("input_tokens")
            + n("cache_creation_input_tokens")
            + n("cache_read_input_tokens"),
        output_tokens: n("output_tokens"),
        cost_usd: v["total_cost_usd"].as_f64(),
    };
    let mut out = Vec::new();
    if usage.total_tokens() > 0 || usage.cost_usd.is_some_and(|c| c > 0.0) {
        out.push(CliOutput::Usage(usage));
    }
    if v["is_error"] == true {
        let reason = v["result"]
            .as_str()
            .filter(|r| !r.is_empty())
            .map(String::from)
            .or_else(|| {
                let errors: Vec<&str> = v["errors"]
                    .as_array()?
                    .iter()
                    .filter_map(Value::as_str)
                    .collect();
                (!errors.is_empty()).then(|| errors.join("; "))
            })
            .unwrap_or_else(|| "claude reported an error".to_string());
        out.push(CliOutput::Failure(reason));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::Value;

    use super::*;

    fn ctx<'a>(api_key: bool, model: Option<&'a str>) -> CliContext<'a> {
        CliContext {
            system: "system prompt",
            user: "user prompt",
            mcp_url: "http://127.0.0.1:4242/mcp",
            token: "tok123",
            dir: Path::new("/work"),
            model,
            max_turns: 40,
            api_key_set: api_key,
            web_search: false,
        }
    }

    fn pairs(args: &[String], flag: &str) -> Option<String> {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    }

    #[test]
    fn command_line_isolates_the_agent_and_exposes_only_the_mads_tools() {
        let inv = Claude.build(&ctx(false, None));
        assert_eq!(inv.program, "claude");
        assert_eq!(inv.args[..2], ["-p".to_string(), "user prompt".to_string()]);
        assert_eq!(
            pairs(&inv.args, "--system-prompt-file").as_deref(),
            Some("/work/system.md")
        );
        assert_eq!(
            pairs(&inv.args, "--mcp-config").as_deref(),
            Some("/work/mcp.json")
        );
        assert_eq!(
            pairs(&inv.args, "--tools").as_deref(),
            Some(""),
            "built-in tools are off"
        );
        assert_eq!(
            pairs(&inv.args, "--allowedTools").as_deref(),
            Some("mcp__mads__*")
        );
        assert_eq!(
            pairs(&inv.args, "--setting-sources").as_deref(),
            Some("project")
        );
        assert_eq!(
            pairs(&inv.args, "--permission-prompts").as_deref(),
            Some("none")
        );
        assert_eq!(
            pairs(&inv.args, "--output-format").as_deref(),
            Some("stream-json")
        );
        assert_eq!(pairs(&inv.args, "--max-turns").as_deref(), Some("40"));
        for flag in [
            "--strict-mcp-config",
            "--disable-slash-commands",
            "--verbose",
            "--no-session-persistence",
        ] {
            assert!(inv.args.iter().any(|a| a == flag), "{flag}");
        }
        assert!(
            !inv.args
                .iter()
                .any(|a| a == "--bare" || a == "--dangerously-skip-permissions")
        );
        assert_eq!(
            inv.env,
            vec![("ENABLE_TOOL_SEARCH".to_string(), "false".to_string())],
            "MCP tools must load upfront"
        );
    }

    #[test]
    fn web_search_opens_the_search_tools_and_nothing_else() {
        let mut c = ctx(false, None);
        c.web_search = true;
        let inv = Claude.build(&c);
        assert_eq!(
            pairs(&inv.args, "--tools").as_deref(),
            Some("WebSearch,WebFetch")
        );
        assert_eq!(
            pairs(&inv.args, "--allowedTools").as_deref(),
            Some("mcp__mads__* WebSearch WebFetch")
        );
        for flag in ["--strict-mcp-config", "--disable-slash-commands"] {
            assert!(
                inv.args.iter().any(|a| a == flag),
                "isolation stays: {flag}"
            );
        }
    }

    #[test]
    fn bare_mode_is_added_when_an_api_key_is_present() {
        assert!(
            Claude
                .build(&ctx(true, None))
                .args
                .iter()
                .any(|a| a == "--bare")
        );
    }

    #[test]
    fn model_is_passed_through() {
        let inv = Claude.build(&ctx(false, Some("claude-sonnet-5-5")));
        assert_eq!(
            pairs(&inv.args, "--model").as_deref(),
            Some("claude-sonnet-5-5")
        );
        assert!(
            !Claude
                .build(&ctx(false, None))
                .args
                .iter()
                .any(|a| a == "--model")
        );
    }

    #[test]
    fn config_files_hold_the_prompt_and_the_authenticated_endpoint() {
        let inv = Claude.build(&ctx(false, None));
        let system = inv.files.iter().find(|(n, _)| n == "system.md").unwrap();
        assert_eq!(system.1, "system prompt");
        let mcp = inv.files.iter().find(|(n, _)| n == "mcp.json").unwrap();
        let v: Value = serde_json::from_str(&mcp.1).unwrap();
        assert_eq!(v["mcpServers"]["mads"]["type"], "http");
        assert_eq!(v["mcpServers"]["mads"]["url"], "http://127.0.0.1:4242/mcp");
        assert_eq!(
            v["mcpServers"]["mads"]["headers"]["Authorization"],
            "Bearer tok123"
        );
    }

    #[test]
    fn the_token_never_appears_on_the_command_line() {
        let inv = Claude.build(&ctx(false, None));
        assert!(
            !inv.args.iter().any(|a| a.contains("tok123")),
            "tokens in argv leak through `ps`"
        );
    }

    fn fixture(name: &str) -> Vec<String> {
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
        )
        .unwrap()
        .lines()
        .map(String::from)
        .collect()
    }

    #[test]
    fn a_real_successful_run_yields_text_and_aggregated_usage() {
        let out: Vec<CliOutput> = fixture("claude-success.jsonl")
            .iter()
            .flat_map(|l| Claude.parse_line(l))
            .collect();
        assert_eq!(out.len(), 2, "{out:?}");
        assert!(matches!(&out[0], CliOutput::Text(t) if t == "ok"));
        match &out[1] {
            CliOutput::Usage(u) => {
                assert_eq!((u.input_tokens, u.output_tokens), (489, 4));
                assert!((u.cost_usd.unwrap() - 0.001018).abs() < 1e-9);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn cache_tokens_count_as_input() {
        let line = r#"{"type":"result","is_error":false,"total_cost_usd":0.5,"usage":{"input_tokens":2,"cache_creation_input_tokens":1526,"cache_read_input_tokens":535,"output_tokens":4}}"#;
        match &Claude.parse_line(line)[..] {
            [CliOutput::Usage(u)] => {
                assert_eq!((u.input_tokens, u.output_tokens), (2 + 1526 + 535, 4))
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_real_budget_error_becomes_a_failure_with_the_reason() {
        let out: Vec<CliOutput> = fixture("claude-budget-error.jsonl")
            .iter()
            .flat_map(|l| Claude.parse_line(l))
            .collect();
        assert!(
            out.iter().any(
                |o| matches!(o, CliOutput::Failure(m) if m.contains("Reached maximum budget"))
            ),
            "{out:?}"
        );
    }

    #[test]
    fn login_errors_surface_the_message() {
        let line = r#"{"type":"result","subtype":"success","is_error":true,"result":"Not logged in · Please run /login","usage":{}}"#;
        assert!(
            Claude
                .parse_line(line)
                .iter()
                .any(|o| matches!(o, CliOutput::Failure(m) if m.contains("Not logged in")))
        );
    }

    #[test]
    fn unknown_and_malformed_lines_are_ignored() {
        for line in [
            "",
            "not json",
            r#"{"type":"system","subtype":"init"}"#,
            r#"{"type":"user"}"#,
            r#"{"type":"stream_event"}"#,
        ] {
            assert!(Claude.parse_line(line).is_empty(), "{line}");
        }
    }

    #[test]
    fn tool_use_blocks_are_not_text() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"mcp__mads__finish","input":{}},{"type":"text","text":"calling"}]}}"#;
        assert_eq!(Claude.parse_line(line).len(), 1);
    }

    #[test]
    fn minimum_version_is_the_one_that_has_permission_prompts() {
        assert_eq!(Claude.min_version(), (2, 1, 259));
        assert_eq!(Claude.binary_env(), "MADS_CLAUDE_BIN");
    }
}
