use crate::drivers::cdp::connection::CdpConnection;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, warn};

pub struct StealthEngine {
    connection: Arc<CdpConnection>,
    isolated_context_id: tokio::sync::RwLock<Option<u64>>,
}

impl StealthEngine {
    pub fn new(connection: Arc<CdpConnection>) -> Self {
        Self {
            connection,
            isolated_context_id: tokio::sync::RwLock::new(None),
        }
    }

    /// Obtains or creates an Isolated World execution context.
    /// In accordance with the Patchright specification, evaluating within an isolated world
    /// prevents websites from detecting injected automation variables, property getters,
    /// or Error stack modifications on the Window object.
    pub async fn get_isolated_context(&self) -> Result<u64, String> {
        {
            let current = self.isolated_context_id.read().await;
            if let Some(id) = *current {
                return Ok(id);
            }
        }

        // 1. Get main frame ID
        let tree_res = self
            .connection
            .send_command("Page.getFrameTree", json!({}))
            .await?;

        let frame_id = tree_res
            .pointer("/frameTree/frame/id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Failed to extract main frameId from Page.getFrameTree".to_string())?;

        // 2. Create an isolated world (zero variable bleed to website JavaScript)
        let world_res = self
            .connection
            .send_command(
                "Page.createIsolatedWorld",
                json!({
                    "frameId": frame_id,
                    "worldName": "specter_stealth_context",
                    "grantUniveralAccess": true
                }),
            )
            .await?;

        let context_id = world_res
            .get("executionContextId")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| "Failed to obtain executionContextId from Page.createIsolatedWorld".to_string())?;

        debug!("Created isolated execution context ID: {}", context_id);
        let mut writer = self.isolated_context_id.write().await;
        *writer = Some(context_id);

        Ok(context_id)
    }

    /// Invalidates cached isolated context ID (e.g. after full page navigation)
    pub async fn reset_context(&self) {
        let mut writer = self.isolated_context_id.write().await;
        *writer = None;
    }

    /// Evaluates JavaScript strictly within the Isolated World context
    pub async fn evaluate_isolated(&self, script: &str) -> Result<Value, String> {
        let context_id = self.get_isolated_context().await?;

        let params = json!({
            "expression": script,
            "contextId": context_id,
            "returnByValue": true,
            "awaitPromise": true
        });

        let res = self.connection.send_command("Runtime.evaluate", params).await?;

        if let Some(exception) = res.get("exceptionDetails") {
            warn!("Isolated World evaluation exception: {:?}", exception);
            return Err(format!("JavaScript exception in isolated world: {:?}", exception));
        }

        Ok(res.pointer("/result/value").cloned().unwrap_or(Value::Null))
    }
}
