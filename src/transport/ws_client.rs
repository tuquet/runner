use crate::core::engine::RunnerEngine;
use crate::protocol::events::{ControlMessage, RunnerMessage};
use crate::protocol::schema::LogChunk;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

pub struct WsClientConfig {
    pub server_url: String,
    pub token: Option<String>,
    pub runner_id: String,
    pub tags: Vec<String>,
}

pub struct WsRunnerClient {
    config: WsClientConfig,
    engine: Arc<RunnerEngine>,
}

impl WsRunnerClient {
    pub fn new(config: WsClientConfig, engine: Arc<RunnerEngine>) -> Self {
        Self { config, engine }
    }

    /// Starts the long-running worker daemon loop with automatic reconnection
    pub async fn start(&self) {
        let mut backoff = Duration::from_secs(1);
        let max_backoff = Duration::from_secs(30);

        loop {
            info!("Connecting outbound to Web Control Plane: {}", self.config.server_url);
            match self.connect_and_run().await {
                Ok(_) => {
                    info!("Connection closed cleanly. Reconnecting in {:?}...", backoff);
                }
                Err(e) => {
                    warn!("Connection lost: {}. Retrying in {:?}...", e, backoff);
                }
            }

            tokio::time::sleep(backoff).await;
            backoff = std::cmp::min(backoff * 2, max_backoff);
        }
    }

    async fn connect_and_run(&self) -> Result<(), String> {
        let mut target_url = self.config.server_url.clone();
        if let Some(ref tok) = self.config.token {
            let sep = if target_url.contains('?') { "&" } else { "?" };
            target_url = format!("{}{}token={}", target_url, sep, tok);
        }

        let (ws_stream, _) = connect_async(&target_url)
            .await
            .map_err(|e| format!("WebSocket handshake failed: {}", e))?;

        info!("Successfully connected to Control Plane!");

        let (mut write, mut read) = ws_stream.split();

        // 1. Send Hello handshake
        let hello = RunnerMessage::Hello {
            runner_id: self.config.runner_id.clone(),
            hostname: std::env::var("COMPUTERNAME")
                .or_else(|_| std::env::var("HOSTNAME"))
                .unwrap_or_else(|_| "unknown-host".to_string()),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            tags: self.config.tags.clone(),
        };

        let hello_json = serde_json::to_string(&hello).map_err(|e| e.to_string())?;
        write
            .send(Message::Text(hello_json.into()))
            .await
            .map_err(|e| format!("Failed to send Hello: {}", e))?;

        // 2. Channel for outbound messages from active jobs
        let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<RunnerMessage>();

        // 3. Heartbeat loop task
        let runner_id_clone = self.config.runner_id.clone();
        let engine_clone = self.engine.clone();
        let heartbeat_tx = outbound_tx.clone();
        let heartbeat_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(20));
            loop {
                interval.tick().await;
                let hb = RunnerMessage::Heartbeat {
                    runner_id: runner_id_clone.clone(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    active_jobs: engine_clone.active_jobs_count(),
                };
                if heartbeat_tx.send(hb).is_err() {
                    break;
                }
            }
        });

        // 4. Outbound sender task
        let sender_handle = tokio::spawn(async move {
            while let Some(msg) = outbound_rx.recv().await {
                if let Ok(json) = serde_json::to_string(&msg) {
                    if write.send(Message::Text(json.into())).await.is_err() {
                        break;
                    }
                }
            }
        });

        // 5. Inbound receiver loop
        let engine_exec = self.engine.clone();
        let job_outbound_tx = outbound_tx.clone();

        while let Some(msg_res) = read.next().await {
            match msg_res {
                Ok(Message::Text(text)) => {
                    let control_msg: ControlMessage = match serde_json::from_str(&text) {
                        Ok(m) => m,
                        Err(e) => {
                            debug!("Ignored unknown message payload: {}", e);
                            continue;
                        }
                    };

                    match control_msg {
                        ControlMessage::DispatchJob { job } => {
                            let job_id = job.id.clone();
                            let engine = engine_exec.clone();
                            let outbound = job_outbound_tx.clone();

                            // Acknowledge start
                            let _ = outbound.send(RunnerMessage::JobStarted {
                                job_id: job_id.clone(),
                                timestamp: chrono::Utc::now().timestamp_millis(),
                            });

                            // Spawn asynchronous job execution
                            tokio::spawn(async move {
                                let (log_tx, mut log_rx) = mpsc::unbounded_channel::<LogChunk>();
                                let log_forward_outbound = outbound.clone();

                                // Stream logs to websocket
                                let stream_task = tokio::spawn(async move {
                                    while let Some(chunk) = log_rx.recv().await {
                                        let _ = log_forward_outbound.send(RunnerMessage::JobLog { chunk });
                                    }
                                });

                                let result = engine.execute_job(job, log_tx).await;
                                let _ = stream_task.await;

                                // Report final result
                                let _ = outbound.send(RunnerMessage::JobFinished { result });
                            });
                        }
                        ControlMessage::Ping { timestamp } => {
                            let _ = job_outbound_tx.send(RunnerMessage::Pong { timestamp });
                        }
                        ControlMessage::CancelJob { job_id } => {
                            warn!("Received CancelJob request for ID: {}", job_id);
                        }
                        ControlMessage::Shutdown => {
                            info!("Received graceful shutdown signal from Control Plane");
                            break;
                        }
                    }
                }
                Ok(Message::Close(_)) => {
                    info!("Remote server closed WebSocket connection");
                    break;
                }
                Ok(Message::Ping(data)) => {
                    debug!("Received WebSocket ping ({} bytes)", data.len());
                }
                Err(e) => {
                    error!("WebSocket stream error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        heartbeat_handle.abort();
        sender_handle.abort();

        Ok(())
    }
}
