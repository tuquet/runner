use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use runner::core::engine::RunnerEngine;
use runner::core::Notify;
use runner::protocol::schema::{DriverType, Job, JobStatus};
use runner::transport::local_channel::LocalRunner;

pub async fn handle_run(
    file: PathBuf,
    engine: Arc<RunnerEngine>,
) -> Result<(), Box<dyn std::error::Error>> {
    if !file.exists() {
        Notify::error(format!("File '{}' not found.", file.display()));
        std::process::exit(1);
    }

    let content = fs::read_to_string(&file)?;
    let job: Job = serde_json::from_str(&content)?;

    let local_runner = LocalRunner::new(engine);
    let result = tokio::select! {
        res = local_runner.run_job(job) => res,
        _ = tokio::signal::ctrl_c() => {
            Notify::cancelled(runner::constants::MSG_JOB_CANCELLED);
            tracing::info!("Received Ctrl+C interrupt. Exiting job execution cleanly.");
            std::process::exit(130);
        }
    };

    if result.status == JobStatus::Completed {
        Notify::success(format!("Job completed in {}ms", result.duration_ms));
        std::process::exit(0);
    } else {
        Notify::failed(format!(
            "Job failed with status {:?} ({}ms): {:?}",
            result.status, result.duration_ms, result.error
        ));
        std::process::exit(result.exit_code.unwrap_or(1));
    }
}

pub async fn handle_exec(
    driver: String,
    command: Option<String>,
    prompt: Option<String>,
    cwd: Option<String>,
    timeout: u64,
    engine: Arc<RunnerEngine>,
) -> Result<(), Box<dyn std::error::Error>> {
    let driver_type = match driver.as_str() {
        "shell" => DriverType::Shell,
        "agent" => DriverType::Agent,
        "automa" => DriverType::Automa,
        "browser" => DriverType::Browser,
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
    let result = tokio::select! {
        res = local_runner.run_job(job) => res,
        _ = tokio::signal::ctrl_c() => {
            Notify::cancelled("Received Ctrl+C signal. Safely terminating ad-hoc execution...");
            tracing::info!("Received Ctrl+C interrupt. Exiting ad-hoc execution cleanly.");
            std::process::exit(130);
        }
    };

    if result.status == JobStatus::Completed {
        Notify::success(format!("Execution finished in {}ms", result.duration_ms));
        std::process::exit(0);
    } else {
        Notify::failed(format!("Execution failed ({:?}): {:?}", result.status, result.error));
        std::process::exit(result.exit_code.unwrap_or(1));
    }
}
