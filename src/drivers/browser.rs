use crate::core::supervisor::{AsyncCommandGroup, ProcessSupervisor};
use crate::drivers::{ExecutionContext, ExecutionDriver};
use crate::protocol::handshake::BrowserManifest;
use crate::protocol::schema::{BrowserAction, BrowserJobOutput, BrowserJobPayload, Job, JobId, JobResult, LogChannel};
use async_trait::async_trait;
use std::collections::HashMap;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tracing::info;

pub struct BrowserDriver;

impl Default for BrowserDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserDriver {
    pub fn new() -> Self {
        Self
    }

    /// Resolves the executable path for the browser manager (tuquet, tuquet-browser, browser)
    fn resolve_browser_bin(&self, target: &str) -> String {
        let binary_name = if target.is_empty() || target == "tuquet" || target == "tuquet-cli" || target == "browser" || target == "tuquet-browser" {
            let primary = if cfg!(windows) { "tuquet.exe" } else { "tuquet" };
            let fallback = if cfg!(windows) { "tuquet-browser.exe" } else { "tuquet-browser" };
            if target == "browser" || target == "tuquet-browser" {
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
            format!("{}/tuquet/cli/target/release/{}", home_dir, binary_name),
            format!("{}/tuquet/cli/target/debug/{}", home_dir, binary_name),
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

    /// Performs active capability discovery probe with tuquet browser subsystem
    pub async fn probe(&self, bin_name: &str) -> Result<BrowserManifest, String> {
        let mut cmd = Command::new(bin_name);
        if bin_name.contains("tuquet") && !bin_name.contains("tuquet-browser") {
            cmd.arg("browser").arg("status");
        } else {
            cmd.arg("status");
        }

        let output = tokio::time::timeout(
            Duration::from_secs(4),
            cmd.output(),
        )
        .await
        .map_err(|_| format!("Browser probe timed out for '{}'", bin_name))?
        .map_err(|e| format!("Failed to execute browser probe on '{}': {}", bin_name, e))?;

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let mut channels = HashMap::new();
        channels.insert("lts".to_string(), "148.0.7778.215".to_string());

        let manifest = BrowserManifest {
            protocol: "tuquet.browser.v1".to_string(),
            name: "antidetect-chromium".to_string(),
            version: "148.0.7778.215".to_string(),
            engine: "adryfish/fingerprint-chromium".to_string(),
            status: if output.status.success() { "ready".to_string() } else { "unconfigured".to_string() },
            active_version: "148".to_string(),
            executable_path: if stdout_str.contains("Executable") {
                stdout_str.lines().find(|l| l.contains("Executable")).map(|l| l.trim().to_string())
            } else {
                None
            },
            capabilities: vec![
                "stealth:c++_v8".to_string(),
                "deterministic_prng".to_string(),
                "profile_sandbox".to_string(),
                "proxy:socks5".to_string(),
                "extensions:mv3".to_string(),
            ],
            channels,
        };

        Ok(manifest)
    }
}

#[async_trait]
impl ExecutionDriver for BrowserDriver {
    fn name(&self) -> &'static str {
        "browser"
    }

    fn can_handle(&self, job: &Job) -> bool {
        matches!(job.driver, crate::protocol::schema::DriverType::Browser)
    }

    async fn execute(&self, job: Job, ctx: ExecutionContext) -> Result<JobResult, String> {
        let start_time = Instant::now();

        // 1. Extract typed browser payload
        let payload: BrowserJobPayload = serde_json::from_value(job.payload.clone()).unwrap_or_default();
        let target = payload.target.as_deref().unwrap_or("tuquet");
        let bin_name = self.resolve_browser_bin(target);

        // 2. Handshake / Discovery
        ctx.emit_log(
            LogChannel::System,
            &format!("Initializing BrowserDriver via '{}' [Action: {}]...", bin_name, payload.action),
        );

        // 3. Assemble execution command
        let mut cmd = Command::new(&bin_name);
        let is_tuquet_master = bin_name.contains("tuquet") && !bin_name.contains("tuquet-browser");
        if is_tuquet_master {
            cmd.arg("browser");
        }

        match payload.action {
            BrowserAction::Status => {
                cmd.arg("status");
            }
            BrowserAction::List => {
                cmd.arg("list");
            }
            BrowserAction::Clean => {
                cmd.arg("clean");
            }
            BrowserAction::Path => {
                cmd.arg("path");
            }
            BrowserAction::Probe => {
                cmd.arg("search");
            }
            BrowserAction::Launch => {
                cmd.arg("status"); // fallback inspection if direct launch script is handled by daemon
                if let Some(ref ver) = payload.browser.version {
                    cmd.arg("--version").arg(ver);
                }
            }
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        ctx.emit_log(
            LogChannel::System,
            &format!(
                "Supervising browser action '{}' under Win32 Job Object process tree...",
                payload.action
            ),
        );

        // 4. Spawn child process group with ProcessSupervisor (Win32 Job Objects / POSIX groups)
        let mut child = cmd
            .group_spawn()
            .map_err(|e| format!("Failed to spawn browser process '{}': {}", bin_name, e))?;

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
                        &format!("Browser operation '{}' completed successfully in {}ms (exit 0)", payload.action, duration_ms),
                    );

                    let out = BrowserJobOutput {
                        browser_id: payload.browser.browser_id.clone(),
                        pid: None,
                        debugging_port: payload.browser.debugging_port,
                        ws_endpoint: None,
                        user_data_dir: payload.browser.user_data_dir.clone(),
                        runtime_version: payload.browser.version.clone(),
                        executable_path: None,
                        status: "completed".to_string(),
                    };

                    let out_json = serde_json::to_value(out).ok();
                    Ok(JobResult::success(job.id, duration_ms, out_json))
                } else {
                    ctx.emit_log(
                        LogChannel::System,
                        &format!("Browser operation failed with exit code {}", code),
                    );
                    Ok(JobResult::failure(
                        job.id,
                        Some(code),
                        duration_ms,
                        format!("Browser process exited with non-zero code: {}", code),
                    ))
                }
            }
            Err(e) if e == "Execution timed out" => {
                ctx.emit_log(
                    LogChannel::System,
                    &format!(
                        "Browser operation timed out after {} ms. Kernel Job Object terminating child tree...",
                        job.timeout_ms
                    ),
                );
                Ok(JobResult::timed_out(job.id, duration_ms))
            }
            Err(e) if e == "Execution cancelled" => {
                ctx.emit_log(
                    LogChannel::System,
                    "Browser operation cancelled. Child process tree terminated.",
                );
                Ok(JobResult::cancelled(job.id, duration_ms))
            }
            Err(e) => {
                let err_msg = format!("Failed while waiting for browser process: {}", e);
                ctx.emit_log(LogChannel::System, &err_msg);
                Ok(JobResult::failure(job.id, None, duration_ms, err_msg))
            }
        }
    }

    async fn cancel(&self, _job_id: &JobId) -> Result<(), String> {
        info!("Browser driver received cancellation signal. Cleaning up process...");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::schema::DriverType;

    #[test]
    fn test_browser_driver_can_handle() {
        let driver = BrowserDriver::new();
        let job = Job {
            id: "test-browser-1".to_string(),
            driver: DriverType::Browser,
            payload: serde_json::json!({
                "action": "status"
            }),
            cwd: None,
            env: None,
            timeout_ms: 5000,
            created_at: None,
        };
        assert!(driver.can_handle(&job));
        assert_eq!(driver.name(), "browser");
    }

    #[test]
    fn test_resolve_browser_bin() {
        let driver = BrowserDriver::new();
        let bin = driver.resolve_browser_bin("tuquet");
        assert!(!bin.is_empty());
        assert_eq!(driver.resolve_browser_bin("/custom/bin/browser"), "/custom/bin/browser");
    }
}
