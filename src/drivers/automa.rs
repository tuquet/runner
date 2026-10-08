use crate::core::supervisor::{AsyncCommandGroup, ProcessSupervisor};
use crate::drivers::{ExecutionContext, ExecutionDriver};
use crate::protocol::handshake::AutomaManifest;
use crate::protocol::schema::{AutomaJobOutput, AutomaJobPayload, Job, JobId, JobResult, LogChannel};
use async_trait::async_trait;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{info, warn};

pub struct AutomaDriver;

impl Default for AutomaDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl AutomaDriver {
    pub fn new() -> Self {
        Self
    }

    /// Resolves the executable path for the automa browser plugin (specter, automa, automa-core)
    fn resolve_automa_bin(&self, target: &str) -> String {
        let binary_name = if target.is_empty() || target == "specter" || target == "automa" || target == "automa-core" {
            let primary = if cfg!(windows) { "specter.exe" } else { "specter" };
            let fallback = if cfg!(windows) { "automa.exe" } else { "automa" };
            if target == "automa" || target == "automa-core" {
                fallback
            } else {
                primary
            }
        } else {
            target
        };

        if std::path::Path::new(binary_name).exists() {
            return binary_name.to_string();
        }

        let home_dir = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).unwrap_or_default();
        let candidates = [
            format!("/usr/local/bin/{}", binary_name),
            format!("{}/.specter/bin/{}", home_dir, binary_name),
            format!("{}/Repository/tuquet/cli/target/release/{}", home_dir, binary_name),
            format!("{}/Repository/tuquet/cli/target/debug/{}", home_dir, binary_name),
            format!("../cli/target/release/{}", binary_name),
            format!("../cli/target/debug/{}", binary_name),
        ];

        for cand in candidates {
            if std::path::Path::new(&cand).exists() {
                return cand;
            }
        }

