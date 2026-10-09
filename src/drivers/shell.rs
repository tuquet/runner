use crate::core::supervisor::{AsyncCommandGroup, ProcessSupervisor};
use crate::drivers::{ExecutionContext, ExecutionDriver};
use crate::protocol::schema::{Job, JobResult, LogChannel};
use async_trait::async_trait;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::{debug, error};

pub struct ShellDriver;

impl Default for ShellDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl ShellDriver {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ExecutionDriver for ShellDriver {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn can_handle(&self, job: &Job) -> bool {
        matches!(job.driver, crate::protocol::schema::DriverType::Shell)
    }

    async fn execute(&self, job: Job, ctx: ExecutionContext) -> Result<JobResult, String> {
        let start_time = Instant::now();

        // 1. Parse command and shell
        let command_str = job
            .payload
            .get("command")
            .or_else(|| job.payload.get("script"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Shell driver requires 'command' or 'script' string in payload".to_string())?;

        let shell_type = job
            .payload
            .get("shell")
            .and_then(|v| v.as_str())
            .unwrap_or(if cfg!(windows) { "powershell" } else { "bash" });

        // 2. Build OS command
        let mut cmd = match shell_type {
            "powershell" | "pwsh" => {
                let mut c = Command::new(if cfg!(windows) { "powershell" } else { "pwsh" });
                c.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", command_str]);
                c
            }
            "cmd" => {
                let mut c = Command::new("cmd.exe");
                c.args(["/C", command_str]);
                c
            }
            _ => {
                let mut c = Command::new("bash");
                c.args(["-c", command_str]);
                c
            }
        };

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

        debug!("Spawning shell process group: {:?}", cmd);
        let mut child = cmd
            .group_spawn()
            .map_err(|e| format!("Failed to spawn shell process: {}", e))?;

        // 4. Set up asynchronous line streaming for stdout & stderr
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

        // 5. Supervise process with Win32 Job Object & Timeout
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
                        format!("Process exited with status code {}", exit_code),
                    ))
                }
            }
            Err(e) if e == "Execution timed out" => {
                Ok(JobResult::timed_out(job.id, elapsed))
            }
            Err(e) if e == "Execution cancelled" => {
                ctx.emit_log(
                    LogChannel::System,
                    "Execution cancelled by control plane. Process tree terminated.",
                );
                Ok(JobResult::cancelled(job.id, elapsed))
            }
            Err(e) => {
                error!("Process supervisor error: {}", e);
                Ok(JobResult::failure(job.id, None, elapsed, e))
            }
        }
    }
}
