mod cli;
mod commands;
mod render;

use std::io::IsTerminal;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use cli::{Cli, Command, exit_code_of};

fn init_tracing(verbose: u8) {
    let default = match verbose {
        0 => "warn",
        1 => "info",
        _ => "debug",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_tracing(cli.verbose);
    let github = std::env::var("GITHUB_ACTIONS").is_ok_and(|v| v == "true");
    let format = render::resolve(cli.format, github, std::io::stdout().is_terminal());
    let result = match cli.command {
        Command::Init(args) => commands::init::run(args, format).await,
        Command::Generate(args) => commands::generate::run(args, format).await,
        Command::Export(args) => commands::export::run(args, format).await,
        Command::Providers => commands::providers::run(),
    };
    let code = match result {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            exit_code_of(&e)
        }
    };
    std::process::exit(code);
}
