use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for an execution job
pub type JobId = String;

/// Driver category to execute the job
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriverType {
    Shell,
    Agent,
    Automa,
    Http,
    #[serde(untagged)]
    Custom(String),
}

impl std::fmt::Display for DriverType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriverType::Shell => write!(f, "shell"),
            DriverType::Agent => write!(f, "agent"),
            DriverType::Automa => write!(f, "automa"),
            DriverType::Http => write!(f, "http"),
            DriverType::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Execution status of a job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

/// Channel source of a log chunk
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogChannel {
    Stdout,
    Stderr,
    System,
}

/// Individual streaming log line or text chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogChunk {
    pub job_id: JobId,
    pub timestamp: i64,
    pub channel: LogChannel,
    pub text: String,
}

impl LogChunk {
    pub fn new(job_id: impl Into<JobId>, channel: LogChannel, text: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
            timestamp: Utc::now().timestamp_millis(),
            channel,
            text: text.into(),
        }
    }
}

/// Universal job specification dispatchable to any driver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub driver: DriverType,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

fn default_timeout_ms() -> u64 {
    300_000 // 5 minutes default
}

/// Final outcome of a job execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: JobId,
    pub status: JobStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
}

impl JobResult {
    pub fn success(job_id: impl Into<JobId>, duration_ms: u64, output: Option<serde_json::Value>) -> Self {
        Self {
            job_id: job_id.into(),
            status: JobStatus::Completed,
            exit_code: Some(0),
            duration_ms,
            error: None,
            output,
        }
    }

    pub fn failure(job_id: impl Into<JobId>, exit_code: Option<i32>, duration_ms: u64, error: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
            status: JobStatus::Failed,
            exit_code,
            duration_ms,
            error: Some(error.into()),
            output: None,
        }
    }

    pub fn timed_out(job_id: impl Into<JobId>, duration_ms: u64) -> Self {
        Self {
            job_id: job_id.into(),
            status: JobStatus::TimedOut,
            exit_code: Some(124),
            duration_ms,
            error: Some("Execution timed out".to_string()),
            output: None,
        }
    }

    pub fn cancelled(job_id: impl Into<JobId>, duration_ms: u64) -> Self {
        Self {
            job_id: job_id.into(),
            status: JobStatus::Cancelled,
            exit_code: Some(130),
            duration_ms,
            error: Some("Execution cancelled".to_string()),
            output: None,
        }
    }
}
