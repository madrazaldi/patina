mod cli;
mod fetch;
mod format;
mod item;
mod search;
mod tui;

use std::process::ExitCode;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use cli::Cli;

fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    let dir = patina_core::config::data_dir();
    std::fs::create_dir_all(&dir).ok()?;
    let file = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("patina-tui.log")).ok()?;
    let filter = EnvFilter::try_from_env("PATINA_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let (writer, guard) = tracing_appender::non_blocking(file);
    tracing_subscriber::fmt().with_writer(writer).with_ansi(false).with_env_filter(filter).init();
    Some(guard)
}

async fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(cmd) => cli::run(cmd).await,
        None => tui::run().await,
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let _guard = init_logging();
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "patina-tui starting");
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("patina-tui: {e}");
            return ExitCode::FAILURE;
        }
    };
    let result = runtime.block_on(run(cli));
    // Closes mpv if it is still open; must run outside the runtime.
    patina_core::player::shutdown();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("patina-tui: {e:#}");
            ExitCode::FAILURE
        }
    }
}
