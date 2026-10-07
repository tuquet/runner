use crate::core::masker::SecretMasker;
use crate::drivers::agent::AgentDriver;
use crate::drivers::automa::AutomaDriver;
use crate::drivers::browser::BrowserDriver;
use crate::drivers::http::HttpDriver;
use crate::drivers::shell::ShellDriver;
use crate::drivers::{DriverRegistry, ExecutionContext};
use crate::protocol::schema::{Job, JobId, JobResult, LogChannel, LogChunk};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{RwLock, Semaphore};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct RunnerEngine {
    registry: DriverRegistry,
    masker: Arc<SecretMasker>,
    active_jobs: Arc<AtomicUsize>,
    active_cancellers: Arc<RwLock<HashMap<JobId, CancellationToken>>>,
    concurrency_semaphore: Arc<Semaphore>,
}

impl Default for RunnerEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RunnerEngine {
    pub fn new() -> Self {
        Self::with_max_concurrency(16)
    }

    pub fn with_max_concurrency(max_concurrency: usize) -> Self {
        let mut registry = DriverRegistry::new();
        registry.register(Box::new(ShellDriver::new()));
        registry.register(Box::new(AgentDriver::new()));
        registry.register(Box::new(AutomaDriver::new()));
        registry.register(Box::new(BrowserDriver::new()));
        registry.register(Box::new(HttpDriver::new()));

        let masker = Arc::new(SecretMasker::new());

        Self {
            registry,
            masker,
            active_jobs: Arc::new(AtomicUsize::new(0)),
            active_cancellers: Arc::new(RwLock::new(HashMap::new())),
            concurrency_semaphore: Arc::new(Semaphore::new(max_concurrency)),
        }
    }

    /// Number of jobs actively executing right now
    pub fn active_jobs_count(&self) -> usize {
        self.active_jobs.load(Ordering::Relaxed)
    }

    /// Triggers cancellation for an active running job by ID
    pub async fn cancel_job(&self, job_id: &str) -> bool {
        let cancellers = self.active_cancellers.read().await;
        if let Some(token) = cancellers.get(job_id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Executes an inbound job through its registered driver
    pub async fn execute_job(&self, job: Job, log_sender: UnboundedSender<LogChunk>) -> JobResult {
        let job_id = job.id.clone();
        info!("RunnerEngine received Job '{}' [Driver: {}]", job_id, job.driver);

        // Concurrency limiter backpressure
        let _permit = match self.concurrency_semaphore.acquire().await {
            Ok(permit) => permit,
            Err(e) => {
                let err_msg = format!("Failed to acquire concurrency permit: {}", e);
                error!("{}", err_msg);
                return JobResult::failure(job_id, Some(1), 0, err_msg);
            }
        };

        self.active_jobs.fetch_add(1, Ordering::Relaxed);
        let active_counter = self.active_jobs.clone();

        let cancel_token = CancellationToken::new();
        {
            let mut cancellers = self.active_cancellers.write().await;
            cancellers.insert(job_id.clone(), cancel_token.clone());
        }

        let driver = match self.registry.find_for_job(&job) {
            Some(d) => d,
            None => {
                let err_msg = format!("No driver registered to handle driver type '{}'", job.driver);
                error!("{}", err_msg);
                let _ = log_sender.send(LogChunk::new(&job_id, LogChannel::System, &err_msg));
                self.active_cancellers.write().await.remove(&job_id);
                active_counter.fetch_sub(1, Ordering::Relaxed);
                return JobResult::failure(job_id, Some(1), 0, err_msg);
            }
        };

        let ctx = ExecutionContext::new(&job_id, log_sender.clone(), self.masker.clone(), cancel_token);
        ctx.emit_log(LogChannel::System, &format!("Dispatched job to driver '{}'", driver.name()));

        let result = match driver.execute(job, ctx.clone()).await {
            Ok(res) => {
                info!("Job '{}' completed with status {:?}", job_id, res.status);
                res
            }
            Err(e) => {
                error!("Job '{}' execution failed: {}", job_id, e);
                ctx.emit_log(LogChannel::System, &format!("Fatal driver error: {}", e));
                JobResult::failure(job_id.clone(), Some(1), 0, e)
            }
        };

        self.active_cancellers.write().await.remove(&job_id);
        active_counter.fetch_sub(1, Ordering::Relaxed);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::schema::{DriverType, JobStatus};
    use std::time::Duration;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_runner_engine_executes_shell_job() {
        let engine = RunnerEngine::new();
        let (tx, _rx) = mpsc::unbounded_channel();

        let job = Job {
            id: "test-shell-1".to_string(),
            driver: DriverType::Shell,
            payload: serde_json::json!({
                "command": "echo hello_runner"
            }),
            cwd: None,
            env: None,
            timeout_ms: 10_000,
            created_at: None,
        };

        let result = engine.execute_job(job, tx).await;
        assert_eq!(result.status, JobStatus::Completed);
        assert_eq!(result.exit_code, Some(0));
    }

    #[tokio::test]
    async fn test_runner_engine_cancels_running_job() {
        let engine = Arc::new(RunnerEngine::new());
        let engine_clone = engine.clone();
        let (tx, _rx) = mpsc::unbounded_channel();

        let job_id = "test-cancel-job".to_string();
        let job = Job {
            id: job_id.clone(),
            driver: DriverType::Shell,
            payload: serde_json::json!({
                "command": "sleep 10"
            }),
            cwd: None,
            env: None,
            timeout_ms: 30_000,
            created_at: None,
        };

        let exec_handle = tokio::spawn(async move {
            engine_clone.execute_job(job, tx).await
        });

        // Wait briefly for job to start running
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(engine.active_jobs_count(), 1);

        // Cancel job
        let cancelled = engine.cancel_job(&job_id).await;
        assert!(cancelled, "cancel_job should return true for active job");

        let result = exec_handle.await.unwrap();
        assert_eq!(result.status, JobStatus::Cancelled);
        assert_eq!(engine.active_jobs_count(), 0);
    }
}