        binary_name.to_string()
    }

    /// Performs an active capability negotiation and handshake probe with automa-core plugin
    pub async fn probe(&self, bin_name: &str) -> Result<AutomaManifest, String> {
        let mut cmd = Command::new(bin_name);
        if bin_name.contains("specter") {
            cmd.arg("automa").arg("probe");
        } else {
            cmd.arg("probe");
        }

        let output = tokio::time::timeout(
            Duration::from_secs(4),
            cmd.output(),
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

        // 1. Extract typed Automa payload with full fallback to loose JSON
        let automa_payload: AutomaJobPayload = serde_json::from_value(job.payload.clone()).unwrap_or_default();
        let target = automa_payload.target.as_deref().unwrap_or("automa");
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
                    protocol: "specter.automa.legacy".to_string(),
                    name: target.to_string(),
                    version: "unknown".to_string(),
                    engine: "chromium-cdp".to_string(),
                    status: "ready".to_string(),
                    capabilities: vec![],
                    plugin_type: "runner_driver".to_string(),
                    requires_browser: Some(true),
                    supported_browsers: Some(vec!["chromium".to_string()]),
                }
            }
        };

        // 3. Assemble execution command using typed AutomaJobPayload & resolved BrowserConfig
        let mut cmd = Command::new(&bin_name);
        if bin_name.contains("specter") {
            cmd.arg("automa");
        }

        let cli_args = automa_payload.build_cli_args();
        for arg in cli_args {
            cmd.arg(arg);
        }

        let browser_cfg = automa_payload.resolved_browser_config();
        // Configure proxy routing if specified
        if let Some(ref proxy) = browser_cfg.proxy {
            cmd.env("ALL_PROXY", &proxy.server);
            cmd.env("HTTP_PROXY", &proxy.server);
            cmd.env("HTTPS_PROXY", &proxy.server);
        }

        // Pass antidetect fingerprint seed if specified
        if let Some(ref fp) = browser_cfg.fingerprint {
            if let Some(seed) = fp.seed {
                cmd.arg("--fingerprint").arg(seed.to_string());
            }
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

        // 4. Spawn child process group with ProcessSupervisor (Win32 Job Objects / POSIX groups)
        let mut child = cmd
            .group_spawn()
            .map_err(|e| format!("Failed to spawn automa plugin '{}': {}", bin_name, e))?;

        let stdout = child.inner().stdout.take();
        let stderr = child.inner().stderr.take();

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

        // 5. Supervise process with Win32 Job Object, POSIX Process Groups, Timeout & Cancellation
        let mut supervisor = ProcessSupervisor::new();
        let timeout_duration = Duration::from_millis(job.timeout_ms);

        let wait_result = supervisor
            .wait_with_timeout_or_cancel(&mut child, timeout_duration, Some(&ctx.cancel_token))
            .await;

        // Drain remaining stream outputs with timeout to prevent pipe deadlock
        let stdout_abort = stdout_handle.abort_handle();
        let stderr_abort = stderr_handle.abort_handle();
        if tokio::time::timeout(
            Duration::from_millis(1500),
            async {
                let _ = tokio::join!(stdout_handle, stderr_handle);
            },
        )
        .await
        .is_err()
        {
            stdout_abort.abort();
            stderr_abort.abort();
        }

        let duration_ms = start_time.elapsed().as_millis() as u64;

        match wait_result {
            Ok(status) => {
                let code = status.code().unwrap_or(-1);
                if status.success() {
                    ctx.emit_log(
                        LogChannel::System,
                        &format!("Browser automation completed successfully in {}ms (exit 0)", duration_ms),
                    );

                    let wf_cfg = automa_payload.resolved_workflow_config();
                    let out = AutomaJobOutput {
                        workflow_id: if wf_cfg.path.is_empty() { None } else { Some(wf_cfg.path) },
                        status: "completed".to_string(),
                        duration_ms,
                        tables: None,
                        variables: wf_cfg.variables.or_else(|| automa_payload.variables.clone()),
                        step_logs: None,
                        artifacts: None,
                    };
                    let out_json = serde_json::to_value(out).ok();

                    Ok(JobResult::success(job.id, duration_ms, out_json))
                } else {
                    ctx.emit_log(
                        LogChannel::System,
                        &format!("Browser automation failed with exit code {}", code),
                    );
                    Ok(JobResult::failure(
                        job.id,
                        Some(code),
                        duration_ms,
                        format!("Process exited with non-zero code: {}", code),
                    ))
                }
            }
            Err(e) if e == "Execution timed out" => {
                ctx.emit_log(
                    LogChannel::System,
                    &format!(
                        "Execution timed out after {} ms. Win32 Job Object terminating Chromium process tree...",
                        job.timeout_ms
                    ),
                );
                Ok(JobResult::timed_out(job.id, duration_ms))
            }
            Err(e) if e == "Execution cancelled" => {
                ctx.emit_log(
                    LogChannel::System,
                    "Browser automation cancelled by control plane. Process tree terminated.",
                );
                Ok(JobResult::cancelled(job.id, duration_ms))
            }
            Err(e) => {
                let err_msg = format!("Failed while waiting for automa plugin process: {}", e);
                ctx.emit_log(LogChannel::System, &err_msg);
                Ok(JobResult::failure(job.id, None, duration_ms, err_msg))
            }
        }
    }

    async fn cancel(&self, _job_id: &JobId) -> Result<(), String> {
        info!("Automa driver received cancellation signal. Cleaning up process...");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::schema::DriverType;

    #[test]
    fn test_automa_driver_can_handle() {
        let driver = AutomaDriver::new();
        let job = Job {
            id: "test-automa-1".to_string(),
            driver: DriverType::Automa,
            payload: serde_json::json!({}),
            cwd: None,
            env: None,
            timeout_ms: 10_000,
            created_at: None,
        };
        assert!(driver.can_handle(&job));
        assert_eq!(driver.name(), "automa");
    }

    #[test]
    fn test_resolve_automa_bin() {
        let driver = AutomaDriver::new();
        let bin = driver.resolve_automa_bin("specter");
        assert!(!bin.is_empty());
        // Verify custom target returns custom name
        assert_eq!(driver.resolve_automa_bin("/custom/bin/runner"), "/custom/bin/runner");
    }
}
