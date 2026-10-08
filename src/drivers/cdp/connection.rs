use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, warn};

#[derive(Serialize)]
struct CdpRequest {
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Value::is_null")]
    params: Value,
}

#[derive(Deserialize)]
#[allow(dead_code)]
struct CdpResponse {
    id: Option<u64>,
    result: Option<Value>,
    error: Option<Value>,
    method: Option<String>,
    params: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct TargetInfo {
    pub id: Option<String>,
    pub title: Option<String>,
    #[serde(rename = "type")]
    pub target_type: Option<String>,
    pub url: Option<String>,
    #[serde(rename = "webSocketDebuggerUrl")]
    pub ws_debugger_url: Option<String>,
}

pub type PendingRequestMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, String>>>>>;

pub struct CdpConnection {
    next_id: AtomicU64,
    outbound_tx: mpsc::UnboundedSender<Message>,
    pending_requests: PendingRequestMap,
}

impl CdpConnection {
    /// Checks if a local TCP port is available for binding
    pub fn is_port_available(port: u16) -> bool {
        std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
    }

    /// Dynamically binds to port 0 to allocate an ephemeral free port from the OS
    pub fn allocate_free_port() -> Result<u16, String> {
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("Failed to bind ephemeral listener: {}", e))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("Failed to get local port: {}", e))?
            .port();
        drop(listener);
        Ok(port)
    }

    /// Reads Chromium's DevToolsActivePort file from user-data-dir.
    /// This is the industry-standard mechanism (Puppeteer/Playwright) to eliminate multi-browser port collisions.
    pub async fn read_active_port(user_data_dir: &std::path::Path) -> Result<(u16, String), String> {
        let file_path = user_data_dir.join("DevToolsActivePort");
        let content = tokio::fs::read_to_string(&file_path)
            .await
            .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;

        let mut lines = content.lines();
        let port_str = lines
            .next()
            .ok_or_else(|| "DevToolsActivePort is missing port line".to_string())?
            .trim();
        let ws_path = lines
            .next()
            .ok_or_else(|| "DevToolsActivePort is missing browser ws_path line".to_string())?
            .trim()
            .to_string();

        let port = port_str
            .parse::<u16>()
            .map_err(|e| format!("Invalid port in DevToolsActivePort ('{}'): {}", port_str, e))?;

        Ok((port, ws_path))
    }

    /// Polls user-data-dir for DevToolsActivePort up to the specified timeout
    pub async fn wait_active_port(
        user_data_dir: &std::path::Path,
        timeout: Duration,
    ) -> Result<(u16, String), String> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if let Ok(res) = Self::read_active_port(user_data_dir).await {
                return Ok(res);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Err(format!(
            "Timeout waiting for DevToolsActivePort in '{}' after {:?}",
            user_data_dir.display(),
            timeout
        ))
    }

    /// Discovers active page targets from Chromium debugging port HTTP API
    pub async fn discover_page_target(port: u16) -> Result<TargetInfo, String> {
        let endpoint = format!("http://127.0.0.1:{}/json/list", port);
        let resp = reqwest::get(&endpoint)
            .await
            .map_err(|e| format!("Failed to query Chromium debug endpoint at {}: {}", endpoint, e))?;

        let targets: Vec<TargetInfo> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse DevTools targets JSON: {}", e))?;

        targets
            .into_iter()
            .find(|t| t.target_type.as_deref() == Some("page") && t.ws_debugger_url.is_some())
            .ok_or_else(|| "No active 'page' target with WebSocket URL found".to_string())
    }

    /// Connects to a Chrome DevTools Protocol WebSocket URL
    pub async fn connect(ws_url: &str) -> Result<Arc<Self>, String> {
        let (ws_stream, _) = connect_async(ws_url)
            .await
            .map_err(|e| format!("Failed to connect to CDP WebSocket {}: {}", ws_url, e))?;

        let (mut write, mut read) = ws_stream.split();
        let (outbound_tx, mut outbound_rx) = mpsc::unbounded_channel::<Message>();
        let pending = Arc::new(Mutex::new(HashMap::<u64, oneshot::Sender<Result<Value, String>>>::new()));

        // Outbound pump
        tokio::spawn(async move {
            while let Some(msg) = outbound_rx.recv().await {
                if let Err(e) = write.send(msg).await {
                    warn!("CDP WebSocket outbound send error: {}", e);
                    break;
                }
            }
        });

        // Inbound pump
        let pending_reader = Arc::clone(&pending);
        tokio::spawn(async move {
            while let Some(msg_res) = read.next().await {
                match msg_res {
                    Ok(Message::Text(text)) => {
                        if let Ok(resp) = serde_json::from_str::<CdpResponse>(&text) {
                            if let Some(id) = resp.id {
                                let mut map = pending_reader.lock().await;
                                if let Some(sender) = map.remove(&id) {
                                    if let Some(err) = resp.error {
                                        let _ = sender.send(Err(err.to_string()));
                                    } else {
                                        let res = resp.result.unwrap_or(Value::Null);
                                        let _ = sender.send(Ok(res));
                                    }
                                }
                            } else {
                                debug!("CDP Event received: {:?}", resp.method);
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        debug!("CDP connection closed by remote browser");
                        break;
                    }
                    Err(e) => {
                        error!("CDP read error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        Ok(Arc::new(Self {
            next_id: AtomicU64::new(1),
            outbound_tx,
            pending_requests: pending,
        }))
    }

    /// Sends a raw CDP command and waits for its matched JSON-RPC response
    pub async fn send_command(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let req = CdpRequest {
            id,
            method: method.to_string(),
            params,
        };

        let req_json = serde_json::to_string(&req).map_err(|e| e.to_string())?;
        let (tx, rx) = oneshot::channel();

        {
            let mut map = self.pending_requests.lock().await;
            map.insert(id, tx);
        }

        self.outbound_tx
            .send(Message::Text(req_json.into()))
            .map_err(|e| format!("Failed to enqueue CDP message: {}", e))?;

        tokio::time::timeout(Duration::from_secs(15), rx)
            .await
            .map_err(|_| format!("CDP command '{}' timed out after 15s", method))?
            .map_err(|_| "CDP response channel dropped".to_string())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_free_port_unique() {
        let p1 = CdpConnection::allocate_free_port().expect("Failed to allocate port 1");
        let p2 = CdpConnection::allocate_free_port().expect("Failed to allocate port 2");
        assert!(p1 > 0);
        assert!(p2 > 0);
    }

    #[test]
    fn test_is_port_available() {
        let port = CdpConnection::allocate_free_port().unwrap();
        assert!(CdpConnection::is_port_available(port));

        let _listener = std::net::TcpListener::bind(("127.0.0.1", port)).unwrap();
        // Now it should be unavailable
        assert!(!CdpConnection::is_port_available(port));
    }

    #[tokio::test]
    async fn test_read_devtools_active_port() {
        let temp_dir = std::env::temp_dir().join(format!("specter_test_profile_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let active_port_file = temp_dir.join("DevToolsActivePort");
        let sample_content = "54321\r\n/devtools/browser/d9b4b087-3c58-45e6-95fa-e68d29b2bf13\r\n";
        tokio::fs::write(&active_port_file, sample_content).await.unwrap();

        let (port, ws_path) = CdpConnection::read_active_port(&temp_dir).await.unwrap();
        assert_eq!(port, 54321);
        assert_eq!(ws_path, "/devtools/browser/d9b4b087-3c58-45e6-95fa-e68d29b2bf13");

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
    }
}
