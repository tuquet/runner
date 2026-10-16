pub mod connection;
pub mod element;
pub mod interaction;
pub mod stealth;
pub mod workflow;

pub use connection::CdpConnection;
pub use element::{ElementBoundingBox, ElementEngine};
pub use interaction::HumanInteractionEngine;
pub use stealth::StealthEngine;
pub use workflow::{BrowserBackend, ExecutionState, WorkflowDagExecutor, WorkflowGraph};

use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};

pub struct CdpSession {
    connection: Arc<CdpConnection>,
    stealth: Arc<StealthEngine>,
    interaction: Arc<HumanInteractionEngine>,
    element: Arc<ElementEngine>,
}

impl CdpSession {
    /// Connects to an existing Chromium instance running with --remote-debugging-port
    pub async fn connect(debugging_port: u16) -> Result<Self, String> {
        let target = CdpConnection::discover_page_target(debugging_port).await?;
        let ws_url = target
            .ws_debugger_url
            .ok_or_else(|| "Target does not have a webSocketDebuggerUrl".to_string())?;

        info!("Connecting CDP session to: {}", ws_url);
        let connection = CdpConnection::connect(&ws_url).await?;
        let stealth = Arc::new(StealthEngine::new(Arc::clone(&connection)));
        let interaction = Arc::new(HumanInteractionEngine::new(Arc::clone(&connection)));
        let element = Arc::new(ElementEngine::new(Arc::clone(&stealth)));

        // Enable Page domain to support navigation lifecycle
        connection.send_command("Page.enable", json!({})).await?;

        Ok(Self {
            connection,
            stealth,
            interaction,
            element,
        })
    }

    /// Connects dynamically using DevToolsActivePort from user-data-dir.
    /// This eliminates port collisions when running multiple browsers concurrently.
    pub async fn connect_with_active_port(user_data_dir: &std::path::Path) -> Result<Self, String> {
        let (port, _) = CdpConnection::wait_active_port(user_data_dir, Duration::from_secs(10)).await?;
        info!("Discovered dynamic DevTools port {} from {}", port, user_data_dir.display());
        Self::connect(port).await
    }

    /// Automatically connects via DevToolsActivePort if user_data_dir is present,
    /// or falls back to an explicit debugging port.
    pub async fn connect_auto(
        port_opt: Option<u16>,
        user_data_dir_opt: Option<&std::path::Path>,
    ) -> Result<Self, String> {
        if let Some(dir) = user_data_dir_opt {
            let active_port_file = dir.join("DevToolsActivePort");
            if active_port_file.exists() {
                return Self::connect_with_active_port(dir).await;
            }
        }

        if let Some(port) = port_opt {
            Self::connect(port).await
        } else if let Some(dir) = user_data_dir_opt {
            Self::connect_with_active_port(dir).await
        } else {
            Err("Neither debugging port nor user_data_dir was specified".to_string())
        }
    }

    /// Access underlying raw CDP connection
    pub fn connection(&self) -> &Arc<CdpConnection> {
        &self.connection
    }

    /// Access underlying ElementEngine
    pub fn element_engine(&self) -> &Arc<ElementEngine> {
        &self.element
    }

    /// Access underlying HumanInteractionEngine
    pub fn interaction_engine(&self) -> &Arc<HumanInteractionEngine> {
        &self.interaction
    }

    /// Navigates to a target URL and waits for page ready state
    pub async fn navigate(&self, url: &str) -> Result<(), String> {
        self.stealth.reset_context().await;
        self.connection
            .send_command("Page.navigate", json!({ "url": url }))
            .await?;

        self.wait_ready_state(Duration::from_secs(15)).await;
        debug!("Page navigation to '{}' completed", url);
        Ok(())
    }

    /// Reloads the active page
    pub async fn reload(&self) -> Result<(), String> {
        self.stealth.reset_context().await;
        self.connection
            .send_command("Page.reload", json!({}))
            .await?;
        self.wait_ready_state(Duration::from_secs(15)).await;
        Ok(())
    }

    /// Navigates backward in browser history
    pub async fn go_back(&self) -> Result<(), String> {
        self.stealth.reset_context().await;
        let _ = self.evaluate("window.history.back()").await;
        self.wait_ready_state(Duration::from_secs(10)).await;
        Ok(())
    }

    /// Navigates forward in browser history
    pub async fn go_forward(&self) -> Result<(), String> {
        self.stealth.reset_context().await;
        let _ = self.evaluate("window.history.forward()").await;
        self.wait_ready_state(Duration::from_secs(10)).await;
        Ok(())
    }

    /// Returns the current page URL from the stealth isolated context
    pub async fn get_url(&self) -> Result<String, String> {
        let val = self.evaluate("window.location.href").await?;
        Ok(val.as_str().unwrap_or_default().to_string())
    }

    async fn wait_ready_state(&self, timeout: Duration) {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if let Ok(res) = self.stealth.evaluate_isolated("document.readyState").await {
                if res.as_str() == Some("complete") {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(120)).await;
        }
    }

    /// Finds element at index, moves mouse along a Bézier curve, and performs a natural click
    pub async fn click_at_index(&self, selector: &str, index: usize) -> Result<(), String> {
        let bbox = self
            .element
            .wait_for_selector_at(selector, index, Duration::from_secs(10))
            .await?;
        self.interaction.click_at(bbox.x, bbox.y).await
    }

    /// Finds element, moves mouse along a Bézier curve, and performs a natural click
    pub async fn click(&self, selector: &str) -> Result<(), String> {
        self.click_at_index(selector, 0).await
    }

