use crate::core::engine::RunnerEngine;
use crate::protocol::events::{ControlMessage, RunnerMessage};
use crate::protocol::schema::{JobResult, LogChunk};
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info, warn};

/// Wrapper enum representing either protocol messages or raw WebSocket control frames
#[derive(Debug)]
pub enum OutboundMessage {
    Protocol(RunnerMessage),
    Raw(Message),
}

pub struct WsClientConfig {
    pub server_url: String,
    pub token: Option<String>,
    pub runner_id: String,
    pub tags: Vec<String>,
}

pub struct WsRunnerClient {
    config: WsClientConfig,
    engine: Arc<RunnerEngine>,
    active_outbound_tx: Arc<RwLock<Option<mpsc::UnboundedSender<OutboundMessage>>>>,
    pending_finished_jobs: Arc<Mutex<Vec<JobResult>>>,
}

impl WsRunnerClient {
    pub fn new(config: WsClientConfig, engine: Arc<RunnerEngine>) -> Self {
        Self {
            config,
            engine,
            active_outbound_tx: Arc::new(RwLock::new(None)),
            pending_finished_jobs: Arc::new(Mutex::new(Vec::new())),
        }
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

        // 2. Channel for outbound messages from active jobs and heartbeats
        let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<OutboundMessage>();

        // Register current active outbound transmitter
        *self.active_outbound_tx.write().await = Some(outbound_tx.clone());

        // Drain and flush any pending finished jobs from previous disconnections
        {
            let mut pending = self.pending_finished_jobs.lock().await;
            for res in pending.drain(..) {
                info!("Flushing pending completed Job '{}' upon reconnect", res.job_id);
                let _ = outbound_tx.send(OutboundMessage::Protocol(RunnerMessage::JobFinished { result: res }));
            }
        }

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
                if heartbeat_tx.send(OutboundMessage::Protocol(hb)).is_err() {
                    break;
                }
            }
        });

        // 4. Outbound sender task
        let sender_handle = tokio::spawn(async move {
            while let Some(msg) = outbound_rx.recv().await {
                let ws_msg = match msg {
                    OutboundMessage::Protocol(proto) => match serde_json::to_string(&proto) {
                        Ok(json) => Message::Text(json.into()),
                        Err(e) => {
                            error!("Failed to serialize RunnerMessage: {}", e);
                            continue;
                        }
                    },
                    OutboundMessage::Raw(raw) => raw,
                };
                if write.send(ws_msg).await.is_err() {
                    break;
                }
            }
        });

        // 5. Inbound receiver loop
        let engine_exec = self.engine.clone();
        let active_tx_shared = self.active_outbound_tx.clone();
        let pending_jobs_shared = self.pending_finished_jobs.clone();
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
                            let active_outbound = active_tx_shared.clone();
                            let pending_jobs = pending_jobs_shared.clone();

                            // Acknowledge start
                            let _ = outbound.send(OutboundMessage::Protocol(RunnerMessage::JobStarted {
                                job_id: job_id.clone(),
                                timestamp: chrono::Utc::now().timestamp_millis(),
                            }));

                            // Spawn asynchronous job execution
                            tokio::spawn(async move {
                                let (log_tx, mut log_rx) = mpsc::unbounded_channel::<LogChunk>();
                                let log_forward_outbound = outbound.clone();

                                // Stream logs to websocket
                                let stream_task = tokio::spawn(async move {
                                    while let Some(chunk) = log_rx.recv().await {
                                        let _ = log_forward_outbound.send(OutboundMessage::Protocol(RunnerMessage::JobLog { chunk }));
                                    }
                                });

                                let result = engine.execute_job(job, log_tx).await;
                                let _ = stream_task.await;

                                // Report final result via active transmitter, or buffer if disconnected
                                let sent = {
                                    let tx_guard = active_outbound.read().await;
                                    if let Some(ref tx) = *tx_guard {
                                        tx.send(OutboundMessage::Protocol(RunnerMessage::JobFinished { result: result.clone() })).is_ok()
                                    } else {
                                        false
                                    }
                                };

                                if !sent {
                                    warn!("WebSocket disconnected when Job '{}' finished. Storing completion report for reconnection flush.", result.job_id);
                                    pending_jobs.lock().await.push(result);
                                }
                            });
                        }
                        ControlMessage::Ping { timestamp } => {
                            let _ = job_outbound_tx.send(OutboundMessage::Protocol(RunnerMessage::Pong { timestamp }));
                        }
                        ControlMessage::CancelJob { job_id } => {
                            info!("Received CancelJob request for ID: {}", job_id);
                            let cancelled = engine_exec.cancel_job(&job_id).await;
                            if cancelled {
                                info!("Successfully dispatched cancellation to active Job '{}'", job_id);
                            } else {
                                warn!("CancelJob requested for inactive or unknown Job '{}'", job_id);
                            }
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
                    debug!("Received WebSocket RFC 6455 Ping ({} bytes), replying with Pong frame", data.len());
                    let _ = job_outbound_tx.send(OutboundMessage::Raw(Message::Pong(data)));
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
        *self.active_outbound_tx.write().await = None;

        Ok(())
    }
}
