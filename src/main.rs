use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tuquet_runner::core::engine::RunnerEngine;
use tuquet_runner::core::enrollment::EnrollmentClient;
use tuquet_runner::core::fingerprint::FingerprintEngine;
use tuquet_runner::core::identity::DeviceIdentity;
use tuquet_runner::protocol::schema::{DriverType, Job, JobStatus};
use tuquet_runner::transport::local_channel::LocalRunner;
use tuquet_runner::transport::ws_client::{WsClientConfig, WsRunnerClient};

#[derive(Parser)]
#[command(
    name = "tqr",
    version,
    about = "Ultra-fast universal distributed execution engine in Rust (tqr / tuquet-runner)",
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
    /// Enroll this machine with Tuquet Cloud (zero-touch device registration)
    Enroll {
        /// Tuquet Cloud / Supabase URL (e.g. https://api.tuquet.dev or http://127.0.0.1:54321)
        #[arg(short, long, env = "TUQUET_CLOUD_URL", default_value = "https://api.tuquet.dev")]
        url: String,

        /// Supabase publishable / anon key
        #[arg(short, long, env = "TUQUET_API_KEY")]
        key: Option<String>,

        /// Optional workspace enrollment token
        #[arg(short, long, env = "TUQUET_ENROLLMENT_TOKEN")]
        token: Option<String>,
    },
    /// Start in background worker daemon mode connecting outbound to Web Control Plane
    Worker {
        /// Web Control Plane WebSocket endpoint (e.g. wss://hub.tuquet.dev/api/v1/runner/ws)
        #[arg(short, long, env = "TUQUET_SERVER")]
        server: Option<String>,

        /// Authentication token for the runner
        #[arg(short, long, env = "TUQUET_TOKEN")]
        token: Option<String>,

        /// Unique runner node ID (defaults to enrolled device_id or hostname-uuid)
        #[arg(long, env = "TUQUET_RUNNER_ID")]
        id: Option<String>,

        /// Comma-separated tags (e.g. windows,workstation,gpu)
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
    },
    /// Display system information, hardware fingerprint, capabilities, and enrollment status
    Info,
}

fn get_config_dir() -> PathBuf {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let scoop_config = parent.join("config");
            if scoop_config.exists() {
                return scoop_config;
            }
        }
    }
    let local = PathBuf::from("config");
    if local.exists() {
        return local;
    }
    if let Ok(user_home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        return PathBuf::from(user_home).join(".tuquet").join("config");
    }
    PathBuf::from("config")
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
    let config_dir = get_config_dir();

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
        Commands::Enroll { url, key, token } => {
            println!("============================================================");
            println!(" Tuquet Runner (tqr) - Cloud Device Enrollment");
            println!("============================================================");
            let specs = FingerprintEngine::collect(&config_dir);
            println!(" Fingerprint:   {}", specs.fingerprint);
            println!(" Hostname:      {}", specs.hostname);
            println!(" Specs:         {} cores, {} MB RAM ({})", specs.cpu_cores, specs.ram_mb, specs.os_info);
            println!(" Capabilities:  {:?}", specs.capabilities);
            println!(" Connecting to: {}", url);
            println!("============================================================");

            let client = EnrollmentClient::new();
            match client.enroll(&url, key.as_deref(), token, &config_dir).await {
                Ok(identity) => {
                    println!("\x1b[32m[SUCCESS] Workstation successfully enrolled with Tuquet Cloud!\x1b[0m");
                    println!(" Device ID:     {}", identity.device_id);
                    println!(" Tenant ID:     {}", identity.tenant_id);
                    println!(" Device Name:   {}", identity.name);
                    println!(" Identity Path: {}", config_dir.join(".identity.json").display());
                    println!("\n👉 Run 'tqr worker' to start processing cloud jobs.");
                }
                Err(e) => {
                    eprintln!("\x1b[31m[ERROR] Enrollment failed: {}\x1b[0m", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::Worker { server, token, id, tags } => {
            let loaded_identity = DeviceIdentity::load(&config_dir).ok().flatten();

            let runner_id = id
                .or_else(|| loaded_identity.as_ref().map(|i| i.device_id.clone()))
                .unwrap_or_else(|| {
                    let host = std::env::var("COMPUTERNAME")
                        .or_else(|_| std::env::var("HOSTNAME"))
                        .unwrap_or_else(|_| "node".to_string());
                    format!("{}-{}", host, &uuid::Uuid::new_v4().simple().to_string()[..8])
                });

            let auth_token = token
                .or_else(|| loaded_identity.as_ref().map(|i| i.device_token.clone()));

            let server_url = server
                .or_else(|| loaded_identity.as_ref().map(|i| {
                    let base = i.cloud_url.trim_end_matches('/');
                    let ws_base = if let Some(stripped) = base.strip_prefix("https://") {
                        format!("wss://{}", stripped)
                    } else if let Some(stripped) = base.strip_prefix("http://") {
                        format!("ws://{}", stripped)
                    } else {
                        base.to_string()
                    };
                    format!("{}/api/v1/runner/ws", ws_base)
                }))
                .unwrap_or_else(|| "wss://hub.tuquet.dev/api/v1/runner/ws".to_string());

            println!("============================================================");
            println!(" Starting Tuquet Runner (tqr) in Worker Daemon Mode");
            println!("============================================================");
            println!(" Runner ID:  {}", runner_id);
            println!(" Server:     {}", server_url);
            println!(" OS/Arch:    {}/{}", std::env::consts::OS, std::env::consts::ARCH);
            println!(" Tags:       {:?}", tags);
            println!(" Enrolled:   {}", if loaded_identity.is_some() { "YES (Cloud Managed)" } else { "NO (Local / Standalone)" });
            println!("============================================================");

            let config = WsClientConfig {
                server_url,
                token: auth_token,
                runner_id,
                tags,
            };

            let client = WsRunnerClient::new(config, engine);
            client.start().await;
        }
        Commands::Info => {
            let specs = FingerprintEngine::collect(&config_dir);
            let identity = DeviceIdentity::load(&config_dir).ok().flatten();

            println!("============================================================");
            println!(" Tuquet Runner (tqr) System Diagnostics");
            println!("============================================================");
            println!(" Version:      {}", env!("CARGO_PKG_VERSION"));
            println!(" Hostname:     {}", specs.hostname);
            println!(" Fingerprint:  {}", specs.fingerprint);
            println!(" Architecture: {}", specs.os_info);
            println!(" Resources:    {} CPU Cores | {} MB RAM", specs.cpu_cores, specs.ram_mb);
            println!(" Capabilities: {:?}", specs.capabilities);
            println!(" Supervision:  {}", if cfg!(windows) { "Win32 Job Objects (Kernel-Level Zero-Zombie)" } else { "POSIX Process Groups" });
            println!(" Config Path:  {}", config_dir.display());
            println!("------------------------------------------------------------");
            if let Some(id) = identity {
                println!(" Enrollment:   ENROLLED (Cloud Active)");
                println!(" Device ID:    {}", id.device_id);
                println!(" Tenant ID:    {}", id.tenant_id);
                println!(" Device Name:  {}", id.name);
                println!(" Cloud URL:    {}", id.cloud_url);
                println!(" Enrolled At:  {}", id.enrolled_at);
            } else {
                println!(" Enrollment:   NOT ENROLLED (Run 'tqr enroll' to connect)");
            }
            println!("============================================================");
        }
    }

    Ok(())
}
