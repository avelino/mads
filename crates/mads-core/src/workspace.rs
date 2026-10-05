use std::{collections::BTreeMap, io, path::Path};

use serde::{Deserialize, Serialize};

use crate::{google::Account, input::Input, perf::Live, usage::Usage};

pub const WORKSPACE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Workspace {
    pub version: u32,
    pub input: Input,
    pub account: Account,
    /// Keyed by mission id (`plan`, `campaign:<slug>`).
    pub missions: BTreeMap<String, MissionState>,
    /// Set by `mads optimize`: the account as it ran and what the reports say about it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live: Option<Live>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MissionState {
    pub status: MissionStatus,
    pub attempts: u32,
    pub usage: Usage,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissionStatus {
    #[default]
    Pending,
    Running,
    Finished,
    Failed {
        reason: String,
    },
}

impl Workspace {
    pub fn new(input: Input) -> Self {
        Self {
            version: WORKSPACE_VERSION,
            input,
            account: Account::default(),
            missions: BTreeMap::new(),
            live: None,
        }
    }

    /// Writes `<path>.tmp` then renames, so a crash never leaves a half-written file.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(io::Error::other)?;
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }

    pub fn load(path: &Path) -> io::Result<Self> {
        let bytes = std::fs::read(path)?;
        let ws: Workspace = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
        if ws.version != WORKSPACE_VERSION {
            let msg = format!(
                "unsupported workspace version {} (expected {WORKSPACE_VERSION})",
                ws.version
            );
            return Err(io::Error::other(msg));
        }
        Ok(ws)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Budget, Business, ExportConfig, ExportStatus};
    use crate::money::Cents;

    fn input() -> Input {
        Input {
            logo: None,
            formats: Vec::new(),
            design: String::new(),
            focus: None,
            app: None,
            business: Business {
                name: "Acme".into(),
                url: "https://acme.com".into(),
                language: "en-US".into(),
                locations: vec!["United States".into()],
                goal: "g".into(),
                description: "d".repeat(30),
                conversion_tracking: false,
                restricted: vec![],
                brand_terms: vec!["acme".into()],
                competitors: vec![],
                avoid: vec![],
                pages: vec![],
            },
            budget: Budget {
                daily: Cents(5000),
                currency: "USD".into(),
                max_cpc: None,
            },
            export: ExportConfig {
                status: ExportStatus::Paused,
                url_suffix: String::new(),
                eu_political_ads: false,
                decimal_comma: false,
            },
            research: String::new(),
            catalog: vec![],
        }
    }

    #[test]
    fn save_then_load_roundtrips_and_leaves_no_tmp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        let mut ws = Workspace::new(input());
        ws.missions.insert(
            "plan".into(),
            MissionState {
                status: MissionStatus::Failed {
                    reason: "boom".into(),
                },
                attempts: 2,
                usage: Usage::default(),
            },
        );
        ws.save(&path).unwrap();
        assert_eq!(Workspace::load(&path).unwrap(), ws);
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, ["workspace.json"]);
    }

    #[test]
    fn load_rejects_unknown_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("workspace.json");
        let mut ws = Workspace::new(input());
        ws.version = 99;
        ws.save(&path).unwrap();
        let err = Workspace::load(&path).unwrap_err();
        assert!(err.to_string().contains("version"), "{err}");
    }

    #[test]
    fn load_missing_file_is_an_error() {
        assert!(Workspace::load(std::path::Path::new("/nonexistent/workspace.json")).is_err());
    }

    #[test]
    fn mission_state_defaults_to_pending() {
        assert_eq!(MissionState::default().status, MissionStatus::Pending);
        assert_eq!(MissionState::default().attempts, 0);
    }
}
