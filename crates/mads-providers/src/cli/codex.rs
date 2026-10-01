use mads_core::usage::Usage;
use serde_json::Value;

use super::{CliAgent, CliContext, CliInvocation, CliOutput, program_for};

const TOKEN_ENV: &str = "MADS_MCP_TOKEN";

/// `codex exec` with the mads tools as its only MCP server. The bearer token travels in the
/// environment, never in the command line.
pub struct Codex;

impl CliAgent for Codex {
    fn name(&self) -> &'static str {
        "codex"
    }

    fn binary(&self) -> &'static str {
        "codex"
    }

    fn binary_env(&self) -> &'static str {
        "MADS_CODEX_BIN"
    }

    fn min_version(&self) -> (u32, u32, u32) {
        (0, 0, 0)
    }

    fn build(&self, c: &CliContext<'_>) -> CliInvocation {
        let overrides = [
            format!(
                "model_instructions_file=\"{}\"",
                c.dir.join("system.md").display()
            ),
            format!("mcp_servers.mads.url=\"{}\"", c.mcp_url),
            format!("mcp_servers.mads.bearer_token_env_var=\"{TOKEN_ENV}\""),
            "mcp_servers.mads.required=true".to_string(),
            "features.shell_tool=false".to_string(),
            "features.multi_agent=false".to_string(),
            format!(
                "web_search=\"{}\"",
                if c.web_search { "live" } else { "disabled" }
            ),
        ];
        let mut args: Vec<String> = [
            "exec",
            c.user,
            "--json",
            "--ephemeral",
            "--skip-git-repo-check",
            "--ignore-user-config",
            "--sandbox",
            "read-only",
        ]
        .map(String::from)
        .into();
        for o in overrides {
            args.extend(["-c".into(), o]);
        }
        if let Some(model) = c.model {
            args.extend(["--model".into(), model.into()]);
        }
        CliInvocation {
            program: program_for(self),
            args,
            env: vec![(TOKEN_ENV.to_string(), c.token.to_string())],
            files: vec![("system.md".into(), c.system.into())],
        }
    }

    fn parse_line(&self, line: &str) -> Vec<CliOutput> {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            return Vec::new();
        };
        match v["type"].as_str() {
            Some("item.completed") => item_output(&v["item"]),
            Some("turn.completed") => {
                let n = |key: &str| v["usage"][key].as_u64().unwrap_or(0);
                let usage = Usage {
                    input_tokens: n("input_tokens"),
                    output_tokens: n("output_tokens"),
                    cost_usd: None,
                };
                vec![CliOutput::Usage(usage)]
            }
            Some("turn.failed") => failure(&v["error"]["message"]),
            Some("error") => failure(&v["message"]),
            _ => Vec::new(),
        }
    }
}

fn failure(message: &Value) -> Vec<CliOutput> {
    vec![CliOutput::Failure(
        message
            .as_str()
            .unwrap_or("codex reported an error")
            .to_string(),
    )]
}

