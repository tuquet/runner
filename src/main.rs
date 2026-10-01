use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tuquet_runner::core::engine::RunnerEngine;
use tuquet_runner::core::enrollment::EnrollmentClient;
use tuquet_runner::core::environments::{EnvironmentConfig, EnvironmentRegistry};
use tuquet_runner::core::fingerprint::FingerprintEngine;
use tuquet_runner::core::identity::DeviceIdentity;
use tuquet_runner::protocol::schema::{DriverType, Job, JobStatus};
use tuquet_runner::transport::local_channel::LocalRunner;
use tuquet_runner::transport::ws_client::{WsClientConfig, WsRunnerClient};

#[derive(Parser)]
#[command(
    name = "tuquet-runner",
    version,
    about = "Ultra-fast universal distributed execution engine in Rust",
    long_about = "Executes AI agents, browser workflows, shell commands, and webhooks across distributed nodes."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum EnvCommands {
    /// List all configured environments and show active environment
    List,
    /// Switch active environment, purge old identity, and enroll with target environment
    Switch {
        /// Target environment: dev, local, or prod
        name: String,
    },
    /// Configure or override an environment endpoint
    Set {
        /// Environment name (e.g. prod, staging)
        name: String,
        /// Supabase or Cloud REST URL
        #[arg(long)]
        url: String,
        /// Supabase publishable / anon key
        #[arg(long)]
        key: String,
        /// Human-friendly label
        #[arg(long)]
        label: Option<String>,
    },
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
        /// Target environment: local (default), dev, or prod
        #[arg(short, long, env = "TUQUET_ENV", default_value = "local")]
        env: String,

        /// Custom Tuquet Cloud / Supabase URL (overrides environment default)
        #[arg(short, long, env = "TUQUET_CLOUD_URL")]
        url: Option<String>,

        /// Custom Supabase publishable / anon key (overrides environment default)
        #[arg(short, long, env = "TUQUET_API_KEY")]
        key: Option<String>,

        /// Optional workspace enrollment token
        #[arg(short, long, env = "TUQUET_ENROLLMENT_TOKEN")]
        token: Option<String>,
    },
    /// Manage and switch environments (dev, local, prod)
    Env {
        #[command(subcommand)]
        action: EnvCommands,
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
    /// Reset local device enrollment credentials by deleting .identity.json
    Purge,
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
                "automa" => DriverType::Automa,
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
        Commands::Enroll { env, url, key, token } => {
            let env_config = EnvironmentRegistry::get(&env, &config_dir);
            let target_url = url.or_else(|| env_config.as_ref().map(|c| c.url.clone()))
                .unwrap_or_default();
            let target_key = key.or_else(|| env_config.as_ref().and_then(|c| c.api_key.clone()));

            if target_url.is_empty() {
                eprintln!("\x1b[31m[ERROR] Environment '{}' has no URL configured.\x1b[0m", env);
                if env.to_lowercase() == "prod" {
                    eprintln!("Configure production endpoint first: tqr env set prod --url <URL> --key <KEY>");
                }
                std::process::exit(1);
            }

            println!("============================================================");
            println!(" Tuquet Runner (tqr) - Device Enrollment");
            println!("============================================================");
            let specs = FingerprintEngine::collect(&config_dir);
            println!(" Environment:   {} ({})", env.to_uppercase(), env_config.as_ref().map(|c| c.label.as_str()).unwrap_or("Custom"));
            println!(" Fingerprint:   {}", specs.fingerprint);
            println!(" Hostname:      {}", specs.hostname);
            println!(" Specs:         {} cores, {} MB RAM ({})", specs.cpu_cores, specs.ram_mb, specs.os_info);
            println!(" Capabilities:  {:?}", specs.capabilities);
            println!(" Connecting to: {}", target_url);
            println!("============================================================");

            let client = EnrollmentClient::new();
            match client.enroll(&target_url, target_key.as_deref(), token, &env, &config_dir).await {
                Ok(identity) => {
                    println!("\x1b[32m[SUCCESS] Workstation successfully enrolled with Tuquet Cloud!\x1b[0m");
                    println!(" Environment:   {}", identity.env.to_uppercase());
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
        Commands::Env { action } => {
            let loaded_identity = DeviceIdentity::load(&config_dir).ok().flatten();
            let current_env = loaded_identity.as_ref().map(|i| i.env.to_lowercase()).unwrap_or_default();

            match action {
                EnvCommands::List => {
                    println!("================================================================================");
                    println!(" Tuquet Runner (tqr) Environments");
                    println!("================================================================================");
                    println!(" {:<8} {:<10} {:<38} DESCRIPTION", "STATUS", "ENV", "URL");
                    println!("--------------------------------------------------------------------------------");

                    for e in EnvironmentRegistry::list(&config_dir) {
                        let is_active = e.name.to_lowercase() == current_env;
                        let status_str = if is_active {
                            "\x1b[32m* ACTIVE\x1b[0m"
                        } else {
                            "  IDLE  "
                        };
                        let url_display = if e.url.is_empty() {
                            "<not configured>"
                        } else {
                            &e.url
                        };
                        println!(" {:<17} {:<10} {:<38} {}", status_str, e.name.to_uppercase(), url_display, e.label);
                    }
                    println!("================================================================================");
                    if current_env.is_empty() {
                        println!("👉 Run 'tqr env switch <dev|local>' to connect.");
                    } else {
                        println!("👉 Active environment: \x1b[32m{}\x1b[0m. Switch anytime with: tqr env switch <target>", current_env.to_uppercase());
                    }
                }
                EnvCommands::Switch { name } => {
                    let target_name = name.to_lowercase();
                    let target_env = EnvironmentRegistry::get(&target_name, &config_dir);

                    let env_cfg = match target_env {
                        Some(cfg) if !cfg.url.is_empty() => cfg,
                        Some(_) => {
                            eprintln!("\x1b[31m[ERROR] Environment '{}' has not been configured with a URL yet.\x1b[0m", target_name);
                            eprintln!("Use 'tqr env set {} --url <URL> --key <KEY>' first.", target_name);
                            std::process::exit(1);
                        }
                        None => {
                            eprintln!("\x1b[31m[ERROR] Unknown environment: '{}'. Available: dev, local, prod\x1b[0m", target_name);
                            std::process::exit(1);
                        }
                    };

                    println!("============================================================");
                    println!(" Tuquet Runner (tqr) - Switching Environment");
                    println!("============================================================");
                    if !current_env.is_empty() {
                        println!(" Current Environment: {}", current_env.to_uppercase());
                    }
                    println!(" Target Environment:  {} ({})", env_cfg.name.to_uppercase(), env_cfg.url);
                    println!(" Purging old credentials...");
                    let _ = DeviceIdentity::purge(&config_dir);

                    println!(" Enrolling into {}...", env_cfg.name.to_uppercase());
                    let client = EnrollmentClient::new();
                    match client.enroll(&env_cfg.url, env_cfg.api_key.as_deref(), None, &env_cfg.name, &config_dir).await {
                        Ok(identity) => {
                            println!("\x1b[32m[SUCCESS] Successfully switched and enrolled into {}!\x1b[0m", env_cfg.name.to_uppercase());
                            println!(" Environment:   {}", identity.env.to_uppercase());
                            println!(" Device ID:     {}", identity.device_id);
                            println!(" Cloud URL:     {}", identity.cloud_url);
                            println!(" Identity Path: {}", config_dir.join(".identity.json").display());
                            println!("\n👉 Run 'tqr worker' to start processing jobs on this environment.");
                        }
                        Err(e) => {
                            eprintln!("\x1b[31m[ERROR] Enrollment into '{}' failed: {}\x1b[0m", env_cfg.name, e);
                            std::process::exit(1);
                        }
                    }
                }
                EnvCommands::Set { name, url, key, label } => {
                    let target_name = name.to_lowercase();
                    let lbl = label.unwrap_or_else(|| format!("Custom {} environment", target_name));
                    let cfg = EnvironmentConfig {
                        name: target_name.clone(),
                        label: lbl,
                        url: url.clone(),
                        api_key: Some(key),
                    };

                    match EnvironmentRegistry::set(cfg, &config_dir) {
                        Ok(_) => {
                            println!("\x1b[32m[SUCCESS] Environment '{}' configured successfully!\x1b[0m", target_name.to_uppercase());
                            println!(" URL: {}", url);
                            println!("Run 'tqr env switch {}' to switch to it.", target_name);
                        }
                        Err(e) => {
                            eprintln!("\x1b[31m[ERROR] Failed to save environment: {}\x1b[0m", e);
                            std::process::exit(1);
                        }
                    }
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
            println!(" Runner ID:   {}", runner_id);
            println!(" Server:      {}", server_url);
            println!(" OS/Arch:     {}/{}", std::env::consts::OS, std::env::consts::ARCH);
            println!(" Tags:        {:?}", tags);
            println!(" Environment: {}", loaded_identity.as_ref().map(|i| i.env.to_uppercase()).unwrap_or_else(|| "STANDALONE".to_string()));
            println!("============================================================");

            // Spawn cloud heartbeat task if device identity is present
            if let Some(ref ident) = loaded_identity {
                let cloud_url = ident.cloud_url.clone();
                let api_key = ident.api_key.clone();
                let device_id = ident.device_id.clone();
                let device_token = ident.device_token.clone();
                let env_name = ident.env.clone();
                let config_dir_clone = config_dir.clone();
                let engine_clone = engine.clone();

                tokio::spawn(async move {
                    let client = EnrollmentClient::new();
                    let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));

                    loop {
                        interval.tick().await;
                        let active = engine_clone.active_jobs_count();
                        let telemetry = serde_json::json!({
                            "timestamp": chrono::Utc::now().to_rfc3339(),
                            "version": env!("CARGO_PKG_VERSION")
                        });

                        match client.heartbeat(&cloud_url, api_key.as_deref(), &device_id, &device_token, active, telemetry).await {
                            Ok(true) => {
                                tracing::debug!("Cloud heartbeat sent successfully");
                            }
                            Ok(false) => {
                                eprintln!("\n\x1b[33m[REVOCATION DETECTED] Cloud device identity {} was removed from runners.devices!\x1b[0m", device_id);
                                eprintln!("Purging local credentials and initiating self-healing re-enrollment...");
                                let _ = DeviceIdentity::purge(&config_dir_clone);
                                match client.enroll(&cloud_url, api_key.as_deref(), None, &env_name, &config_dir_clone).await {
                                    Ok(new_id) => {
                                        println!("\x1b[32m[SELF-HEALING RECOVERED] Fresh Device ID assigned: {}\x1b[0m", new_id.device_id);
                                    }
                                    Err(e) => {
                                        eprintln!("\x1b[31m[SELF-HEALING FAILED] Re-enrollment failed: {}\x1b[0m", e);
                                    }
                                }
                                break;
                            }
                            Err(e) => {
                                tracing::warn!("Cloud heartbeat transient error: {}", e);
                            }
                        }
                    }
                });
            }

            let config = WsClientConfig {
                server_url,
                token: auth_token,
                runner_id,
                tags,
            };

            let client = WsRunnerClient::new(config, engine);
            client.start().await;
        }
        Commands::Purge => {
            match DeviceIdentity::purge(&config_dir) {
                Ok(_) => {
                    println!("\x1b[32m[SUCCESS] Local device identity purged from {}\x1b[0m", config_dir.join(".identity.json").display());
                    println!("Workstation is now in a clean, unenrolled state. Run 'tqr env switch <dev|local>' to re-register.");
                }
                Err(e) => {
                    eprintln!("\x1b[31m[ERROR] Failed to purge identity: {}\x1b[0m", e);
                    std::process::exit(1);
                }
            }
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
                println!(" Enrollment:   ENROLLED");
                println!(" Environment:  {} ({})", id.env.to_uppercase(), id.cloud_url);
                println!(" Device ID:    {}", id.device_id);
                println!(" Tenant ID:    {}", id.tenant_id);
                println!(" Device Name:  {}", id.name);
                println!(" Enrolled At:  {}", id.enrolled_at);
            } else {
                println!(" Enrollment:   NOT ENROLLED (Run 'tqr env switch <dev|local>' to connect)");
            }
            println!("============================================================");
        }
    }

    Ok(())
}
