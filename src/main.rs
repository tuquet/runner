mod cli;
mod commands;

use clap::Parser;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use runner::core::engine::RunnerEngine;
use crate::cli::{Cli, Commands, get_config_dir};
use crate::commands::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing logger
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    let cli = Cli::parse();
    let engine = Arc::new(RunnerEngine::new());
    let config_dir = get_config_dir();

    match cli.command {
        Commands::Run { file } => handle_run(file, engine).await?,
        Commands::Exec { driver, command, prompt, cwd, timeout } => {
            handle_exec(driver, command, prompt, cwd, timeout, engine).await?
        }
        Commands::Enroll { env, url, key, token } => {
            handle_enroll(env, url, key, token, &config_dir).await?
        }
        Commands::Env { action } => handle_env(action, &config_dir).await?,
        Commands::Worker { server, token, id, tags } => {
            handle_worker(server, token, id, tags, engine, &config_dir).await?
        }
        Commands::Purge => handle_purge(&config_dir)?,
        Commands::Info => handle_info(&config_dir)?,
    }

    Ok(())
}
