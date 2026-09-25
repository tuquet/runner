use crate::core::masker::SecretMasker;
use crate::drivers::agent::AgentDriver;
use crate::drivers::automa::AutomaDriver;
use crate::drivers::http::HttpDriver;
use crate::drivers::shell::ShellDriver;
use crate::drivers::{DriverRegistry, ExecutionContext};
use crate::protocol::schema::{Job, JobResult, LogChannel, LogChunk};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{error, info};

pub struct RunnerEngine {
    registry: DriverRegistry,
    masker: Arc<SecretMasker>,
    active_jobs: Arc<AtomicUsize>,
}

impl RunnerEngine {
    pub fn new() -> Self {
        let mut registry = DriverRegistry::new();
        registry.register(Box::new(ShellDriver::new()));
        registry.register(Box::new(AgentDriver::new()));
        registry.register(Box::new(AutomaDriver::new()));
        registry.register(Box::new(HttpDriver::new()));

        let masker = Arc::new(SecretMasker::new());

        Self {
            registry,
            masker,
            active_jobs: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Number of jobs actively executing right now
    pub fn active_jobs_count(&self) -> usize {
        self.active_jobs.load(Ordering::Relaxed)
    }

    /// Executes an inbound job through its registered driver
    pub async fn execute_job(&self, job: Job, log_sender: UnboundedSender<LogChunk>) -> JobResult {
        let job_id = job.id.clone();
        info!("RunnerEngine received Job '{}' [Driver: {}]", job_id, job.driver);

        self.active_jobs.fetch_add(1, Ordering::Relaxed);
        let active_counter = self.active_jobs.clone();

        let driver = match self.registry.find_for_job(&job) {
            Some(d) => d,
            None => {
                let err_msg = format!("No driver registered to handle driver type '{}'", job.driver);
                error!("{}", err_msg);
                let _ = log_sender.send(LogChunk::new(&job_id, LogChannel::System, &err_msg));
                active_counter.fetch_sub(1, Ordering::Relaxed);
                return JobResult::failure(job_id, Some(1), 0, err_msg);
            }
        };

        let ctx = ExecutionContext::new(&job_id, log_sender.clone(), self.masker.clone());
        ctx.emit_log(LogChannel::System, &format!("Dispatched job to driver '{}'", driver.name()));

        let result = match driver.execute(job, ctx.clone()).await {
            Ok(res) => {
                info!("Job '{}' completed with status {:?}", job_id, res.status);
                res
            }
            Err(e) => {
                error!("Job '{}' execution failed: {}", job_id, e);
                ctx.emit_log(LogChannel::System, &format!("Fatal driver error: {}", e));
                JobResult::failure(job_id, Some(1), 0, e)
            }
        };

        active_counter.fetch_sub(1, Ordering::Relaxed);
        result
    }
}
