use crate::core::supervisor::ProcessSupervisor;
use crate::drivers::{ExecutionContext, ExecutionDriver};
use crate::protocol::schema::{Job, JobId, JobResult, LogChannel};
use async_trait::async_trait;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::info;

pub struct AgentDriver;

impl AgentDriver {
    pub fn new() -> Self {
        Self
    }

    /// Resolves the executable path for the agent runner (claude-agy or claude)
    fn resolve_agent_bin(&self, target: &str) -> String {
        if target.is_empty() || target == "claude-agy" {
            // Check if claude-agy is available in PATH
            if cfg!(windows) {
                "claude-agy.cmd".to_string()
            } else {
                "claude-agy".to_string()
            }
        } else {
            target.to_string()
        }
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

        // 2. Prepare command invocation
        let bin_name = self.resolve_agent_bin(target);
        let mut cmd = Command::new(&bin_name);

        // One-shot prompt mode
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

        // 3. Configure working directory & environment
        if let Some(ref cwd) = job.cwd {
            cmd.current_dir(cwd);
        }

        if let Some(ref env_vars) = job.env {
            for (k, v) in env_vars {
                cmd.env(k, v);
            }
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        info!("AgentDriver launching {} with prompt ({} chars)", bin_name, prompt.len());
        let mut child = cmd.spawn().map_err(|e| {
            format!(
                "Failed to spawn AI agent '{}'. Ensure it is installed and in PATH (e.g. 'scoop install claude-agy'): {}",
                bin_name, e
            )
        })?;

        // 4. Stream output in real time
        let stdout = child.stdout.take().ok_or("Failed to capture stdout pipe")?;
        let stderr = child.stderr.take().ok_or("Failed to capture stderr pipe")?;

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

        // 5. Supervise process with timeout
        let mut supervisor = ProcessSupervisor::new();
        let timeout_duration = Duration::from_millis(job.timeout_ms);

        let wait_result = supervisor.wait_with_timeout(&mut child, timeout_duration).await;

        let _ = tokio::join!(stdout_handle, stderr_handle);
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
            Err(e) => Ok(JobResult::failure(job.id, None, elapsed, e)),
        }
    }

    async fn cancel(&self, _job_id: &JobId) -> Result<(), String> {
        Ok(())
    }
}
