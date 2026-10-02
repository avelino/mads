use std::{path::PathBuf, time::Duration};

use clap::{Args, Parser, Subcommand};
use mads_providers::ProviderSelection;

use crate::render::Format;

#[derive(Parser)]
#[command(
    name = "mads",
    version,
    about = "Generate Google Ads campaigns from a business description"
)]
pub struct Cli {
    /// Output format. `auto` picks github on GitHub Actions, pretty on a terminal, plain otherwise.
    #[arg(
        long,
        global = true,
        env = "MADS_FORMAT",
        value_enum,
        default_value = "auto"
    )]
    pub format: Format,
    /// Log to stderr: -v for info, -vv for debug.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    #[command(subcommand)]
    pub command: Command,
}

// Parsed once at startup, so the size difference between variants costs nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
pub enum Command {
    /// Study a website with an agent and draft business.toml, catalog.csv and research.md for review.
    Init(InitArgs),
    /// Generate campaigns and the Google Ads bulk upload CSVs from business.toml.
    Generate(GenerateArgs),
    /// Validate and export a finished run again, without calling any model.
    Export(ExportArgs),
    /// List the providers and whether each one is ready.
    Providers,
}

#[derive(Args)]
pub struct InitArgs {
    /// Website to read. The agent only requests pages on this host.
    #[arg(long)]
    pub from_url: String,
    /// Daily budget in the account currency. The site does not say how much you want to spend.
    #[arg(long)]
    pub daily_budget: f64,
    /// ISO 4217 code, such as BRL or USD.
    #[arg(long)]
    pub currency: String,
    /// Where business.toml, catalog.csv and research.md are written.
    #[arg(long, default_value = ".")]
    pub out_dir: PathBuf,
    /// Catalog items the agent may add.
    #[arg(long, default_value_t = 50)]
    pub catalog_limit: usize,
    /// Overwrite existing files.
    #[arg(long)]
    pub force: bool,
    /// Keep the agent off the web: it learns from the site only. Agent CLIs search the web by default.
    #[arg(long)]
    pub no_web_search: bool,
    /// Advertise only the offer of --from-url (a route, a product line), not the whole business.
    #[arg(long)]
    pub focus: bool,
    #[command(flatten)]
    pub agent: AgentArgs,
}

#[derive(Args)]
pub struct GenerateArgs {
    /// Path to business.toml.
    #[arg(required_unless_present = "resume")]
    pub business: Option<PathBuf>,
    /// Directory where run directories are created.
    #[arg(long, default_value = "out")]
    pub out: PathBuf,
    /// Run directory to resume: only the missions that are not finished run again.
    #[arg(long)]
    pub resume: Option<PathBuf>,
    /// Campaign missions that run at the same time.
    #[arg(long, default_value_t = 4)]
    pub parallel: usize,
    /// Do not request the final and sitelink URLs.
    #[arg(long)]
    pub skip_url_check: bool,
    /// Ad groups allowed in the whole account.
    #[arg(long, default_value_t = 50)]
    pub max_ad_groups: usize,
    /// Image model for Performance Max and Demand Gen: auto, gemini, openai or none.
    /// auto picks gemini when GEMINI_API_KEY is set, then openai when OPENAI_API_KEY is set.
    #[arg(long, env = "MADS_IMAGE_PROVIDER", default_value = "auto")]
    pub image_provider: String,
    /// Image model name, such as gemini-2.5-flash-image or gpt-image-1.
    #[arg(long, env = "MADS_IMAGE_MODEL")]
    pub image_model: Option<String>,
    /// New images allowed in one run. Every image costs money.
    #[arg(long, default_value_t = 40)]
    pub max_images: usize,
    #[command(flatten)]
    pub agent: AgentArgs,
}

#[derive(Args)]
pub struct ExportArgs {
    /// Run directory with a finished run.
    pub run_dir: PathBuf,
    #[arg(long)]
    pub skip_url_check: bool,
    /// Ad groups allowed in the whole account.
    #[arg(long, default_value_t = 50)]
    pub max_ad_groups: usize,
}

#[derive(Args)]
pub struct AgentArgs {
    /// anthropic, openai, gemini, openrouter, groq, deepseek, xai, ollama, openai-compat, claude-cli, codex-cli or gemini-cli.
    #[arg(long, env = "MADS_PROVIDER")]
    pub provider: Option<String>,
    #[arg(long, env = "MADS_MODEL")]
    pub model: Option<String>,
    /// Model for the plan mission only.
    #[arg(long, env = "MADS_PLAN_MODEL")]
    pub plan_model: Option<String>,
    /// Endpoint for openai-compat.
    #[arg(long, env = "MADS_BASE_URL")]
    pub base_url: Option<String>,
    /// Turns allowed per mission.
    #[arg(long, default_value_t = 40)]
    pub max_turns: usize,
    /// Time allowed per mission, such as 90s, 15m or 2h.
    #[arg(long, default_value = "15m", value_parser = parse_duration)]
    pub mission_timeout: Duration,
    /// Input plus output tokens allowed in the whole run. 0 disables the limit.
    #[arg(long, default_value_t = 4_000_000)]
    pub max_tokens: u64,
    /// Extra attempts for a mission that fails.
    #[arg(long, default_value_t = 1)]
    pub mission_retries: u32,
    /// JSON script for the hidden `replay` provider.
    #[arg(long, hide = true)]
    pub script: Option<PathBuf>,
}

