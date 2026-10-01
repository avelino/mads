use mads_core::usage::Usage;
use serde_json::{Value, json};

use super::{CliAgent, CliContext, CliInvocation, CliOutput, program_for};

/// Gemini CLI in headless mode. Settings and the system prompt are redirected to private files
/// through environment variables, so the user's own configuration is left alone.
pub struct Gemini;

impl CliAgent for Gemini {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn binary(&self) -> &'static str {
        "gemini"
    }

    fn binary_env(&self) -> &'static str {
        "MADS_GEMINI_BIN"
    }

    fn min_version(&self) -> (u32, u32, u32) {
        (0, 0, 0)
    }

    fn build(&self, c: &CliContext<'_>) -> CliInvocation {
        let mut args: Vec<String> = [
            "-p",
            c.user,
            "--output-format",
            "stream-json",
            "--approval-mode",
            "yolo",
            "--allowed-mcp-server-names",
            "mads",
        ]
        .map(String::from)
        .into();
        if let Some(model) = c.model {
            args.extend(["--model".into(), model.into()]);
        }
        let settings = json!({
            "mcpServers": {"mads": {
                "httpUrl": c.mcp_url,
                "headers": {"Authorization": format!("Bearer {}", c.token)},
                "trust": true,
            }},
            "tools": {"core": []},
            "model": {"maxSessionTurns": c.max_turns},
        });
        let path = |name: &str| c.dir.join(name).display().to_string();
        CliInvocation {
            program: program_for(self),
            args,
            env: vec![
                (
                    "GEMINI_CLI_SYSTEM_SETTINGS_PATH".into(),
                    path("settings.json"),
                ),
                ("GEMINI_SYSTEM_MD".into(), path("system.md")),
            ],
            files: vec![
                ("system.md".into(), c.system.into()),
                ("settings.json".into(), settings.to_string()),
            ],
        }
    }

    fn parse_line(&self, line: &str) -> Vec<CliOutput> {
        let Ok(v) = serde_json::from_str::<Value>(line) else {
            return Vec::new();
        };
        match v["type"].as_str() {
            Some("message") if v["role"] == "assistant" => v["content"]
                .as_str()
                .map(|t| vec![CliOutput::Text(t.to_string())])
                .unwrap_or_default(),
            Some("error") => {
                vec![CliOutput::Failure(
                    v["message"]
                        .as_str()
                        .unwrap_or("gemini reported an error")
                        .to_string(),
                )]
            }
            Some("result") => result_outputs(&v),
            _ => Vec::new(),
        }
    }
}

fn result_outputs(v: &Value) -> Vec<CliOutput> {
    let n = |key: &str| v["stats"][key].as_u64();
    let (input, output) = match (n("input_tokens"), n("output_tokens"), n("total_tokens")) {
        (Some(i), Some(o), _) => (i, o),
        (_, _, Some(total)) => (total, 0),
        _ => (0, 0),
    };
    let mut out = Vec::new();
    if input + output > 0 {
        out.push(CliOutput::Usage(Usage {
            input_tokens: input,
            output_tokens: output,
            cost_usd: None,
        }));
    }
    if v["status"].as_str().is_some_and(|s| s != "success") {
        let message = v["error"]["message"]
            .as_str()
            .unwrap_or("gemini reported an error");
        out.push(CliOutput::Failure(message.to_string()));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::Value;

    use super::*;

    fn ctx<'a>(model: Option<&'a str>) -> CliContext<'a> {
        CliContext {
            system: "sys",
            user: "do it",
            mcp_url: "http://127.0.0.1:9/mcp",
            token: "tok123",
            dir: Path::new("/work"),
            model,
            max_turns: 25,
            api_key_set: false,
        }
    }

    #[test]
    fn headless_run_only_allows_the_mads_server() {
        let inv = Gemini.build(&ctx(Some("gemini-x")));
        assert_eq!(inv.program, "gemini");
        assert_eq!(inv.args[..2], ["-p".to_string(), "do it".to_string()]);
        let val = |flag: &str| {
            inv.args
                .iter()
                .position(|a| a == flag)
                .map(|i| inv.args[i + 1].clone())
        };
        assert_eq!(val("--output-format").as_deref(), Some("stream-json"));
        assert_eq!(val("--approval-mode").as_deref(), Some("yolo"));
        assert_eq!(val("--allowed-mcp-server-names").as_deref(), Some("mads"));
        assert_eq!(val("--model").as_deref(), Some("gemini-x"));
    }

    #[test]
    fn settings_file_registers_the_server_and_empties_the_core_tools() {
        let inv = Gemini.build(&ctx(None));
        let (_, text) = inv
            .files
            .iter()
            .find(|(n, _)| n == "settings.json")
            .unwrap();
        let v: Value = serde_json::from_str(text).unwrap();
        assert_eq!(v["mcpServers"]["mads"]["httpUrl"], "http://127.0.0.1:9/mcp");
        assert_eq!(
            v["mcpServers"]["mads"]["headers"]["Authorization"],
            "Bearer tok123"
        );
        assert_eq!(v["mcpServers"]["mads"]["trust"], true);
        assert_eq!(v["tools"]["core"], serde_json::json!([]));
        assert_eq!(v["model"]["maxSessionTurns"], 25);
    }

    #[test]
    fn env_points_the_cli_at_our_settings_and_system_prompt() {
        let inv = Gemini.build(&ctx(None));
        let env: std::collections::HashMap<_, _> = inv.env.iter().cloned().collect();
        assert_eq!(
            env["GEMINI_CLI_SYSTEM_SETTINGS_PATH"],
            "/work/settings.json"
        );
        assert_eq!(env["GEMINI_SYSTEM_MD"], "/work/system.md");
        assert!(!inv.args.iter().any(|a| a.contains("tok123")));
    }

    #[test]
    fn assistant_messages_and_usage_are_parsed() {
        let msg = r#"{"type":"message","role":"assistant","content":"on it"}"#;
        assert!(matches!(&Gemini.parse_line(msg)[..], [CliOutput::Text(t)] if t == "on it"));
        let user = r#"{"type":"message","role":"user","content":"do it"}"#;
        assert!(
            Gemini.parse_line(user).is_empty(),
            "the echoed prompt is not agent text"
        );
        let result = r#"{"type":"result","status":"success","stats":{"total_tokens":150,"input_tokens":120,"output_tokens":30}}"#;
        match &Gemini.parse_line(result)[..] {
            [CliOutput::Usage(u)] => assert_eq!((u.input_tokens, u.output_tokens), (120, 30)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn errors_are_reported() {
        let err = r#"{"type":"error","message":"quota exceeded"}"#;
        assert!(
            matches!(&Gemini.parse_line(err)[..], [CliOutput::Failure(m)] if m == "quota exceeded")
        );
        let bad = r#"{"type":"result","status":"error","error":{"message":"turn limit"},"stats":{"total_tokens":10}}"#;
        assert!(
            Gemini
                .parse_line(bad)
                .iter()
                .any(|o| matches!(o, CliOutput::Failure(m) if m == "turn limit"))
        );
    }

    #[test]
    fn unknown_lines_are_ignored() {
        for line in [
            "",
            "x",
            r#"{"type":"init","session_id":"s"}"#,
            r#"{"type":"tool_use","tool_name":"x"}"#,
        ] {
            assert!(Gemini.parse_line(line).is_empty(), "{line}");
        }
    }
}
