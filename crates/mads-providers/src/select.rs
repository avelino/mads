use std::{path::PathBuf, sync::Arc};

use mads_core::{
    agent::{Driver, LoopDriver, ScriptedDriver},
    run::Drivers,
};

use crate::{
    cli::{Claude, CliAgent, CliDriver, Codex, Gemini, check_cli, program_for},
    genai_model::{GenaiChatModel, api_provider},
};

/// What the user asked for on the command line or in `MADS_*` variables.
#[derive(Debug, Clone, Default)]
pub struct ProviderSelection {
    pub provider: String,
    pub model: Option<String>,
    pub plan_model: Option<String>,
    pub base_url: Option<String>,
    /// `replay` only: JSON script with recorded responses.
    pub script: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("unknown provider '{0}'; run `mads providers` to see the options")]
    Unknown(String),
    #[error("provider '{0}' is not available in this build yet")]
    NotAvailable(String),
    #[error("{0}")]
    Config(String),
    #[error("script: {0}")]
    Script(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Api,
    Cli,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderStatus {
    Ready,
    MissingEnv(String),
    CliNotFound(String),
    NotAvailable,
}

impl ProviderStatus {
    pub fn describe(&self) -> String {
        match self {
            ProviderStatus::Ready => "ready".into(),
            ProviderStatus::MissingEnv(v) => format!("missing {v}"),
            ProviderStatus::CliNotFound(b) => format!("`{b}` not found in PATH"),
            ProviderStatus::NotAvailable => "not available in this build".into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub name: &'static str,
    pub kind: Kind,
    pub status: ProviderStatus,
}

pub(crate) struct Spec {
    pub name: &'static str,
    pub kind: Kind,
    pub env: Option<&'static str>,
    pub binary: Option<&'static str>,
    pub implemented: bool,
}

const fn api(name: &'static str, env: Option<&'static str>) -> Spec {
    Spec {
        name,
        kind: Kind::Api,
        env,
        binary: None,
        implemented: true,
    }
}

const fn cli(name: &'static str, binary: &'static str) -> Spec {
    Spec {
        name,
        kind: Kind::Cli,
        env: None,
        binary: Some(binary),
        implemented: true,
    }
}

const SPECS: [Spec; 12] = [
    api("anthropic", Some("ANTHROPIC_API_KEY")),
    api("openai", Some("OPENAI_API_KEY")),
    api("gemini", Some("GEMINI_API_KEY")),
    api("openrouter", Some("OPENROUTER_API_KEY")),
    api("groq", Some("GROQ_API_KEY")),
    api("deepseek", Some("DEEPSEEK_API_KEY")),
    api("xai", Some("XAI_API_KEY")),
    api("ollama", None),
    api("openai-compat", None),
    cli("claude-cli", "claude"),
    cli("codex-cli", "codex"),
    cli("gemini-cli", "gemini"),
];

pub(crate) fn status_of(
    spec: &Spec,
    has_env: &dyn Fn(&str) -> bool,
    has_binary: &dyn Fn(&str) -> bool,
) -> ProviderStatus {
    if !spec.implemented {
        return ProviderStatus::NotAvailable;
    }
    if let Some(var) = spec.env.filter(|v| !has_env(v)) {
        return ProviderStatus::MissingEnv(var.into());
    }
    if let Some(bin) = spec.binary.filter(|b| !has_binary(b)) {
        return ProviderStatus::CliNotFound(bin.into());
    }
    ProviderStatus::Ready
}

/// Honors the `MADS_<NAME>_BIN` override, then looks in PATH, like `generate` does.
fn binary_in_path(name: &str) -> bool {
    let var = format!("MADS_{}_BIN", name.to_uppercase());
    if let Some(custom) = std::env::var_os(&var).filter(|v| !v.is_empty()) {
        let path = std::path::PathBuf::from(&custom);
        return path.is_file()
            || std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .any(|d| d.join(&custom).is_file());
    }
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|dir| dir.join(name).is_file()))
}

pub fn list_providers() -> Vec<ProviderInfo> {
    let has_env = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    SPECS
        .iter()
        .map(|s| ProviderInfo {
            name: s.name,
            kind: s.kind,
            status: status_of(s, &has_env, &binary_in_path),
        })
        .collect()
}

pub fn build_drivers(sel: &ProviderSelection) -> Result<Drivers, ProviderError> {
    match sel.provider.as_str() {
        "" => Err(ProviderError::Config(
            "no provider: use --provider or MADS_PROVIDER".into(),
        )),
        "replay" => replay(sel),
        "claude-cli" => Ok(cli_drivers(sel, || Claude)),
        "codex-cli" => Ok(cli_drivers(sel, || Codex)),
        "gemini-cli" => Ok(cli_drivers(sel, || Gemini)),
        "openai-compat" => api_drivers(sel, "openai-compat"),
        other if api_provider(other).is_some() => api_drivers(sel, other),
        other if SPECS.iter().any(|s| s.name == other) => {
            Err(ProviderError::NotAvailable(other.into()))
        }
        other => Err(ProviderError::Unknown(other.into())),
    }
}

/// Agent CLIs pick their own default model, so `--model` is optional here.
fn cli_drivers<A: CliAgent + 'static>(sel: &ProviderSelection, make: impl Fn() -> A) -> Drivers {
    let plan_model = sel.plan_model.clone().or_else(|| sel.model.clone());
    Drivers {
        plan: Arc::new(CliDriver::new(make()).with_model(plan_model)),
        campaign: Arc::new(CliDriver::new(make()).with_model(sel.model.clone())),
        image: None,
    }
}

