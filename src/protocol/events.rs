use crate::protocol::schema::{Job, JobId, JobResult, LogChunk};
use serde::{Deserialize, Serialize};

/// Messages sent outbound from Runner to Web Control Plane
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RunnerMessage {
    /// Initial registration and capability handshake
    Hello {
        runner_id: String,
        hostname: String,
        os: String,
        arch: String,
        version: String,
        tags: Vec<String>,
    },
    /// Periodic heartbeat ping
    Heartbeat {
        runner_id: String,
        timestamp: i64,
        active_jobs: usize,
    },
    /// Acknowledgment of job dispatch
    JobStarted {
        job_id: JobId,
        timestamp: i64,
    },
    /// Real-time streaming log chunk
    JobLog {
        chunk: LogChunk,
    },
    /// Progress update (0 - 100)
    JobProgress {
        job_id: JobId,
        percent: u8,
        message: Option<String>,
    },
    /// Final job completion report
    JobFinished {
        result: JobResult,
    },
    /// Heartbeat response (Pong)
    Pong {
        timestamp: i64,
    },
}

/// Messages sent inbound from Web Control Plane to Runner
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum ControlMessage {
    /// Dispatch a new job for execution
    DispatchJob {
        job: Job,
    },
    /// Cancel an actively running job
    CancelJob {
        job_id: JobId,
    },
    /// Liveness probe ping
    Ping {
        timestamp: i64,
    },
    /// Gracefully shutdown runner daemon
    Shutdown,
}
