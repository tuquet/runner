use crate::core::engine::RunnerEngine;
use crate::protocol::schema::{Job, JobResult, LogChannel, LogChunk};
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct LocalRunner {
    engine: Arc<RunnerEngine>,
}

impl LocalRunner {
    pub fn new(engine: Arc<RunnerEngine>) -> Self {
        Self { engine }
    }

    /// Executes a job locally and streams logs directly to terminal stdout/stderr
    pub async fn run_job(&self, job: Job) -> JobResult {
        let (tx, mut rx) = mpsc::unbounded_channel::<LogChunk>();

        // Background printer task
        let printer = tokio::spawn(async move {
            while let Some(chunk) = rx.recv().await {
                match chunk.channel {
                    LogChannel::Stdout => {
                        println!("{}", chunk.text);
                    }
                    LogChannel::Stderr => {
                        eprintln!("\x1b[31m{}\x1b[0m", chunk.text);
                    }
                    LogChannel::System => {
                        eprintln!("\x1b[36m[*] {}\x1b[0m", chunk.text);
                    }
                }
            }
        });

        let result = self.engine.execute_job(job, tx).await;
        let _ = printer.await;

        result
    }
}
