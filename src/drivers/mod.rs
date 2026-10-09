pub mod agent;
pub mod automa;
pub mod browser;
pub mod cdp;
pub mod http;
pub mod shell;

use crate::core::masker::SecretMasker;
use crate::protocol::schema::{DriverType, Job, JobId, JobResult, LogChannel, LogChunk};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

/// Context passed to each driver during job execution
#[derive(Clone)]
pub struct ExecutionContext {
    pub job_id: JobId,
    pub log_sender: UnboundedSender<LogChunk>,
    pub masker: Arc<SecretMasker>,
    pub cancel_token: CancellationToken,
}

impl ExecutionContext {
    pub fn new(
        job_id: impl Into<JobId>,
        log_sender: UnboundedSender<LogChunk>,
        masker: Arc<SecretMasker>,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            log_sender,
            masker,
            cancel_token,
        }
    }

    /// Emits a sanitized log chunk to the transport stream
    pub fn emit_log(&self, channel: LogChannel, raw_text: &str) {
        let masked_text = self.masker.mask(raw_text);
        let chunk = LogChunk::new(&self.job_id, channel, masked_text);
        let _ = self.log_sender.send(chunk);
    }
}

/// Common trait that all pluggable execution drivers must implement
#[async_trait]
pub trait ExecutionDriver: Send + Sync {
    /// Identifier for this driver (e.g. "shell", "agent", "http")
    fn name(&self) -> &'static str;

    /// Checks if this driver can execute the given job
    fn can_handle(&self, job: &Job) -> bool;

    /// Executes the job and returns the final JobResult
    async fn execute(&self, job: Job, ctx: ExecutionContext) -> Result<JobResult, String>;

    /// Requests cancellation of a running job.
    /// By default, drivers rely on cooperative cancellation via `ctx.cancel_token`.
    async fn cancel(&self, _job_id: &JobId) -> Result<(), String> {
        Ok(())
    }
}

/// Registry holding active drivers
pub struct DriverRegistry {
    drivers: HashMap<String, Box<dyn ExecutionDriver>>,
}

impl Default for DriverRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl DriverRegistry {
    pub fn new() -> Self {
        Self {
            drivers: HashMap::new(),
        }
    }

    pub fn register(&mut self, driver: Box<dyn ExecutionDriver>) {
        self.drivers.insert(driver.name().to_string(), driver);
    }

    pub fn get(&self, name: &str) -> Option<&dyn ExecutionDriver> {
        self.drivers.get(name).map(|b| b.as_ref())
    }

    pub fn find_for_job(&self, job: &Job) -> Option<&dyn ExecutionDriver> {
        match &job.driver {
            DriverType::Shell => self.get("shell"),
            DriverType::Agent => self.get("agent"),
            DriverType::Automa => self.get("automa"),
            DriverType::Browser => self.get("browser"),
            DriverType::Http => self.get("http"),
            DriverType::Custom(name) => self.get(name),
        }
    }
}
