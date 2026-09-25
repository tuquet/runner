pub mod agent;
pub mod automa;
pub mod http;
pub mod shell;

use crate::core::masker::SecretMasker;
use crate::protocol::schema::{DriverType, Job, JobId, JobResult, LogChannel, LogChunk};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

/// Context passed to each driver during job execution
#[derive(Clone)]
pub struct ExecutionContext {
    pub job_id: JobId,
    pub log_sender: UnboundedSender<LogChunk>,
    pub masker: Arc<SecretMasker>,
}

impl ExecutionContext {
    pub fn new(job_id: impl Into<JobId>, log_sender: UnboundedSender<LogChunk>, masker: Arc<SecretMasker>) -> Self {
        Self {
            job_id: job_id.into(),
            log_sender,
            masker,
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

    /// Requests cancellation of a running job
    async fn cancel(&self, job_id: &JobId) -> Result<(), String>;
}

/// Registry holding active drivers
pub struct DriverRegistry {
    drivers: HashMap<String, Box<dyn ExecutionDriver>>,
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

    pub fn get(&self, name: &str) -> Option<&Box<dyn ExecutionDriver>> {
        self.drivers.get(name)
    }

    pub fn find_for_job(&self, job: &Job) -> Option<&Box<dyn ExecutionDriver>> {
        match &job.driver {
            DriverType::Shell => self.drivers.get("shell"),
            DriverType::Agent => self.drivers.get("agent"),
            DriverType::Automa => self.drivers.get("automa").or_else(|| self.drivers.get("shell")),
            DriverType::Http => self.drivers.get("http"),
            DriverType::Custom(name) => self.drivers.get(name),
        }
    }
}
