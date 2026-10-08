use crate::core::supervisor::{AsyncCommandGroup, ProcessSupervisor};
use crate::drivers::{ExecutionContext, ExecutionDriver};
use crate::protocol::handshake::{AgentAuthStatus, AgentManifest};
use crate::protocol::schema::{Job, JobId, JobResult, LogChannel};
use async_trait::async_trait;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{info, warn};

pub struct AgentDriver;

impl Default for AgentDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentDriver {
    pub fn new() -> Self {
        Self
    }

    /// Resolves the executable path for the agent runner (claude-agy or claude)
    fn resolve_agent_bin(&self, target: &str) -> String {
        if target.is_empty() || target == "claude-agy" {
            if cfg!(windows) {
                "claude-agy.cmd".to_string()
            } else {
                "claude-agy".to_string()
            }
        } else {
            target.to_string()
        }
    }

    /// Performs an active capability negotiation and authentication health probe
    pub async fn probe(&self, bin_name: &str) -> Result<AgentManifest, String> {
        let output = tokio::time::timeout(
            Duration::from_secs(4),
            Command::new(bin_name).arg("probe").output(),
        )
        .await
        .map_err(|_| format!("Handshake probe timed out for '{}'", bin_name))?
        .map_err(|e| format!("Failed to execute handshake probe on '{}': {}", bin_name, e))?;

        if !output.status.success() {
            return Err(format!(
                "Agent probe command failed with exit code {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let manifest: AgentManifest = serde_json::from_str(stdout_str.trim()).map_err(|e| {
            format!(
                "Failed to parse AgentManifest JSON: {} (Output: '{}')",
                e,
                stdout_str.trim()
            )
        })?;

        Ok(manifest)
    }
}

#[async_trait]
impl ExecutionDriver for AgentDriver {
    fn name(&self) -> &'static str {
        "agent"
    }

    fn can_handle(&self, job: &Job) -> bool {
        matches!(job.driver, crate::protocol::schema::DriverType::Agent)
    }

    async fn execute(&self, job: Job, ctx: ExecutionContext) -> Result<JobResult, String> {
        let start_time = Instant::now();

        // 1. Extract agent parameters from payload
        let prompt = job
            .payload
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Agent driver requires 'prompt' string in payload".to_string())?;

        let target = job
            .payload
            .get("target")
            .and_then(|v| v.as_str())
            .unwrap_or("claude-agy");

        let bypass_permissions = job
            .payload
            .get("bypass_permissions")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let model = job.payload.get("model").and_then(|v| v.as_str());
        let token_path = job.payload.get("token_path").and_then(|v| v.as_str());

        let bin_name = self.resolve_agent_bin(target);

        // 2. Handshake Phase: Probe agent capabilities & auth health
        ctx.emit_log(
            LogChannel::System,
            &format!("Initiating handshake probe with agent '{}'...", bin_name),
        );

        let manifest = match self.probe(&bin_name).await {
            Ok(m) => {
                ctx.emit_log(
                    LogChannel::System,
                    &format!(
                        "Handshake OK: '{}' v{} [Protocol: {}, Account: {}, Proxy: {}]",
                        m.name, m.version, m.protocol, m.auth.email, m.proxy_ready
                    ),
                );
                m
            }
            Err(e) => {
                warn!("Handshake probe warning: {}", e);
                ctx.emit_log(
                    LogChannel::System,
                    &format!("Probe notice: {}. Proceeding with standard execution.", e),
                );
                // Fallback manifest for compatibility
                AgentManifest {
                    protocol: crate::constants::PROTOCOL_AGENT_LEGACY.to_string(),
                    name: target.to_string(),
                    version: "unknown".to_string(),
                    engine: "claude-code".to_string(),
                    auth: AgentAuthStatus {
                        valid: true,
                        email: "unknown".to_string(),
                        provider: "legacy".to_string(),
                    },
                    capabilities: vec![],
                    models: vec![],
                    proxy_ready: true,
                }
            }
        };

        // Assert valid authentication before wasting resources
        if !manifest.auth.valid {
            let err_msg = format!(
                "Handshake rejected: Agent '{}' has invalid or missing OAuth credentials. Please log in first.",
                manifest.name
            );
            ctx.emit_log(LogChannel::System, &err_msg);
            return Ok(JobResult::failure(job.id, Some(1), 0, err_msg));
        }

        // 3. Prepare execution command
        let mut cmd = Command::new(&bin_name);
        cmd.arg("-p").arg(prompt);

        if bypass_permissions {
            cmd.arg("--dangerously-skip-permissions");
        }

        if let Some(m) = model {
            cmd.arg("--model").arg(m);
        }

        if let Some(tp) = token_path {
            cmd.arg("--token-path").arg(tp);
        }

        // 4. Configure working directory & environment
        if let Some(ref cwd) = job.cwd {
            cmd.current_dir(cwd);
        }

        if let Some(ref env_vars) = job.env {
            for (k, v) in env_vars {
                cmd.env(k, v);
            }
        }

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        info!(
            "AgentDriver executing {} with prompt ({} chars)",
            bin_name,
            prompt.len()
        );
        let mut child = cmd.group_spawn().map_err(|e| {
            format!(
                "Failed to spawn AI agent '{}'. Ensure it is installed and in PATH (e.g. 'scoop install claude-agy'): {}",
                bin_name, e
            )
        })?;

        // 5. Asynchronous streaming of stdout and stderr
        let stdout = child.inner().stdout.take().ok_or("Failed to capture stdout pipe")?;
        let stderr = child.inner().stderr.take().ok_or("Failed to capture stderr pipe")?;

        let ctx_stdout = ctx.clone();
        let stdout_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                ctx_stdout.emit_log(LogChannel::Stdout, &line);
            }
        });

        let ctx_stderr = ctx.clone();
        let stderr_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                ctx_stderr.emit_log(LogChannel::Stderr, &line);
            }
        });

        // 6. Supervise execution with kernel timeout & cancellation
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
        let elapsed = start_time.elapsed().as_millis() as u64;

        match wait_result {
            Ok(status) => {
                let exit_code = status.code().unwrap_or(-1);
                if status.success() {
                    Ok(JobResult::success(job.id, elapsed, None))
                } else {
                    Ok(JobResult::failure(
                        job.id,
                        Some(exit_code),
                        elapsed,
                        format!("Agent exited with status code {}", exit_code),
                    ))
                }
            }
            Err(e) if e == "Execution timed out" => Ok(JobResult::timed_out(job.id, elapsed)),
            Err(e) if e == "Execution cancelled" => {
                ctx.emit_log(
                    LogChannel::System,
                    "Agent execution cancelled by control plane. Process tree terminated.",
                );
                Ok(JobResult::cancelled(job.id, elapsed))
            }
            Err(e) => Ok(JobResult::failure(job.id, None, elapsed, e)),
        }
    }

    async fn cancel(&self, _job_id: &JobId) -> Result<(), String> {
        Ok(())
    }
}