/// Fails before any mission starts when an agent CLI is missing or too old.
pub async fn preflight(sel: &ProviderSelection) -> Result<(), ProviderError> {
    let agent: &dyn CliAgent = match sel.provider.as_str() {
        "claude-cli" => &Claude,
        "codex-cli" => &Codex,
        "gemini-cli" => &Gemini,
        _ => return Ok(()),
    };
    check_cli(&program_for(agent), agent.name(), agent.min_version())
        .await
        .map_err(ProviderError::Config)
}

fn api_drivers(sel: &ProviderSelection, provider: &str) -> Result<Drivers, ProviderError> {
    let model = sel.model.as_deref().ok_or_else(|| {
        ProviderError::Config(format!(
            "--model (or MADS_MODEL) is required for {provider}"
        ))
    })?;
    let plan_model = sel.plan_model.as_deref().unwrap_or(model);
    let make = |m: &str| -> Result<Arc<dyn Driver>, ProviderError> {
        let chat = if provider == "openai-compat" {
            let base = sel.base_url.as_deref().ok_or_else(|| {
                ProviderError::Config(
                    "--base-url (or MADS_BASE_URL) is required for openai-compat".into(),
                )
            })?;
            GenaiChatModel::openai_compat(
                base,
                m,
                std::env::var("MADS_API_KEY").ok().filter(|k| !k.is_empty()),
            )
        } else {
            GenaiChatModel::for_provider(provider, m)
                .ok_or_else(|| ProviderError::NotAvailable(provider.into()))?
        };
        Ok(Arc::new(LoopDriver::new(Arc::new(chat))))
    };
    Ok(Drivers {
        plan: make(plan_model)?,
        campaign: make(model)?,
        image: None,
    })
}