impl AgentArgs {
    pub fn selection(&self) -> ProviderSelection {
        ProviderSelection {
            provider: self.provider.clone().unwrap_or_default(),
            model: self.model.clone(),
            plan_model: self.plan_model.clone(),
            base_url: self.base_url.clone(),
            script: self.script.clone(),
        }
    }
}

/// `90s`, `15m`, `2h` or bare seconds. Zero is rejected.
pub fn parse_duration(text: &str) -> Result<Duration, String> {
    let t = text.trim();
    let (digits, unit) = match t.char_indices().last() {
        Some((i, c @ ('s' | 'm' | 'h'))) => (&t[..i], c),
        _ => (t, 's'),
    };
    let n: u64 = digits
        .parse()
        .map_err(|_| format!("'{text}' is not a duration (use 90s, 15m or 2h)"))?;
    if n == 0 {
        return Err("duration must be greater than 0".into());
    }
    let secs = match unit {
        'h' => n * 3600,
        'm' => n * 60,
        _ => n,
    };
    Ok(Duration::from_secs(secs))
}

/// An error that decides the process exit code.
#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct CliError {
    pub code: i32,
    pub message: String,
}

/// Exit code 2: the input or the flags are wrong.
pub fn usage(message: impl Into<String>) -> anyhow::Error {
    CliError {
        code: 2,
        message: message.into(),
    }
    .into()
}

pub fn exit_code_of(e: &anyhow::Error) -> i32 {
    e.downcast_ref::<CliError>().map_or(1, |c| c.code)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;

    #[test]
    fn durations_accept_units_and_default_to_seconds() {
        assert_eq!(parse_duration("30s"), Ok(Duration::from_secs(30)));
        assert_eq!(parse_duration("15m"), Ok(Duration::from_secs(900)));
        assert_eq!(parse_duration("2h"), Ok(Duration::from_secs(7200)));
        assert_eq!(parse_duration("90"), Ok(Duration::from_secs(90)));
        assert_eq!(parse_duration(" 5m "), Ok(Duration::from_secs(300)));
    }

    #[test]
    fn durations_reject_garbage_and_zero() {
        for bad in ["", "m", "abc", "-5s", "0", "0m", "1.5h", "10x"] {
            assert!(parse_duration(bad).is_err(), "{bad:?} should fail");
        }
    }

    #[test]
    fn generate_defaults_match_the_spec() {
        let cli = Cli::try_parse_from(["mads", "generate", "b.toml"]).unwrap();
        let Command::Generate(g) = cli.command else {
            panic!("expected generate")
        };
        assert_eq!(g.business.as_deref(), Some(std::path::Path::new("b.toml")));
        assert_eq!(g.out, std::path::PathBuf::from("out"));
        assert_eq!(
            (g.parallel, g.max_ad_groups, g.skip_url_check),
            (4, 50, false)
        );
        assert_eq!(
            (
                g.agent.max_turns,
                g.agent.max_tokens,
                g.agent.mission_retries
            ),
            (40, 4_000_000, 1)
        );
        assert_eq!(g.agent.mission_timeout, Duration::from_secs(900));
    }

    #[test]
    fn generate_needs_a_file_unless_resuming() {
        assert!(Cli::try_parse_from(["mads", "generate"]).is_err());
        assert!(Cli::try_parse_from(["mads", "generate", "--resume", "out/x"]).is_ok());
    }

    #[test]
    fn global_flags_work_after_the_subcommand() {
        let cli =
            Cli::try_parse_from(["mads", "generate", "b.toml", "--format", "json", "-vv"]).unwrap();
        assert_eq!(cli.format, Format::Json);
        assert_eq!(cli.verbose, 2);
    }

    #[test]
    fn selection_carries_the_agent_flags() {
        let cli = Cli::try_parse_from([
            "mads",
            "generate",
            "b.toml",
            "--provider",
            "ollama",
            "--model",
            "m",
            "--plan-model",
            "p",
            "--base-url",
            "http://x",
        ])
        .unwrap();
        let Command::Generate(g) = cli.command else {
            panic!()
        };
        let sel = g.agent.selection();
        assert_eq!(
            (
                sel.provider.as_str(),
                sel.model.as_deref(),
                sel.plan_model.as_deref(),
                sel.base_url.as_deref()
            ),
            ("ollama", Some("m"), Some("p"), Some("http://x"))
        );
    }

    #[test]
    fn unknown_flags_are_usage_errors() {
        assert!(Cli::try_parse_from(["mads", "generate", "b.toml", "--nope"]).is_err());
    }

    #[test]
    fn cli_error_carries_its_exit_code() {
        let e = usage("bad input");
        assert_eq!(exit_code_of(&e), 2);
        assert_eq!(exit_code_of(&anyhow::anyhow!("boom")), 1);
    }
}
