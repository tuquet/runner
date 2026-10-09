use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "runner",
    version,
    about = "Ultra-fast universal distributed execution engine in Rust",
    long_about = "Executes AI agents, browser workflows, shell commands, and webhooks across distributed nodes."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum EnvCommands {
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
pub enum Commands {
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
    /// Enroll this machine with Specter Cloud (zero-touch device registration)
    Enroll {
        /// Target environment: local (default), dev, or prod
        #[arg(short, long, env = "SPECTER_ENV", default_value = "local")]
        env: String,

        /// Custom Specter Cloud / Supabase URL (overrides environment default)
        #[arg(short, long, env = "SPECTER_CLOUD_URL")]
        url: Option<String>,

        /// Custom Supabase publishable / anon key (overrides environment default)
        #[arg(short, long, env = "SPECTER_API_KEY")]
        key: Option<String>,

        /// Optional workspace enrollment token
        #[arg(short, long, env = "SPECTER_ENROLLMENT_TOKEN")]
        token: Option<String>,
    },
    /// Manage and switch environments (dev, local, prod)
    Env {
        #[command(subcommand)]
        action: EnvCommands,
    },
    /// Start in background worker daemon mode connecting outbound to Web Control Plane
    Worker {
        /// Web Control Plane WebSocket endpoint (e.g. wss://hub.specter.dev/api/v1/runner/ws)
        #[arg(short, long, env = "SPECTER_SERVER")]
        server: Option<String>,

        /// Authentication token for the runner
        #[arg(short, long, env = "SPECTER_TOKEN")]
        token: Option<String>,

        /// Unique runner node ID (defaults to enrolled device_id or hostname-uuid)
        #[arg(long, env = "SPECTER_RUNNER_ID")]
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

pub fn get_config_dir() -> PathBuf {
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
    if let Ok(dir) = std::env::var(runner::constants::ENV_SPECTER_HOME) {
        if !dir.trim().is_empty() {
            return PathBuf::from(dir).join("config");
        }
    }
    if let Ok(user_home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
        return PathBuf::from(user_home).join(runner::constants::DEFAULT_SSOT_DIR_NAME).join("config");
    }
    PathBuf::from("config")
}