fn replay(sel: &ProviderSelection) -> Result<Drivers, ProviderError> {
    let path = sel
        .script
        .as_ref()
        .ok_or_else(|| ProviderError::Config("replay needs --script <file.json>".into()))?;
    let text = std::fs::read_to_string(path)
        .map_err(|e| ProviderError::Script(format!("{}: {e}", path.display())))?;
    let driver: Arc<dyn Driver> = Arc::new(
        ScriptedDriver::from_json(&text).map_err(|e| ProviderError::Script(e.to_string()))?,
    );
    Ok(Drivers {
        plan: driver.clone(),
        campaign: driver,
        image: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(provider: &str) -> ProviderSelection {
        ProviderSelection {
            provider: provider.into(),
            ..Default::default()
        }
    }

    #[test]
    fn replay_needs_a_script_file() {
        assert!(matches!(
            build_drivers(&sel("replay")),
            Err(ProviderError::Config(_))
        ));
        let mut s = sel("replay");
        s.script = Some("/nonexistent/script.json".into());
        assert!(matches!(build_drivers(&s), Err(ProviderError::Script(_))));
    }

    #[test]
    fn replay_loads_a_valid_script() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        std::fs::write(&path, r#"{"missions": {"plan": []}}"#).unwrap();
        let mut s = sel("replay");
        s.script = Some(path);
        assert!(build_drivers(&s).is_ok());
    }

    #[test]
    fn invalid_script_json_is_a_script_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        std::fs::write(&path, "{").unwrap();
        let mut s = sel("replay");
        s.script = Some(path);
        assert!(matches!(build_drivers(&s), Err(ProviderError::Script(_))));
    }

    #[test]
    fn unknown_provider_is_rejected_with_a_hint() {
        let err = build_drivers(&sel("gpt-9000")).err().unwrap();
        assert!(matches!(err, ProviderError::Unknown(_)));
        assert!(err.to_string().contains("mads providers"));
    }

    #[test]
    fn api_providers_need_a_model() {
        let err = build_drivers(&sel("anthropic")).err().unwrap();
        assert!(
            matches!(err, ProviderError::Config(_)) && err.to_string().contains("--model"),
            "{err}"
        );
    }

    #[test]
    fn api_providers_build_with_a_model_and_no_network() {
        for p in [
            "anthropic",
            "openai",
            "gemini",
            "openrouter",
            "groq",
            "deepseek",
            "xai",
            "ollama",
        ] {
            let mut s = sel(p);
            s.model = Some("some-model".into());
            assert!(build_drivers(&s).is_ok(), "{p}");
        }
    }

    #[test]
    fn a_plan_model_is_accepted() {
        let mut s = sel("anthropic");
        s.model = Some("m".into());
        s.plan_model = Some("p".into());
        assert!(build_drivers(&s).is_ok());
    }

    #[test]
    fn openai_compat_needs_a_base_url() {
        let mut s = sel("openai-compat");
        s.model = Some("m".into());
        let err = build_drivers(&s).err().unwrap();
        assert!(err.to_string().contains("--base-url"), "{err}");
        s.base_url = Some("http://localhost:8000/v1".into());
        assert!(build_drivers(&s).is_ok());
    }

    #[test]
    fn cli_providers_build_without_a_model_or_starting_anything() {
        for p in ["claude-cli", "codex-cli", "gemini-cli"] {
            assert!(build_drivers(&sel(p)).is_ok(), "{p}");
        }
    }

    #[tokio::test]
    async fn preflight_only_checks_cli_providers() {
        for p in ["anthropic", "openai-compat", "replay", ""] {
            assert!(preflight(&sel(p)).await.is_ok(), "{p}");
        }
    }

    #[test]
    fn empty_provider_is_a_config_error() {
        assert!(matches!(
            build_drivers(&sel("")),
            Err(ProviderError::Config(_))
        ));
    }

    #[test]
    fn listing_has_the_documented_providers_and_hides_replay() {
        let names: Vec<_> = list_providers().into_iter().map(|p| p.name).collect();
        assert_eq!(
            names,
            [
                "anthropic",
                "openai",
                "gemini",
                "openrouter",
                "groq",
                "deepseek",
                "xai",
                "ollama",
                "openai-compat",
                "claude-cli",
                "codex-cli",
                "gemini-cli"
            ]
        );
    }

    fn spec(env: Option<&'static str>, binary: Option<&'static str>, implemented: bool) -> Spec {
        Spec {
            name: "x",
            kind: if binary.is_some() {
                Kind::Cli
            } else {
                Kind::Api
            },
            env,
            binary,
            implemented,
        }
    }

    #[test]
    fn status_depends_on_env_binary_and_implementation() {
        let no_env = |_: &str| false;
        let has_env = |k: &str| k == "KEY";
        let no_bin = |_: &str| false;
        let has_bin = |b: &str| b == "tool";
        assert_eq!(
            status_of(&spec(Some("KEY"), None, true), &has_env, &no_bin),
            ProviderStatus::Ready
        );
        assert_eq!(
            status_of(&spec(Some("KEY"), None, true), &no_env, &no_bin),
            ProviderStatus::MissingEnv("KEY".into())
        );
        assert_eq!(
            status_of(&spec(None, Some("tool"), true), &no_env, &has_bin),
            ProviderStatus::Ready
        );
        assert_eq!(
            status_of(&spec(None, Some("tool"), true), &no_env, &no_bin),
            ProviderStatus::CliNotFound("tool".into())
        );
        assert_eq!(
            status_of(&spec(None, None, true), &no_env, &no_bin),
            ProviderStatus::Ready,
            "ollama needs nothing"
        );
        assert_eq!(
            status_of(&spec(Some("KEY"), None, false), &has_env, &no_bin),
            ProviderStatus::NotAvailable
        );
    }
}
