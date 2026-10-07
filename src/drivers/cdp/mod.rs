pub mod connection;
pub mod element;
pub mod interaction;
pub mod stealth;

pub use connection::CdpConnection;
use element::ElementEngine;
use interaction::HumanInteractionEngine;
use stealth::StealthEngine;

use serde_json::json;
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

    /// Navigates to a target URL and waits for page ready state
    pub async fn navigate(&self, url: &str) -> Result<(), String> {
        self.stealth.reset_context().await;
        self.connection
            .send_command("Page.navigate", json!({ "url": url }))
            .await?;

        // Wait for readyState === 'complete' inside isolated world
        let start = std::time::Instant::now();
        while start.elapsed() < Duration::from_secs(15) {
            if let Ok(res) = self
                .stealth
                .evaluate_isolated("document.readyState")
                .await
            {
                if res.as_str() == Some("complete") {
                    debug!("Page navigation to '{}' completed", url);
                    return Ok(());
                }
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }

        Ok(())
    }

    /// Finds element, moves mouse along a Bézier curve, and performs a natural click
    pub async fn click(&self, selector: &str) -> Result<(), String> {
        let bbox = self
            .element
            .wait_for_selector(selector, Duration::from_secs(10))
            .await?;
        self.interaction.click_at(bbox.x, bbox.y).await?;
        Ok(())
    }

    /// Focuses an input element and types text with natural human keystroke cadence
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<(), String> {
        self.click(selector).await?;
        self.interaction.type_text(text).await?;
        Ok(())
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
            // 1. Check if token has already been generated
            if self.is_turnstile_solved().await.unwrap_or(false) {
                info!("Cloudflare Turnstile challenge verified (token issued)!");
                return Ok(true);
            }

            // 2. Scan for rendered Turnstile challenge box
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

        // Final verification check before returning
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
