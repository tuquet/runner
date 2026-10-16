use crate::drivers::cdp::stealth::StealthEngine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// Generates a safe JavaScript helper inside the Isolated World that resolves CSS or XPath
    /// selectors into an array of matched DOM Elements without mutating any DOM attributes.
    fn js_resolve_elements_fn() -> &'static str {
        r#"
        function __specterQueryAll(rawSel) {
            if (!rawSel || typeof rawSel !== 'string') return [];
            const sel = rawSel.trim();
            const isXpath = sel.startsWith('/') || sel.startsWith('(/') || sel.startsWith('./') || sel.startsWith('xpath=');
            const cleanSel = sel.startsWith('xpath=') ? sel.slice(6) : sel;
            if (isXpath) {
                const snap = document.evaluate(cleanSel, document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
                const out = [];
                for (let i = 0; i < snap.snapshotLength; i++) {
                    const node = snap.snapshotItem(i);
                    if (node && node.nodeType === 1) out.push(node);
                }
                return out;
            }
            return Array.from(document.querySelectorAll(cleanSel));
        }
        "#
    }

    /// Queries element coordinates at a specific index strictly inside the stealth isolated context
    pub async fn query_selector_at(
        &self,
        selector: &str,
        index: usize,
    ) -> Result<Option<ElementBoundingBox>, String> {
        let sel_json = serde_json::to_string(selector).map_err(|e| e.to_string())?;
        let script = format!(
            r#"(() => {{
                {helper}
                const els = __specterQueryAll({sel});
                const el = els[{idx}];
                if (!el) return null;
                el.scrollIntoView({{ block: 'center', inline: 'nearest' }});
                const rect = el.getBoundingClientRect();
                const style = window.getComputedStyle(el);
                return {{
                    x: rect.left + rect.width / 2,
                    y: rect.top + rect.height / 2,
                    width: rect.width,
                    height: rect.height,
                    visible: rect.width > 0 && rect.height > 0 && style.visibility !== 'hidden' && style.display !== 'none'
                }};
            }})()"#,
            helper = Self::js_resolve_elements_fn(),
            sel = sel_json,
            idx = index
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

    /// Queries the first matching element's coordinates strictly inside the stealth isolated context
    pub async fn query_selector(&self, selector: &str) -> Result<Option<ElementBoundingBox>, String> {
        self.query_selector_at(selector, 0).await
    }

    /// Auto-waits for a selector at a specific index to appear and become visible within timeout
    pub async fn wait_for_selector_at(
        &self,
        selector: &str,
        index: usize,
        timeout: Duration,
    ) -> Result<ElementBoundingBox, String> {
        let start = std::time::Instant::now();
        loop {
            if let Ok(Some(bbox)) = self.query_selector_at(selector, index).await {
                if bbox.visible {
                    return Ok(bbox);
                }
            }
            if start.elapsed() >= timeout {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        Err(format!(
            "Timeout waiting for selector '{}' (index {}) after {:?}",
            selector, index, timeout
        ))
    }

    /// Auto-waits for a selector to appear and become visible within timeout
    pub async fn wait_for_selector(
        &self,
        selector: &str,
        timeout: Duration,
    ) -> Result<ElementBoundingBox, String> {
        self.wait_for_selector_at(selector, 0, timeout).await
    }

    /// Checks whether at least one matching element exists within timeout
    pub async fn element_exists(&self, selector: &str, timeout: Duration) -> Result<bool, String> {
        let start = std::time::Instant::now();
        loop {
            let count = self.count_elements(selector).await.unwrap_or(0);
            if count > 0 {
                return Ok(true);
            }
            if start.elapsed() >= timeout {
                return Ok(false);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Counts how many elements match a CSS or XPath selector without mutating the DOM
    pub async fn count_elements(&self, selector: &str) -> Result<usize, String> {
        let sel_json = serde_json::to_string(selector).map_err(|e| e.to_string())?;
        let script = format!(
            r#"(() => {{
                {helper}
                return __specterQueryAll({sel}).length;
            }})()"#,
            helper = Self::js_resolve_elements_fn(),
            sel = sel_json
        );
        let val = self.stealth.evaluate_isolated(&script).await?;
        Ok(val.as_u64().unwrap_or(0) as usize)
    }

    /// Extracts innerText/textContent from element(s) in the Isolated World
    pub async fn get_text(
        &self,
        selector: &str,
        index: Option<usize>,
        multiple: bool,
        include_tags: bool,
    ) -> Result<Value, String> {
        let sel_json = serde_json::to_string(selector).map_err(|e| e.to_string())?;
        let idx_val = index.map(|i| i as i64).unwrap_or(-1);
        let script = format!(
            r#"(() => {{
                {helper}
                const els = __specterQueryAll({sel});
                const extract = (el) => ({include_tags} ? el.innerHTML : (el.innerText ?? el.textContent ?? '')).trim();
                if ({multiple}) {{
                    return els.map(extract);
                }}
                const target = {idx} >= 0 ? els[{idx}] : els[0];
                if (!target) return null;
                return extract(target);
            }})()"#,
            helper = Self::js_resolve_elements_fn(),
            sel = sel_json,
            include_tags = include_tags,
            multiple = multiple,
            idx = idx_val
        );

        self.stealth.evaluate_isolated(&script).await
    }

    /// Extracts an attribute or property value from element(s) in the Isolated World
    pub async fn get_attribute(
        &self,
        selector: &str,
        attr_name: &str,
        index: Option<usize>,
        multiple: bool,
    ) -> Result<Value, String> {
        let sel_json = serde_json::to_string(selector).map_err(|e| e.to_string())?;
        let attr_json = serde_json::to_string(attr_name).map_err(|e| e.to_string())?;
        let idx_val = index.map(|i| i as i64).unwrap_or(-1);
        let script = format!(
            r#"(() => {{
                {helper}
                const els = __specterQueryAll({sel});
                const attr = {attr};
                const extract = (el) => {{
                    const v = el.getAttribute(attr);
                    if (v !== null && v !== undefined) return v;
                    return el[attr] !== undefined ? String(el[attr]) : null;
                }};
                if ({multiple}) {{
                    return els.map(extract);
                }}
                const target = {idx} >= 0 ? els[{idx}] : els[0];
                if (!target) return null;
                return extract(target);
            }})()"#,
            helper = Self::js_resolve_elements_fn(),
            sel = sel_json,
            attr = attr_json,
            multiple = multiple,
            idx = idx_val
        );

        self.stealth.evaluate_isolated(&script).await
    }

    /// Prepares or sets form element values (clear input, select dropdown, toggle checkbox/radio)
    pub async fn manipulate_form_element(
        &self,
        selector: &str,
        index: usize,
        form_type: &str,
        value: &str,
        clear_value: bool,
        selected: Option<bool>,
    ) -> Result<Value, String> {
        let sel_json = serde_json::to_string(selector).map_err(|e| e.to_string())?;
        let type_json = serde_json::to_string(form_type).map_err(|e| e.to_string())?;
        let val_json = serde_json::to_string(value).map_err(|e| e.to_string())?;
        let selected_json = match selected {
            Some(b) => json!(b),
            None => Value::Null,
        };

        let script = format!(
            r#"(() => {{
                {helper}
                const els = __specterQueryAll({sel});
                const el = els[{idx}];
                if (!el) return {{ ok: false, error: 'Element not found' }};
                const ftype = {ftype};
                const val = {val};
                const selFlag = {sel_flag};

                if (ftype === 'select') {{
                    el.value = val;
                    el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                    el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                    return {{ ok: true, value: el.value }};
                }}
                if (ftype === 'checkbox' || ftype === 'radio') {{
                    const nextState = selFlag !== null ? Boolean(selFlag) : !el.checked;
                    if (el.checked !== nextState) {{
                        el.checked = nextState;
                        el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                        el.dispatchEvent(new Event('change', {{ bubbles: true }}));
                    }}
                    return {{ ok: true, value: el.checked }};
                }}
                // text-field
                el.focus();
                if ({clear}) {{
                    el.value = '';
                    el.dispatchEvent(new Event('input', {{ bubbles: true }}));
                }}
                return {{ ok: true, value: el.value }};
            }})()"#,
            helper = Self::js_resolve_elements_fn(),
            sel = sel_json,
            idx = index,
            ftype = type_json,
            val = val_json,
            sel_flag = selected_json,
            clear = clear_value
        );

        self.stealth.evaluate_isolated(&script).await
    }

    /// Scrolls a specific element or the window in the Isolated World
    pub async fn scroll_element(
        &self,
        selector: &str,
        index: usize,
        scroll_x: f64,
        scroll_y: f64,
        into_view: bool,
        smooth: bool,
    ) -> Result<bool, String> {
        let sel_json = serde_json::to_string(selector).map_err(|e| e.to_string())?;
        let behavior = if smooth { "smooth" } else { "auto" };
        let script = format!(
            r#"(() => {{
                {helper}
                const els = __specterQueryAll({sel});
                const el = els[{idx}] || document.scrollingElement || document.documentElement;
                if (!el) return false;
                if ({into_view}) {{
                    el.scrollIntoView({{ behavior: '{behavior}', block: 'center' }});
                }} else {{
                    el.scrollTo({{ left: {sx}, top: {sy}, behavior: '{behavior}' }});
                }}
                return true;
            }})()"#,
            helper = Self::js_resolve_elements_fn(),
            sel = sel_json,
            idx = index,
            into_view = into_view,
            behavior = behavior,
            sx = scroll_x,
            sy = scroll_y
        );

        let val = self.stealth.evaluate_isolated(&script).await?;
        Ok(val.as_bool().unwrap_or(false))
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
                const innerDivs = Array.from(c.querySelectorAll("div"));
                for (const d of innerDivs) {
                    const rect = d.getBoundingClientRect();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_resolve_elements_helper_contains_xpath_and_css() {
        let helper = ElementEngine::js_resolve_elements_fn();
        assert!(helper.contains("document.evaluate"));
        assert!(helper.contains("document.querySelectorAll"));
        assert!(!helper.contains("setAttribute"));
    }
}
