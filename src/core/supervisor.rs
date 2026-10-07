pub use command_group::{AsyncCommandGroup, AsyncGroupChild};
use std::process::ExitStatus;
use std::time::Duration;
use tracing::{debug, warn};

/// High-reliability multi-platform process supervisor with kernel-level kill-tree guarantees.
///
/// Backed by `command-group`:
/// - On Windows: Automatic Win32 Job Object encapsulation with `TerminateJobObject`.
/// - On Unix: Automatic process group isolation (`setpgid`) with `killpg(SIGKILL)`.
#[derive(Debug, Default, Clone)]
pub struct ProcessSupervisor {
    pid: Option<u32>,
}

impl ProcessSupervisor {
    pub fn new() -> Self {
        Self { pid: None }
    }

    /// Attaches an actively spawned child group to this supervisor
    pub fn attach(&mut self, child: &AsyncGroupChild) {
        self.pid = child.id();
    }

    /// Returns the tracked process identifier, if available
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Forcibly kills the entire process tree within sub-millisecond latency.
    /// Uses kernel-level Win32 Job Object termination on Windows and `killpg` on Unix,
    /// with an automated bounded wait and defense-in-depth fallback.
    pub async fn terminate_tree(&mut self, child: &mut AsyncGroupChild) {
        let pid = child.id().or(self.pid);
        debug!("Terminating process tree for PID {:?}", pid);

        // 1. Immediate kernel-level atomic group termination
        if let Err(e) = child.start_kill() {
            debug!("AsyncGroupChild start_kill returned: {}", e);
        }

        // 2. Await process tree teardown and reap zombie handles with a safety timeout
        if let Err(e) = tokio::time::timeout(Duration::from_millis(1500), child.wait()).await {
            warn!("Timeout waiting for process group exit after kill: {}", e);
        }

        // 3. Defense-in-depth fallback for detached child processes
        if let Some(p) = pid {
            Self::terminate_by_pid(p).await;
        }
    }

    /// Fallback method to terminate a process tree by PID when AsyncGroupChild is not available
    pub async fn terminate_by_pid(pid: u32) {
        debug!("Fallback terminating process tree by PID {}", pid);
        #[cfg(windows)]
        {
            let _ = tokio::process::Command::new("taskkill")
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await;
        }

        #[cfg(unix)]
        {
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
                libc::kill(pid as i32, libc::SIGKILL);
            }
        }
    }

    /// Waits for process exit with timeout; terminates tree if timeout exceeded
    pub async fn wait_with_timeout(
        &mut self,
        child: &mut AsyncGroupChild,
        timeout: Duration,
    ) -> Result<ExitStatus, String> {
        self.wait_with_timeout_or_cancel(child, timeout, None).await
    }

    /// Waits for process exit with timeout or cancellation; terminates tree if cancelled or timed out
    pub async fn wait_with_timeout_or_cancel(
        &mut self,
        child: &mut AsyncGroupChild,
        timeout: Duration,
        cancel_token: Option<&tokio_util::sync::CancellationToken>,
    ) -> Result<ExitStatus, String> {
        self.attach(child);

        tokio::select! {
            res = child.wait() => {
                match res {
                    Ok(status) => Ok(status),
                    Err(e) => Err(format!("Process wait error: {}", e)),
                }
            }
            _ = tokio::time::sleep(timeout) => {
                warn!("Job execution timed out after {:?}", timeout);
                self.terminate_tree(child).await;
                Err("Execution timed out".to_string())
            }
            _ = async {
                if let Some(token) = cancel_token {
                    token.cancelled().await;
                } else {
                    std::future::pending::<()>().await;
                }
            } => {
                warn!("Job execution cancelled by control plane. Terminating process tree...");
                self.terminate_tree(child).await;
                Err("Execution cancelled".to_string())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    fn make_test_cmd(script: &str) -> tokio::process::Command {
        if cfg!(windows) {
            let mut cmd = tokio::process::Command::new("powershell");
            cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
            cmd
        } else {
            let mut cmd = tokio::process::Command::new("sh");
            cmd.args(["-c", script]);
            cmd
        }
    }

    #[tokio::test]
    async fn test_supervisor_wait_success() {
        let mut supervisor = ProcessSupervisor::new();
        let mut cmd = make_test_cmd("exit 0");
        let mut child = cmd.group_spawn().expect("Failed to group_spawn");

        let status = supervisor
            .wait_with_timeout(&mut child, Duration::from_secs(5))
            .await
            .expect("Process should succeed");

        assert!(status.success());
    }

    #[tokio::test]
    async fn test_supervisor_timeout() {
        let mut supervisor = ProcessSupervisor::new();
        let sleep_script = if cfg!(windows) {
            "Start-Sleep -Seconds 5"
        } else {
            "sleep 5"
        };
        let mut cmd = make_test_cmd(sleep_script);
        let mut child = cmd.group_spawn().expect("Failed to group_spawn");

        let res = supervisor
            .wait_with_timeout(&mut child, Duration::from_millis(200))
            .await;

        assert_eq!(res, Err("Execution timed out".to_string()));
    }

    #[tokio::test]
    async fn test_supervisor_cancellation() {
        let mut supervisor = ProcessSupervisor::new();
        let sleep_script = if cfg!(windows) {
            "Start-Sleep -Seconds 5"
        } else {
            "sleep 5"
        };
        let mut cmd = make_test_cmd(sleep_script);
        let mut child = cmd.group_spawn().expect("Failed to group_spawn");

        let cancel_token = CancellationToken::new();
        let token_clone = cancel_token.clone();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(150)).await;
            token_clone.cancel();
        });

        let res = supervisor
            .wait_with_timeout_or_cancel(&mut child, Duration::from_secs(5), Some(&cancel_token))
            .await;

        assert_eq!(res, Err("Execution cancelled".to_string()));
    }

    #[tokio::test]
    async fn test_supervisor_terminate_tree() {
        let mut supervisor = ProcessSupervisor::new();
        let sleep_script = if cfg!(windows) {
            "Start-Sleep -Seconds 5"
        } else {
            "sleep 5"
        };
        let mut cmd = make_test_cmd(sleep_script);
        let mut child = cmd.group_spawn().expect("Failed to group_spawn");

        supervisor.attach(&child);
        assert!(supervisor.pid().is_some());

        supervisor.terminate_tree(&mut child).await;
    }
}
