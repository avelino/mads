use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use crate::google::Issue;

/// What a tool returns to the agent.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolOutput {
    pub is_error: bool,
    pub content: Value,
    /// One line for progress events.
    pub summary: String,
}

impl ToolOutput {
    pub fn ok(result: Value, warnings: &[Issue], summary: impl Into<String>) -> Self {
        let content = json!({"ok": true, "result": result, "warnings": warnings});
        Self {
            is_error: false,
            content,
            summary: summary.into(),
        }
    }

    pub fn fail_issues(errors: &[Issue], warnings: &[Issue], summary: impl Into<String>) -> Self {
        let content = json!({"ok": false, "errors": errors, "warnings": warnings});
        Self {
            is_error: true,
            content,
            summary: summary.into(),
        }
    }

    pub fn fail(code: &str, message: impl Into<String>) -> Self {
        let message = message.into();
        let issue = Issue::error(code, "", message.clone());
        Self::fail_issues(&[issue], &[], format!("{code}: {message}"))
    }

    pub fn text(&self) -> String {
        self.content.to_string()
    }
}

/// Deserializes tool arguments; a failure becomes an `ARGS` error the agent can read.
pub fn parse_args<T: DeserializeOwned>(args: Value) -> Result<T, ToolOutput> {
    serde_json::from_value(args).map_err(|e| ToolOutput::fail("ARGS", e.to_string()))
}

pub fn split(issues: Vec<Issue>) -> (Vec<Issue>, Vec<Issue>) {
    issues.into_iter().partition(Issue::is_error)
}

pub fn cents_to_f64(c: crate::money::Cents) -> f64 {
    c.0 as f64 / 100.0
}
