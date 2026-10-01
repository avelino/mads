mod campaign;
mod output;
mod plan;
pub mod schema;
#[cfg(test)]
mod tests;

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
};

use async_trait::async_trait;
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::Mutex;

pub use output::ToolOutput;
pub(crate) use output::parse_args;

use crate::{google::SnippetHeader, workspace::Workspace};

pub type SharedWorkspace = Arc<Mutex<Workspace>>;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Which tools a mission can see and which part of the account it can touch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MissionKind {
    Plan,
    Campaign { slug: String },
}

#[derive(Debug, Clone, Copy)]
pub struct ToolSettings {
    pub max_ad_groups: usize,
    pub max_turns: usize,
}

impl Default for ToolSettings {
    fn default() -> Self {
        Self {
            max_ad_groups: 50,
            max_turns: 40,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("campaign '{0}' is not in the account plan")]
    UnknownCampaign(String),
}

#[async_trait]
pub trait ToolHost: Send + Sync {
    fn specs(&self) -> Vec<ToolSpec>;
    async fn call(&self, name: &str, args: Value) -> ToolOutput;
    /// True once the mission's `finish` tool succeeded.
    fn finished(&self) -> bool;
}

pub struct MissionTools {
    ws: SharedWorkspace,
    kind: MissionKind,
    settings: ToolSettings,
    persist: Option<PathBuf>,
    finished: AtomicBool,
    calls: AtomicUsize,
}

impl MissionTools {
    pub async fn new(
        ws: SharedWorkspace,
        kind: MissionKind,
        settings: ToolSettings,
        persist: Option<PathBuf>,
    ) -> Result<Self, ToolError> {
        if let MissionKind::Campaign { slug } = &kind {
            let known = ws
                .lock()
                .await
                .account
                .campaigns
                .iter()
                .any(|c| &c.slug == slug);
            if !known {
                return Err(ToolError::UnknownCampaign(slug.clone()));
            }
        }
        Ok(Self {
            ws,
            kind,
            settings,
            persist,
            finished: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
        })
    }

    fn mark_finished(&self) {
        self.finished.store(true, Ordering::SeqCst);
    }

    /// Saves after a mutation so an interrupted run can resume.
    fn persist(&self, ws: &Workspace) -> Result<(), ToolOutput> {
        match &self.persist {
            Some(path) => ws
                .save(path)
                .map_err(|e| ToolOutput::fail("PERSIST", e.to_string())),
            None => Ok(()),
        }
    }
}

#[async_trait]
impl ToolHost for MissionTools {
    fn specs(&self) -> Vec<ToolSpec> {
        match self.kind {
            MissionKind::Plan => plan::specs(),
            MissionKind::Campaign { .. } => campaign::specs(),
        }
    }

    async fn call(&self, name: &str, args: Value) -> ToolOutput {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if n > self.settings.max_turns * 4 {
            return ToolOutput::fail("LIMIT", "tool call budget for this mission is exhausted");
        }
        if !self.specs().iter().any(|s| s.name == name) {
            return ToolOutput::fail(
                "UNKNOWN_TOOL",
                format!("'{name}' is not a tool of this mission"),
            );
        }
        match &self.kind {
            MissionKind::Plan => plan::call(self, name, args).await,
            MissionKind::Campaign { slug } => campaign::call(self, slug, name, args).await,
        }
    }

    fn finished(&self) -> bool {
        self.finished.load(Ordering::SeqCst)
    }
}

/// Limits the agent has to respect, in plain numbers.
fn rules_summary(settings: &ToolSettings) -> Value {
    let headers: Vec<Value> = SnippetHeader::ALL
        .iter()
        .filter_map(|h| serde_json::to_value(h).ok())
        .collect();
    json!({
        "headline_max_chars": 30,
        "description_max_chars": 90,
        "path_max_chars": 15,
        "sitelink_text_max_chars": 25,
        "sitelink_description_max_chars": 35,
        "callout_max_chars": 25,
        "snippet_value_max_chars": 25,
        "headline_exclamation_allowed": false,
        "brand_kit": {"headlines": "8 to 12", "descriptions": "2 to 3"},
        "rsa_specific": {"headlines": "3 to 7", "descriptions": "1 to 2"},
        "keywords_per_ad_group": "1 to 50",
        "keyword_spec": {"variants": "1 to 6", "modifiers": "0 to 10", "extra": "0 to 20"},
        "negatives_per_scope": "0 to 100",
        "campaigns": "1 to 5",
        "sitelinks": "2 to 8 (4 or more recommended)",
        "callouts": "2 to 10 (4 or more recommended)",
        "snippets": "0 to 2, 3 to 10 values each",
        "snippet_headers": headers,
        "intents": ["brand", "catalog", "generic", "competitor"],
        "match_types": ["phrase", "exact"],
        "bid_strategies": ["manual_cpc"],
        "max_ad_groups": settings.max_ad_groups,
    })
}
