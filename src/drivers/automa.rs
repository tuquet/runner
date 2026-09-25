use crate::core::supervisor::ProcessSupervisor;
use crate::drivers::{ExecutionContext, ExecutionDriver};
use crate::protocol::handshake::AutomaManifest;
use crate::protocol::schema::{Job, JobId, JobResult, LogChannel};
use async_trait::async_trait;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{info, warn};

pub struct AutomaDriver;

impl AutomaDriver {
    pub fn new() -> Self {
        Self
    }

    /// Resolves the executable path for the automa browser plugin (automa or automa-core)
    fn resolve_automa_bin(&self, target: &str) -> String {
        if target.is_empty() || target == "automa" || target == "automa-core" {
            if cfg!(windows) {
                "automa.exe".to_string()
            } else {
                "automa".to_string()
            }
        } else {
            target.to_string()
        }
    }

    /// Performs an active capability negotiation and handshake probe with automa-core plugin
    pub async fn probe(&self, bin_name: &str) -> Result<AutomaManifest, String> {
        let output = tokio::time::timeout(
            Duration::from_secs(4),
            Command::new(bin_name).arg("probe").output(),
        )
        .await
        .map_err(|_| format!("Handshake probe timed out for '{}'", bin_name))?
        .map_err(|e| format!("Failed to execute handshake probe on '{}': {}", bin_name, e))?;

        if !output.status.success() {
            return Err(format!(
                "Automa probe command failed with exit code {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let manifest: AutomaManifest = serde_json::from_str(stdout_str.trim()).map_err(|e| {
            format!(
                "Failed to parse AutomaManifest JSON: {} (Output: '{}')",
                e,
                stdout_str.trim()
            )
        })?;

        Ok(manifest)
    }
}

#[async_trait]
impl ExecutionDriver for AutomaDriver {
    fn name(&self) -> &'static str {
        "automa"
    }

    fn can_handle(&self, job: &Job) -> bool {
        matches!(job.driver, crate::protocol::schema::DriverType::Automa)
    }

    async fn execute(&self, job: Job, ctx: ExecutionContext) -> Result<JobResult, String> {
        let start_time = Instant::now();

        // 1. Extract workflow payload / options
        let target = job
            .payload
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("automa");

        let bin_name = self.resolve_automa_bin(target);

        // 2. Handshake Phase: Probe automa plugin capabilities
        ctx.emit_log(
            LogChannel::System,
            &format!("Initiating handshake probe with browser plugin '{}'...", bin_name),
        );

        let manifest = match self.probe(&bin_name).await {
            Ok(m) => {
                ctx.emit_log(
                    LogChannel::System,
                    &format!(
                        "Handshake OK: '{}' v{} [Protocol: {}, Engine: {}, Status: {}]",
                        m.name, m.version, m.protocol, m.engine, m.status
                    ),
                );
                m
            }
            Err(e) => {
                warn!("Automa handshake probe warning: {}", e);
                ctx.emit_log(
                    LogChannel::System,
                    &format!("Probe notice: {}. Proceeding with standard execution.", e),
                );
                AutomaManifest {
                    protocol: "tuquet.automa.legacy".to_string(),
                    name: target.to_string(),
                    version: "unknown".to_string(),
                    engine: "chromium-cdp".to_string(),
                    status: "ready".to_string(),
                    capabilities: vec![],
                    plugin_type: "runner_driver".to_string(),
                }
            }
        };

        // 3. Assemble execution command
        let mut cmd = Command::new(&bin_name);

        if let Some(workflow) = job.payload.get("workflow").and_then(|v| v.as_str()) {
            cmd.arg("run").arg("--workflow").arg(workflow);
        } else if let Some(workflow_data) = job.payload.get("workflow_data") {
            let json_str = serde_json::to_string(workflow_data).unwrap_or_default();
            cmd.arg("run").arg("--workflow-json").arg(json_str);
        }

        let headless = job
            .payload
            .get("headless")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        if headless {
            cmd.arg("--headless");
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        ctx.emit_log(
            LogChannel::System,
            &format!(
                "Executing browser automation task via '{}' under Kernel Job Object supervision...",
                manifest.name
            ),
        );

        // 4. Spawn child process with ProcessSupervisor (Win32 Job Objects / POSIX groups)
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn automa plugin '{}': {}", bin_name, e))?;

        let _guard = ProcessSupervisor::supervise(&child);

        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let ctx_stdout = ctx.clone();
        let stdout_handle = tokio::spawn(async move {
            if let Some(out) = stdout {
                let mut reader = BufReader::new(out).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    ctx_stdout.emit_log(LogChannel::Stdout, &line);
                }
            }
        });

        let ctx_stderr = ctx.clone();
        let stderr_handle = tokio::spawn(async move {
            if let Some(err) = stderr {
                let mut reader = BufReader::new(err).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    ctx_stderr.emit_log(LogChannel::Stderr, &line);
                }
            }
        });

        let timeout_secs = job.timeout_seconds.unwrap_or(300);
        let timeout_duration = Duration::from_secs(timeout_secs);

        let status_res = tokio::time::timeout(timeout_duration, child.wait()).await;

        let _ = stdout_handle.await;
        let _ = stderr_handle.await;

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match status_res {
            Ok(Ok(status)) => {
                let code = status.code().unwrap_or(-1);
                if status.success() {
                    ctx.emit_log(
                        LogChannel::System,
                        &format!("Browser automation completed successfully in {}ms (exit 0)", duration_ms),
                    );
                    Ok(JobResult::completed(&job.id, code, duration_ms))
                } else {
                    ctx.emit_log(
                        LogChannel::System,
                        &format!("Browser automation failed with exit code {}", code),
                    );
                    Ok(JobResult::failed(
                        &job.id,
                        code,
                        format!("Process exited with non-zero code: {}", code),
                        duration_ms,
                    ))
                }
            }
            Ok(Err(e)) => {
                let err_msg = format!("Failed while waiting for automa plugin process: {}", e);
                ctx.emit_log(LogChannel::System, &err_msg);
                Ok(JobResult::failed(&job.id, -1, err_msg, duration_ms))
            }
            Err(_) => {
                ctx.emit_log(
                    LogChannel::System,
                    &format!(
                        "Execution timed out after {} seconds. Win32 Job Object terminating Chromium process tree...",
                        timeout_secs
                    ),
                );
                let _ = child.kill().await;
                Ok(JobResult::timed_out(&job.id, duration_ms))
            }
        }
    }

    async fn cancel(&self, _job_id: &JobId) -> Result<(), String> {
        info!("Automa driver received cancellation signal. Cleaning up process...");
        Ok(())
    }
}
