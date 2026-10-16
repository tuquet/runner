use crate::core::supervisor::{AsyncCommandGroup, ProcessSupervisor};
use crate::drivers::{ExecutionContext, ExecutionDriver};
use crate::protocol::handshake::AutomaManifest;
use crate::protocol::schema::{AutomaJobOutput, AutomaJobPayload, Job, JobResult, LogChannel};
use async_trait::async_trait;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::warn;

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
        let mut candidates = vec![
            format!("{}/.specter/bin/{}", home_dir, binary_name),
            format!("/usr/local/bin/{}", binary_name),
            format!("target/release/{}", binary_name),
            format!("target/debug/{}", binary_name),
            format!("../target/release/{}", binary_name),
            format!("../target/debug/{}", binary_name),
            format!("../cli/target/release/{}", binary_name),
            format!("../cli/target/debug/{}", binary_name),
        ];

        if let Ok(exe_path) = std::env::current_exe() {
            if let Some(exe_dir) = exe_path.parent() {
                candidates.insert(0, exe_dir.join(binary_name).to_string_lossy().to_string());
            }
        }

        if let Ok(target_dir) = std::env::var("CARGO_TARGET_DIR") {
            candidates.push(format!("{}/release/{}", target_dir, binary_name));
            candidates.push(format!("{}/debug/{}", target_dir, binary_name));
        }

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

    /// Executes an Automa workflow directly in-process via Native Rust CDP `WorkflowDagExecutor`
    pub async fn execute_native_cdp(
        &self,
        job_id: &str,
        automa_payload: &AutomaJobPayload,
        raw_payload: &serde_json::Value,
        ctx: &ExecutionContext,
        start_time: Instant,
    ) -> Option<Result<JobResult, String>> {
        let port_opt = raw_payload
            .get("debuggingPort")
            .or_else(|| raw_payload.get("debugging_port"))
            .and_then(|v| v.as_u64())
            .map(|p| p as u16);

        let user_data_dir_opt = raw_payload
            .get("userDataDir")
            .or_else(|| raw_payload.get("user_data_dir"))
            .and_then(|v| v.as_str())
            .map(std::path::PathBuf::from);

        if port_opt.is_none() && user_data_dir_opt.is_none() {
            return None;
        }

        let wf_cfg = automa_payload.resolved_workflow_config();
        let wf_val = if let Some(data) = wf_cfg.data {
            data
        } else if !wf_cfg.path.is_empty() {
            match tokio::fs::read_to_string(&wf_cfg.path).await {
                Ok(content) => match serde_json::from_str(&content) {
                    Ok(v) => v,
                    Err(e) => return Some(Err(format!("Failed to parse workflow JSON '{}': {}", wf_cfg.path, e))),
                },
                Err(e) => return Some(Err(format!("Failed to read workflow file '{}': {}", wf_cfg.path, e))),
            }
        } else {
            return None;
        };

        let graph = match crate::drivers::cdp::WorkflowGraph::from_json(&wf_val) {
            Ok(g) => g,
            Err(e) => return Some(Err(format!("Invalid workflow graph: {}", e))),
        };

        ctx.emit_log(
            LogChannel::System,
            "Connecting to Antidetect Chromium via native CDP (Zero-Extension Stealth)...",
        );

        let cdp_session = match crate::drivers::cdp::CdpSession::connect_auto(
            port_opt,
            user_data_dir_opt.as_deref(),
        )
        .await
        {
            Ok(s) => std::sync::Arc::new(s),
            Err(e) => return Some(Err(format!("Failed to connect CDP session: {}", e))),
        };

        let mut initial_state = crate::drivers::cdp::ExecutionState::new();
        if let Some(vars) = wf_cfg.variables {
            for (k, v) in vars {
                initial_state.set_variable(&k, v);
            }
        }

        let executor = crate::drivers::cdp::WorkflowDagExecutor::new(graph, cdp_session);
        match executor.execute(Some(initial_state), ctx.cancel_token.clone()).await {
            Ok(final_state) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                for step in &final_state.step_logs {
                    ctx.emit_log(
                        LogChannel::Stdout,
                        &format!("[{}] {} ({}ms)", step.status, step.name, step.duration_ms.unwrap_or(0)),
                    );
                }

                let tables: Vec<serde_json::Value> = final_state
                    .table
                    .into_iter()
                    .map(serde_json::Value::Object)
                    .collect();

                let out = AutomaJobOutput {
                    workflow_id: if wf_cfg.path.is_empty() { None } else { Some(wf_cfg.path) },
                    status: "completed".to_string(),
                    duration_ms,
                    tables: Some(tables),
                    variables: Some(final_state.variables),
                    step_logs: Some(final_state.step_logs),
                    artifacts: None,
                };
                let out_json = serde_json::to_value(out).ok();
                Some(Ok(JobResult::success(job_id.to_string(), duration_ms, out_json)))
            }
            Err(e) if e.contains("cancelled") => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                Some(Ok(JobResult::cancelled(job_id.to_string(), duration_ms)))
            }
            Err(e) => {
                let duration_ms = start_time.elapsed().as_millis() as u64;
                Some(Ok(JobResult::failure(job_id.to_string(), Some(1), duration_ms, e)))
            }
        }
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

        // 1b. If direct CDP port or user_data_dir is provided, execute in-process via WorkflowDagExecutor
        if let Some(res) = self
            .execute_native_cdp(job.id.as_str(), &automa_payload, &job.payload, &ctx, start_time)
            .await
        {
            return res;
        }

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
                    protocol: crate::constants::PROTOCOL_AUTOMA_LEGACY.to_string(),
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