    /// Moves mouse pointer over the matched element along a Bézier curve
    pub async fn hover_at_index(&self, selector: &str, index: usize) -> Result<(), String> {
        let bbox = self
            .element
            .wait_for_selector_at(selector, index, Duration::from_secs(10))
            .await?;
        self.interaction.move_mouse(bbox.x, bbox.y).await
    }

    /// Focuses an input element at index and types text with natural human keystroke cadence
    pub async fn type_text_at_index(
        &self,
        selector: &str,
        index: usize,
        text: &str,
        clear_value: bool,
        direct_insert: bool,
    ) -> Result<(), String> {
        self.click_at_index(selector, index).await?;
        if clear_value {
            let _ = self
                .element
                .manipulate_form_element(selector, index, "text-field", "", true, None)
                .await?;
        }
        if direct_insert {
            self.interaction.insert_text(text).await?;
        } else {
            self.interaction.type_text(text).await?;
        }
        Ok(())
    }

    /// Focuses an input element and types text with natural human keystroke cadence
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), String> {
        self.type_text_at_index(selector, 0, text, false, false).await
    }

    /// Dispatches a hardware key press (e.g. "Enter", "Tab", "Escape")
    pub async fn press_key(&self, key: &str) -> Result<(), String> {
        self.interaction.press_key(key).await
    }

    /// Smooth human-like mouse wheel scrolling (delta_x, delta_y in CSS pixels)
    pub async fn scroll(&self, delta_x: f64, delta_y: f64) -> Result<(), String> {
        self.interaction.scroll(delta_x, delta_y).await
    }

    /// Convenience helper to scroll down by pixels
    pub async fn scroll_down(&self, pixels: f64) -> Result<(), String> {
        self.interaction.scroll(0.0, pixels.abs()).await
    }

    /// Convenience helper to scroll up by pixels
    pub async fn scroll_up(&self, pixels: f64) -> Result<(), String> {
        self.interaction.scroll(0.0, -pixels.abs()).await
    }

    /// Captures a screenshot (base64 PNG) via CDP Page.captureScreenshot
    pub async fn capture_screenshot(&self, format: &str) -> Result<String, String> {
        let fmt = if format.is_empty() { "png" } else { format };
        let res = self
            .connection
            .send_command("Page.captureScreenshot", json!({ "format": fmt }))
            .await?;
        Ok(res
            .get("data")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string())
    }

    /// Retrieves cookies via Network.getCookies
    pub async fn get_cookies(&self, urls: Option<Vec<String>>) -> Result<Value, String> {
        let params = match urls {
            Some(u) if !u.is_empty() => json!({ "urls": u }),
            _ => json!({}),
        };
        let res = self
            .connection
            .send_command("Network.getCookies", params)
            .await?;
        Ok(res.get("cookies").cloned().unwrap_or(Value::Array(vec![])))
    }

    /// Sets a cookie via Network.setCookie
    pub async fn set_cookie(&self, cookie: Value) -> Result<bool, String> {
        let res = self
            .connection
            .send_command("Network.setCookie", cookie)
            .await?;
        Ok(res.get("success").and_then(|v| v.as_bool()).unwrap_or(true))
    }

    /// Deletes matching cookies via Network.deleteCookies
    pub async fn delete_cookies(&self, name: &str, url: Option<&str>, domain: Option<&str>) -> Result<(), String> {
        let mut params = json!({ "name": name });
        if let Some(u) = url {
            if !u.is_empty() {
                params["url"] = json!(u);
            }
        }
        if let Some(d) = domain {
            if !d.is_empty() {
                params["domain"] = json!(d);
            }
        }
        self.connection
            .send_command("Network.deleteCookies", params)
            .await?;
        Ok(())
    }

    /// Checks if Cloudflare Turnstile has already completed and generated a valid token
    pub async fn is_turnstile_solved(&self) -> Result<bool, String> {
        self.element.is_turnstile_solved().await
    }

    /// Detects and solves Cloudflare Turnstile CAPTCHA automatically
    pub async fn solve_turnstile(&self, timeout: Duration) -> Result<bool, String> {
        let start = std::time::Instant::now();
        info!("Scanning for Cloudflare Turnstile challenge...");

        let mut last_click_at: Option<std::time::Instant> = None;

        while start.elapsed() < timeout {
            if self.is_turnstile_solved().await.unwrap_or(false) {
                info!("Cloudflare Turnstile challenge verified (token issued)!");
                return Ok(true);
            }

            if let Ok(Some(bbox)) = self.element.find_turnstile_box().await {
                let should_click = match last_click_at {
                    None => true,
                    Some(prev) => prev.elapsed() >= Duration::from_secs(4),
                };

                if should_click {
                    info!("Turnstile challenge located at ({:.1}, {:.1}). Executing humanized checkbox click...", bbox.x, bbox.y);
                    self.interaction.click_at(bbox.x, bbox.y).await?;
                    last_click_at = Some(std::time::Instant::now());
                }
            }

            tokio::time::sleep(Duration::from_millis(300)).await;
        }

        let solved = self.is_turnstile_solved().await.unwrap_or(false);
        if solved {
            info!("Cloudflare Turnstile challenge verified (token issued)!");
        }
        Ok(solved)
    }

    /// Evaluates JavaScript within the stealth Isolated World
    pub async fn evaluate(&self, script: &str) -> Result<serde_json::Value, String> {
        self.stealth.evaluate_isolated(script).await
    }
}
