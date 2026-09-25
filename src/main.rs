use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tuquet_runner::core::engine::RunnerEngine;
use tuquet_runner::protocol::schema::{DriverType, Job, JobStatus};
use tuquet_runner::transport::local_channel::LocalRunner;
use tuquet_runner::transport::ws_client::{WsClientConfig, WsRunnerClient};

#[derive(Parser)]
#[command(
    name = "tuquet-runner",
    version,
    about = "Ultra-high performance universal distributed execution engine in Rust",
    long_about = "Executes AI agents, browser workflows, shell commands, and webhooks across distributed nodes."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a job specification file (.yaml or .json) locally
    Run {
        /// Path to the job specification YAML or JSON file
        file: PathBuf,
    },
    /// Directly execute an ad-hoc command or prompt
    Exec {
        /// Driver to use (shell, agent, http)
        #[arg(short, long, default_value = "shell")]
        driver: String,

        /// Command to execute (for shell driver)
        #[arg(short, long)]
        command: Option<String>,

        /// Prompt to execute (for agent driver)
        #[arg(short, long)]
        prompt: Option<String>,

        /// Working directory
        #[arg(long)]
        cwd: Option<String>,

        /// Execution timeout in milliseconds
        #[arg(long, default_value_t = 300000)]
        timeout: u64,
    },
    /// Start in background worker daemon mode connecting outbound to Web Control Plane
    Worker {
        /// Web Control Plane WebSocket endpoint (e.g. wss://hub.tuquet.dev/api/v1/runner/ws)
        #[arg(short, long, env = "TUQUET_SERVER")]
        server: String,

        /// Authentication token for the runner
        #[arg(short, long, env = "TUQUET_TOKEN")]
        token: Option<String>,

        /// Unique runner node ID (defaults to hostname-uuid)
        #[arg(long, env = "TUQUET_RUNNER_ID")]
        id: Option<String>,

        /// Comma-separated tags (e.g. windows,workstation,gpu)
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// Display system information, capabilities, and registered drivers
    Info,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing logger
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    let cli = Cli::parse();
    let engine = Arc::new(RunnerEngine::new());

    match cli.command {
        Commands::Run { file } => {
            if !file.exists() {
                eprintln!("\x1b[31m[ERROR] File '{}' not found.\x1b[0m", file.display());
                std::process::exit(1);
            }

            let content = fs::read_to_string(&file)?;
            let job: Job = if file.extension().and_then(|s| s.to_str()) == Some("json") {
                serde_json::from_str(&content)?
            } else {
                serde_yaml::from_str(&content)?
            };

            let local_runner = LocalRunner::new(engine);
            let result = local_runner.run_job(job).await;

            if result.status == JobStatus::Completed {
                println!("\n\x1b[32m[SUCCESS] Job completed in {}ms\x1b[0m", result.duration_ms);
                std::process::exit(0);
            } else {
                eprintln!("\n\x1b[31m[FAILED] Job failed with status {:?} ({}ms): {:?}\x1b[0m",
                    result.status, result.duration_ms, result.error
                );
                std::process::exit(result.exit_code.unwrap_or(1));
            }
        }
        Commands::Exec {
            driver,
            command,
            prompt,
            cwd,
            timeout,
        } => {
            let driver_type = match driver.as_str() {
                "shell" => DriverType::Shell,
                "agent" => DriverType::Agent,
                "http" => DriverType::Http,
                other => DriverType::Custom(other.to_string()),
            };

            let mut payload_map = serde_json::Map::new();
            if let Some(cmd) = command {
                payload_map.insert("command".to_string(), serde_json::Value::String(cmd));
            }
            if let Some(p) = prompt {
                payload_map.insert("prompt".to_string(), serde_json::Value::String(p));
            }

            let job = Job {
                id: format!("exec-{}", uuid::Uuid::new_v4().simple()),
                driver: driver_type,
                payload: serde_json::Value::Object(payload_map),
                cwd,
                env: None,
                timeout_ms: timeout,
                created_at: Some(chrono::Utc::now()),
            };

            let local_runner = LocalRunner::new(engine);
            let result = local_runner.run_job(job).await;

            if result.status == JobStatus::Completed {
                println!("\n\x1b[32m[SUCCESS] Execution finished in {}ms\x1b[0m", result.duration_ms);
                std::process::exit(0);
            } else {
                eprintln!("\n\x1b[31m[FAILED] Execution failed ({:?}): {:?}\x1b[0m", result.status, result.error);
                std::process::exit(result.exit_code.unwrap_or(1));
            }
        }
        Commands::Worker { server, token, id, tags } => {
            let runner_id = id.unwrap_or_else(|| {
                let host = std::env::var("COMPUTERNAME")
                    .or_else(|_| std::env::var("HOSTNAME"))
                    .unwrap_or_else(|_| "node".to_string());
                format!("{}-{}", host, &uuid::Uuid::new_v4().simple().to_string()[..8])
            });

            println!("============================================================");
            println!(" Starting Tuquet-Runner in Worker Daemon Mode");
            println!("============================================================");
            println!(" Runner ID:  {}", runner_id);
            println!(" Server:     {}", server);
            println!(" OS/Arch:    {}/{}", std::env::consts::OS, std::env::consts::ARCH);
            println!(" Tags:       {:?}", tags);
            println!("============================================================");

            let config = WsClientConfig {
                server_url: server,
                token,
                runner_id,
                tags,
            };

            let client = WsRunnerClient::new(config, engine);
            client.start().await;
        }
        Commands::Info => {
            println!("============================================================");
            println!(" Tuquet-Runner System Information & Diagnostics");
            println!("============================================================");
            println!(" Version:      {}", env!("CARGO_PKG_VERSION"));
            println!(" OS:           {}", std::env::consts::OS);
            println!(" Architecture: {}", std::env::consts::ARCH);
            println!(" Drivers:      [shell, agent (claude-agy), automa, http]");
            println!(" Memory Idle:  <10MB (Rust Native Zero-GC)");
            println!(" Supervision:  {}", if cfg!(windows) { "Win32 Job Objects (Kernel-Level Zero-Zombie)" } else { "POSIX Process Groups" });
            println!("============================================================");
        }
    }

    Ok(())
}
