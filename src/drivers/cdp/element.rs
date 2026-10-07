use crate::drivers::cdp::stealth::StealthEngine;
use serde::Deserialize;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Deserialize)]
pub struct ElementBoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub visible: bool,
}

pub struct ElementEngine {
    stealth: Arc<StealthEngine>,
}

impl ElementEngine {
    pub fn new(stealth: Arc<StealthEngine>) -> Self {
        Self { stealth }
    }

    /// Queries element coordinates strictly inside the stealth isolated context
    pub async fn query_selector(&self, selector: &str) -> Result<Option<ElementBoundingBox>, String> {
        // Safe string escaping for injection into isolated JS function
        let escaped_selector = selector.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!(
            r#"(() => {{
                const el = document.querySelector("{}");
                if (!el) return null;
                const rect = el.getBoundingClientRect();
                return {{
                    x: rect.left + rect.width / 2,
                    y: rect.top + rect.height / 2,
                    width: rect.width,
                    height: rect.height,
                    visible: rect.width > 0 && rect.height > 0 && window.getComputedStyle(el).visibility !== 'hidden'
                }};
            }})()"#,
            escaped_selector
        );

        let val = self.stealth.evaluate_isolated(&script).await?;
        if val.is_null() {
            Ok(None)
        } else {
            let bbox: ElementBoundingBox = serde_json::from_value(val)
                .map_err(|e| format!("Failed to parse ElementBoundingBox: {}", e))?;
            Ok(Some(bbox))
        }
    }

    /// Auto-waits for a selector to appear and become visible within timeout
    pub async fn wait_for_selector(
        &self,
        selector: &str,
        timeout: Duration,
    ) -> Result<ElementBoundingBox, String> {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            if let Ok(Some(bbox)) = self.query_selector(selector).await {
                if bbox.visible {
                    return Ok(bbox);
                }
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Err(format!(
            "Timeout waiting for selector '{}' after {:?}",
            selector, timeout
        ))
    }

    /// Detects Cloudflare Turnstile challenge iframes and computes the interactive checkbox target
    pub async fn find_turnstile_box(&self) -> Result<Option<ElementBoundingBox>, String> {
        let script = r#"(() => {
            // 1. Direct or open shadow root iframes
            const iframes = Array.from(document.querySelectorAll("iframe"));
            for (const f of iframes) {
                const src = f.getAttribute("src") || "";
                const title = f.getAttribute("title") || "";
                const id = f.getAttribute("id") || "";
                if (src.includes("challenges.cloudflare.com") || src.includes("turnstile") || title.toLowerCase().includes("cloudflare") || id.toLowerCase().includes("cf-turnstile")) {
                    f.scrollIntoView({ block: 'center' });
                    const rect = f.getBoundingClientRect();
                    // Turnstile standard widget is 300x65px
                    if (rect.width >= 200 && rect.height >= 55) {
                        return {
                            x: rect.left + 28,
                            y: rect.top + 32,
                            width: rect.width,
                            height: rect.height,
                            visible: true
                        };
                    }
                }
            }

            // 2. Declarative Closed Shadow DOM host inside Cloudflare Turnstile containers
            const cfContainers = Array.from(document.querySelectorAll(".cf-turnstile, [data-sitekey]"));
            for (const c of cfContainers) {
                // Find inner shadow host wrapper (div with width ~300 and height ~65)
                const innerDivs = Array.from(c.querySelectorAll("div"));
                for (const d of innerDivs) {
                    const rect = d.getBoundingClientRect();
                    // Match rendered widget dimensions (300x65px)
                    if (rect.width >= 200 && rect.height >= 55 && rect.height <= 120) {
                        d.scrollIntoView({ block: 'center' });
                        const updatedRect = d.getBoundingClientRect();
                        return {
                            x: updatedRect.left + 28,
                            y: updatedRect.top + 32,
                            width: updatedRect.width,
                            height: updatedRect.height,
                            visible: true
                        };
                    }
                }

                // Fallback: If container itself has rendered dimensions >= 55px height
                const cRect = c.getBoundingClientRect();
                if (cRect.width >= 200 && cRect.height >= 55) {
                    c.scrollIntoView({ block: 'center' });
                    const updated = c.getBoundingClientRect();
                    return {
                        x: updated.left + 28,
                        y: updated.top + 32,
                        width: updated.width,
                        height: updated.height,
                        visible: true
                    };
                }
            }

            return null;
        })()"#;

        let val = self.stealth.evaluate_isolated(script).await?;
        if val.is_null() {
            Ok(None)
        } else {
            let bbox: ElementBoundingBox = serde_json::from_value(val)
                .map_err(|e| format!("Failed to parse Turnstile BoundingBox: {}", e))?;
            Ok(Some(bbox))
        }
    }

    /// Verifies if the Cloudflare Turnstile challenge has been passed (response token generated)
    pub async fn is_turnstile_solved(&self) -> Result<bool, String> {
        let script = r#"(() => {
            const resp = document.querySelector('[name="cf-turnstile-response"]');
            if (resp && resp.value && resp.value.length > 20) return true;
            const successEl = document.querySelector('.cf-turnstile-success, [data-state="success"]');
            if (successEl) return true;
            return false;
        })()"#;

        let val = self.stealth.evaluate_isolated(script).await?;
        Ok(val.as_bool().unwrap_or(false))
    }
}