fn item_output(item: &Value) -> Vec<CliOutput> {
    match item["type"].as_str() {
        Some("agent_message") => item["text"]
            .as_str()
            .map(|t| vec![CliOutput::Text(t.to_string())])
            .unwrap_or_default(),
        Some("web_search") => vec![CliOutput::Text(format!(
            "web search: {}",
            item["query"].as_str().unwrap_or_default()
        ))],
        Some(kind @ ("command_execution" | "file_change")) => {
            vec![CliOutput::Text(format!(
                "warning: codex used a built-in tool ({kind}) that should be off"
            ))]
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    fn ctx<'a>(model: Option<&'a str>) -> CliContext<'a> {
        CliContext {
            system: "sys",
            user: "do it",
            mcp_url: "http://127.0.0.1:9/mcp",
            token: "tok123",
            dir: Path::new("/work"),
            model,
            max_turns: 40,
            api_key_set: false,
            web_search: false,
        }
    }

    #[test]
    fn exec_is_read_only_ephemeral_and_ignores_the_user_config() {
        let inv = Codex.build(&ctx(None));
        assert_eq!(inv.program, "codex");
        assert_eq!(inv.args[..2], ["exec".to_string(), "do it".to_string()]);
        for flag in [
            "--json",
            "--ephemeral",
            "--skip-git-repo-check",
            "--ignore-user-config",
        ] {
            assert!(inv.args.iter().any(|a| a == flag), "{flag}");
        }
        let sandbox = inv.args.iter().position(|a| a == "--sandbox").unwrap();
        assert_eq!(inv.args[sandbox + 1], "read-only");
    }

    #[test]
    fn config_overrides_register_the_server_and_switch_off_builtin_tools() {
        let inv = Codex.build(&ctx(None));
        let overrides: Vec<&str> = inv
            .args
            .windows(2)
            .filter(|w| w[0] == "-c")
            .map(|w| w[1].as_str())
            .collect();
        for expected in [
            "model_instructions_file=\"/work/system.md\"",
            "mcp_servers.mads.url=\"http://127.0.0.1:9/mcp\"",
            "mcp_servers.mads.bearer_token_env_var=\"MADS_MCP_TOKEN\"",
            "mcp_servers.mads.required=true",
            "features.shell_tool=false",
            "features.multi_agent=false",
            "web_search=\"disabled\"",
        ] {
            assert!(
                overrides.contains(&expected),
                "missing {expected}: {overrides:?}"
            );
        }
    }

    #[test]
    fn web_search_switches_codex_search_to_live() {
        let mut c = ctx(None);
        c.web_search = true;
        let inv = Codex.build(&c);
        assert!(inv.args.iter().any(|a| a == "web_search=\"live\""));
        assert!(!inv.args.iter().any(|a| a == "web_search=\"disabled\""));
        assert!(inv.args.iter().any(|a| a == "features.shell_tool=false"));
    }

    #[test]
    fn a_web_search_item_is_reported_as_progress() {
        let line = r#"{"type":"item.completed","item":{"id":"i","type":"web_search","query":"top wines brazil"}}"#;
        assert!(
            matches!(&Codex.parse_line(line)[..], [CliOutput::Text(t)] if t == "web search: top wines brazil")
        );
    }

    #[test]
    fn the_token_goes_through_the_environment_not_argv() {
        let inv = Codex.build(&ctx(None));
        assert_eq!(
            inv.env,
            vec![("MADS_MCP_TOKEN".to_string(), "tok123".to_string())]
        );
        assert!(!inv.args.iter().any(|a| a.contains("tok123")));
    }

    #[test]
    fn only_the_system_prompt_is_written_to_disk() {
        let inv = Codex.build(&ctx(Some("gpt-x")));
        assert_eq!(
            inv.files,
            vec![("system.md".to_string(), "sys".to_string())]
        );
        let m = inv.args.iter().position(|a| a == "--model").unwrap();
        assert_eq!(inv.args[m + 1], "gpt-x");
    }

    #[test]
    fn agent_messages_and_usage_are_parsed() {
        let msg = r#"{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"working on it"}}"#;
        assert!(matches!(&Codex.parse_line(msg)[..], [CliOutput::Text(t)] if t == "working on it"));
        let done = r#"{"type":"turn.completed","usage":{"input_tokens":120,"cached_input_tokens":20,"output_tokens":30}}"#;
        match &Codex.parse_line(done)[..] {
            [CliOutput::Usage(u)] => assert_eq!((u.input_tokens, u.output_tokens), (120, 30)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn failures_are_reported() {
        let failed = r#"{"type":"turn.failed","error":{"message":"rate limited"}}"#;
        assert!(
            matches!(&Codex.parse_line(failed)[..], [CliOutput::Failure(m)] if m == "rate limited")
        );
        let err = r#"{"type":"error","message":"boom"}"#;
        assert!(matches!(&Codex.parse_line(err)[..], [CliOutput::Failure(m)] if m == "boom"));
    }

    #[test]
    fn builtin_tool_use_is_flagged_as_a_warning() {
        let cmd = r#"{"type":"item.completed","item":{"id":"i","type":"command_execution","command":"ls"}}"#;
        match &Codex.parse_line(cmd)[..] {
            [CliOutput::Text(t)] => assert!(
                t.contains("warning") && t.contains("command_execution"),
                "{t}"
            ),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn unknown_lines_are_ignored() {
        for line in [
            "",
            "garbage",
            r#"{"type":"thread.started","thread_id":"x"}"#,
            r#"{"type":"turn.started"}"#,
        ] {
            assert!(Codex.parse_line(line).is_empty(), "{line}");
        }
    }
}
