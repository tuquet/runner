use crate::drivers::cdp::CdpSession;
use crate::protocol::schema::AutomaStepLog;
use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

#[async_trait]
pub trait BrowserBackend: Send + Sync {
    async fn navigate(&self, url: &str) -> Result<(), String>;
    async fn reload(&self) -> Result<(), String>;
    async fn go_back(&self) -> Result<(), String>;
    async fn go_forward(&self) -> Result<(), String>;
    async fn get_url(&self) -> Result<String, String>;
    async fn click_at_index(&self, selector: &str, index: usize) -> Result<(), String>;
    async fn hover_at_index(&self, selector: &str, index: usize) -> Result<(), String>;
    async fn type_text_at_index(
        &self,
        selector: &str,
        index: usize,
        text: &str,
        clear: bool,
        direct: bool,
    ) -> Result<(), String>;
    async fn press_key(&self, key: &str) -> Result<(), String>;
    async fn scroll(&self, dx: f64, dy: f64) -> Result<(), String>;
    async fn scroll_element(
        &self,
        selector: &str,
        index: usize,
        dx: f64,
        dy: f64,
        into_view: bool,
        smooth: bool,
    ) -> Result<bool, String>;
    async fn get_text(
        &self,
        selector: &str,
        index: Option<usize>,
        multiple: bool,
        include_tags: bool,
    ) -> Result<Value, String>;
    async fn get_attribute(
        &self,
        selector: &str,
        attr: &str,
        index: Option<usize>,
        multiple: bool,
    ) -> Result<Value, String>;
    async fn element_exists(&self, selector: &str, timeout: Duration) -> Result<bool, String>;
    async fn count_elements(&self, selector: &str) -> Result<usize, String>;
    async fn manipulate_form_element(
        &self,
        selector: &str,
        index: usize,
        form_type: &str,
        value: &str,
        clear: bool,
        selected: Option<bool>,
    ) -> Result<Value, String>;
    async fn capture_screenshot(&self, format: &str) -> Result<String, String>;
    async fn get_cookies(&self, urls: Option<Vec<String>>) -> Result<Value, String>;
    async fn set_cookie(&self, cookie: Value) -> Result<bool, String>;
    async fn delete_cookies(
        &self,
        name: &str,
        url: Option<&str>,
        domain: Option<&str>,
    ) -> Result<(), String>;
    async fn evaluate(&self, script: &str) -> Result<Value, String>;
    async fn solve_turnstile(&self, timeout: Duration) -> Result<bool, String>;
}

#[async_trait]
impl BrowserBackend for CdpSession {
    async fn navigate(&self, url: &str) -> Result<(), String> {
        self.navigate(url).await
    }
    async fn reload(&self) -> Result<(), String> {
        self.reload().await
    }
    async fn go_back(&self) -> Result<(), String> {
        self.go_back().await
    }
    async fn go_forward(&self) -> Result<(), String> {
        self.go_forward().await
    }
    async fn get_url(&self) -> Result<String, String> {
        self.get_url().await
    }
    async fn click_at_index(&self, selector: &str, index: usize) -> Result<(), String> {
        self.click_at_index(selector, index).await
    }
    async fn hover_at_index(&self, selector: &str, index: usize) -> Result<(), String> {
        self.hover_at_index(selector, index).await
    }
    async fn type_text_at_index(
        &self,
        selector: &str,
        index: usize,
        text: &str,
        clear: bool,
        direct: bool,
    ) -> Result<(), String> {
        self.type_text_at_index(selector, index, text, clear, direct)
            .await
    }
    async fn press_key(&self, key: &str) -> Result<(), String> {
        self.press_key(key).await
    }
    async fn scroll(&self, dx: f64, dy: f64) -> Result<(), String> {
        self.scroll(dx, dy).await
    }
    async fn scroll_element(
        &self,
        selector: &str,
        index: usize,
        dx: f64,
        dy: f64,
        into_view: bool,
        smooth: bool,
    ) -> Result<bool, String> {
        self.element_engine()
            .scroll_element(selector, index, dx, dy, into_view, smooth)
            .await
    }
    async fn get_text(
        &self,
        selector: &str,
        index: Option<usize>,
        multiple: bool,
        include_tags: bool,
    ) -> Result<Value, String> {
        self.element_engine()
            .get_text(selector, index, multiple, include_tags)
            .await
    }
    async fn get_attribute(
        &self,
        selector: &str,
        attr: &str,
        index: Option<usize>,
        multiple: bool,
    ) -> Result<Value, String> {
        self.element_engine()
            .get_attribute(selector, attr, index, multiple)
            .await
    }
    async fn element_exists(&self, selector: &str, timeout: Duration) -> Result<bool, String> {
        self.element_engine()
            .element_exists(selector, timeout)
            .await
    }
    async fn count_elements(&self, selector: &str) -> Result<usize, String> {
        self.element_engine().count_elements(selector).await
    }
    async fn manipulate_form_element(
        &self,
        selector: &str,
        index: usize,
        form_type: &str,
        value: &str,
        clear: bool,
        selected: Option<bool>,
    ) -> Result<Value, String> {
        self.element_engine()
            .manipulate_form_element(selector, index, form_type, value, clear, selected)
            .await
    }
    async fn capture_screenshot(&self, format: &str) -> Result<String, String> {
        self.capture_screenshot(format).await
    }
    async fn get_cookies(&self, urls: Option<Vec<String>>) -> Result<Value, String> {
        self.get_cookies(urls).await
    }
    async fn set_cookie(&self, cookie: Value) -> Result<bool, String> {
        self.set_cookie(cookie).await
    }
    async fn delete_cookies(
        &self,
        name: &str,
        url: Option<&str>,
        domain: Option<&str>,
    ) -> Result<(), String> {
        self.delete_cookies(name, url, domain).await
    }
    async fn evaluate(&self, script: &str) -> Result<Value, String> {
        self.evaluate(script).await
    }
    async fn solve_turnstile(&self, timeout: Duration) -> Result<bool, String> {
        self.solve_turnstile(timeout).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowSettings {
    #[serde(default, alias = "debugMode")]
    pub debug_mode: bool,
    #[serde(default, alias = "blockDelay")]
    pub block_delay: u64,
    #[serde(default, alias = "onError")]
    pub on_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    #[serde(default)]
    pub id: Option<String>,
    pub source: String,
    pub target: String,
    #[serde(default, alias = "sourceHandle")]
    pub source_handle: Option<String>,
    #[serde(default, alias = "targetHandle")]
    pub target_handle: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConnectionTarget {
    pub target_node_id: String,
    pub target_handle: String,
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowGraph {
    pub name: String,
    pub nodes: HashMap<String, WorkflowNode>,
    /// Maps "{source_id}-output-{output_index_or_name}" -> Vec<ConnectionTarget>
    pub connections: HashMap<String, Vec<ConnectionTarget>>,
    pub trigger_id: Option<String>,
    pub settings: WorkflowSettings,
    pub global_data: HashMap<String, Value>,
}

impl WorkflowGraph {
    pub fn from_json(val: &Value) -> Result<Self, String> {
        let name = val
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Untitled Workflow")
            .to_string();

        let settings: WorkflowSettings = val
            .get("settings")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let mut raw_nodes: Vec<WorkflowNode> = Vec::new();
        let mut raw_edges: Vec<WorkflowEdge> = Vec::new();

        if let Some(drawflow) = val.get("drawflow") {
            if let Some(nodes_val) = drawflow.get("nodes") {
                if let Ok(nodes) = serde_json::from_value::<Vec<WorkflowNode>>(nodes_val.clone()) {
                    raw_nodes = nodes;
                }
            }
            if let Some(edges_val) = drawflow.get("edges") {
                if let Ok(edges) = serde_json::from_value::<Vec<WorkflowEdge>>(edges_val.clone()) {
                    raw_edges = edges;
                }
            }
        }

        if raw_nodes.is_empty() {
            if let Some(nodes_val) = val.get("nodes") {
                if let Ok(nodes) = serde_json::from_value::<Vec<WorkflowNode>>(nodes_val.clone()) {
                    raw_nodes = nodes;
                }
            }
        }
        if raw_edges.is_empty() {
            if let Some(edges_val) = val.get("edges") {
                if let Ok(edges) = serde_json::from_value::<Vec<WorkflowEdge>>(edges_val.clone()) {
                    raw_edges = edges;
                }
            }
        }

        let mut nodes = HashMap::new();
        let mut trigger_id = None;

        for node in raw_nodes {
            if node.label == "trigger" && trigger_id.is_none() {
                trigger_id = Some(node.id.clone());
            }
            nodes.insert(node.id.clone(), node);
        }

        let mut connections: HashMap<String, Vec<ConnectionTarget>> = HashMap::new();
        for edge in raw_edges {
            let handle = edge
                .source_handle
                .clone()
                .unwrap_or_else(|| format!("{}-output-1", edge.source));
            connections
                .entry(handle)
                .or_default()
                .push(ConnectionTarget {
                    target_node_id: edge.target,
                    target_handle: edge.target_handle.unwrap_or_default(),
                });
        }

        let mut global_data = HashMap::new();
        if let Some(gd) = val.get("globalData") {
            if let Some(obj) = gd.as_object() {
                for (k, v) in obj {
                    global_data.insert(k.clone(), v.clone());
                }
            }
        }

        Ok(Self {
            name,
            nodes,
            connections,
            trigger_id,
            settings,
            global_data,
        })
    }
}

#[derive(Debug, Clone)]
pub struct LoopState {
    pub index: usize,
    pub block_id: String,
    pub items: Vec<Value>,
    pub max_loop: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionState {
    pub variables: HashMap<String, Value>,
    pub table: Vec<Map<String, Value>>,
    pub loop_list: HashMap<String, LoopState>,
    pub loop_data: HashMap<String, Value>,
    pub repeated_tasks: HashMap<String, u32>,
    pub prev_block_data: Value,
    pub active_tab_url: String,
    pub step_logs: Vec<AutomaStepLog>,
}

impl ExecutionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_variable(&mut self, name: &str, val: Value) {
        if let Some(stripped) = name.strip_prefix("$push:") {
            let var_name = stripped.trim();
            let entry = self
                .variables
                .entry(var_name.to_string())
                .or_insert_with(|| Value::Array(vec![]));
            if let Some(arr) = entry.as_array_mut() {
                arr.push(val);
            } else {
                let prev = entry.clone();
                *entry = Value::Array(vec![prev, val]);
            }
        } else {
            self.variables.insert(name.to_string(), val);
        }
    }

    pub fn add_data_to_column(&mut self, col_name: &str, val: Value) {
        let mut row = Map::new();
        row.insert(col_name.to_string(), val);
        self.table.push(row);
    }
}

pub struct WorkflowDagExecutor {
    graph: WorkflowGraph,
    backend: Arc<dyn BrowserBackend>,
}

impl WorkflowDagExecutor {
    pub fn new(graph: WorkflowGraph, backend: Arc<dyn BrowserBackend>) -> Self {
        Self { graph, backend }
    }

    /// Recursively interpolates `{{variables.x}}`, `{{table.0.col}}`, `{{loopData.id}}`, etc.
    pub fn interpolate_string(&self, input: &str, state: &ExecutionState) -> String {
        let re = Regex::new(r"\{\{([^{}]+)\}\}").unwrap();
        re.replace_all(input, |caps: &regex::Captures| {
            let expr = caps[1].trim();
            Self::resolve_expression(expr, state)
        })
        .to_string()
    }

    pub fn resolve_expression(expr: &str, state: &ExecutionState) -> String {
        if expr == "$timestamp" {
            return chrono::Utc::now().timestamp_millis().to_string();
        }
        if expr == "$date" {
            return chrono::Utc::now().format("%Y-%m-%d").to_string();
        }

        if let Some(var_expr) = expr
            .strip_prefix("variables.")
            .or_else(|| expr.strip_prefix("vars."))
        {
            if let Some(v) = state.variables.get(var_expr) {
                return match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
            }
            return String::new();
        }

        if let Some(loop_expr) = expr.strip_prefix("loopData.") {
            let parts: Vec<&str> = loop_expr.split('.').collect();
            if let Some(loop_id) = parts.first() {
                if let Some(ld) = state.loop_data.get(*loop_id) {
                    if parts.len() == 1 {
                        return ld.to_string();
                    }
                    if parts.len() >= 2 {
                        let sub = parts[1];
                        if sub == "$index" {
                            if let Some(idx) = ld.get("$index") {
                                return idx.to_string();
                            }
                        } else if sub == "data" {
                            if let Some(d) = ld.get("data") {
                                if parts.len() == 2 {
                                    return match d {
                                        Value::String(s) => s.clone(),
                                        other => other.to_string(),
                                    };
                                }
                                if parts.len() > 2 {
                                    let field = parts[2];
                                    if let Some(fv) = d.get(field) {
                                        return match fv {
                                            Value::String(s) => s.clone(),
                                            other => other.to_string(),
                                        };
                                    }
                                }
                            }
                        }
                    }
                }
            }
            return String::new();
        }

        if expr == "prevBlockData" {
            return match &state.prev_block_data {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
        }

        if let Some(stripped) = expr.strip_prefix("prevBlockData.") {
            if let Some(fv) = state.prev_block_data.get(stripped) {
                return match fv {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
            }
        }

        if let Some(gd_expr) = expr.strip_prefix("globalData.") {
            if let Some(v) = state.variables.get(gd_expr) {
                return match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
            }
        }

        String::new()
    }

    pub fn interpolate_value(&self, val: &Value, state: &ExecutionState) -> Value {
        match val {
            Value::String(s) => {
                let trimmed = s.trim();
                if trimmed.starts_with("{{")
                    && trimmed.ends_with("}}")
                    && !trimmed[2..trimmed.len() - 2].contains("{{")
                {
                    let expr = trimmed[2..trimmed.len() - 2].trim();
                    if let Some(var_expr) = expr
                        .strip_prefix("variables.")
                        .or_else(|| expr.strip_prefix("vars."))
                    {
                        if let Some(v) = state.variables.get(var_expr) {
                            return v.clone();
                        }
                    }
                    if expr == "prevBlockData" {
                        return state.prev_block_data.clone();
                    }
                }
                Value::String(self.interpolate_string(s, state))
            }
            Value::Array(arr) => Value::Array(
                arr.iter()
                    .map(|item| self.interpolate_value(item, state))
                    .collect(),
            ),
            Value::Object(obj) => {
                let mut out = Map::new();
                for (k, v) in obj {
                    out.insert(k.clone(), self.interpolate_value(v, state));
                }
                Value::Object(out)
            }
            other => other.clone(),
        }
    }

    pub fn execute<'a>(
        &'a self,
        initial_state: Option<ExecutionState>,
        cancel_token: CancellationToken,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<ExecutionState, String>> + Send + 'a>,
    > {
        Box::pin(async move {
            let (state, _) = self
                .execute_with_io(initial_state, None, HashMap::new(), cancel_token)
                .await?;
            Ok(state)
        })
    }

    #[allow(clippy::type_complexity)]
    pub fn execute_with_io<'a>(
        &'a self,
        initial_state: Option<ExecutionState>,
        entry_node_id: Option<String>,
        package_outputs: HashMap<String, String>,
        cancel_token: CancellationToken,
    ) -> std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<(ExecutionState, String), String>> + Send + 'a>,
    > {
        Box::pin(async move {
            let mut state = initial_state.unwrap_or_default();

            let start_node_id = entry_node_id
                .filter(|id| self.graph.nodes.contains_key(id))
                .or_else(|| self.graph.trigger_id.clone())
                .or_else(|| {
                    let targets: std::collections::HashSet<&str> = self
                        .graph
                        .connections
                        .values()
                        .flat_map(|v| v.iter().map(|t| t.target_node_id.as_str()))
                        .collect();
                    self.graph
                        .nodes
                        .keys()
                        .find(|k| !targets.contains(k.as_str()))
                        .cloned()
                })
                .or_else(|| self.graph.nodes.keys().next().cloned())
                .ok_or_else(|| "Workflow contains no nodes".to_string())?;

            let mut next_queue: Vec<(String, Option<String>)> = vec![(start_node_id, None)];
            let mut matched_package_output: Option<String> = None;

            while let Some((current_node_id, incoming_target_handle)) = next_queue.pop() {
                if cancel_token.is_cancelled() {
                    return Err("Execution cancelled".to_string());
                }

                let node = match self.graph.nodes.get(&current_node_id) {
                    Some(n) => n.clone(),
                    None => {
                        warn!("Target node '{}' not found in graph", current_node_id);
                        continue;
                    }
                };

                let start_time = std::time::Instant::now();
                let interpolated_data = self.interpolate_value(&node.data, &state);

                let res = self
                    .execute_block(
                        &node.id,
                        incoming_target_handle.as_deref(),
                        &node.label,
                        &interpolated_data,
                        &mut state,
                        &cancel_token,
                    )
                    .await;

                let duration_ms = start_time.elapsed().as_millis() as u64;

                match res {
                    Ok((block_out, branch)) => {
                        state.prev_block_data = block_out;
                        state.step_logs.push(AutomaStepLog {
                            id: uuid::Uuid::new_v4().to_string(),
                            name: node.label.clone(),
                            block_id: Some(node.id.clone()),
                            status: "success".to_string(),
                            duration_ms: Some(duration_ms),
                            message: None,
                        });

                        // Resolve outgoing connections: if branch matches a node ID directly (e.g. loop block id), jump to it directly
                        if self.graph.nodes.contains_key(&branch) {
                            next_queue.push((branch, None));
                        } else {
                            let output_handle = if branch.starts_with(&node.id) {
                                branch.clone()
                            } else {
                                format!("{}-output-{}", node.id, branch)
                            };

                            if let Some(pkg_out_id) = package_outputs.get(&output_handle) {
                                matched_package_output = Some(pkg_out_id.clone());
                            }

                            if let Some(targets) = self.graph.connections.get(&output_handle) {
                                for t in targets.iter().rev() {
                                    next_queue.push((
                                        t.target_node_id.clone(),
                                        if t.target_handle.is_empty() {
                                            None
                                        } else {
                                            Some(t.target_handle.clone())
                                        },
                                    ));
                                }
                            } else if branch == "1" {
                                if let Some(targets) =
                                    self.graph.connections.get(&format!("{}-output-1", node.id))
                                {
                                    for t in targets.iter().rev() {
                                        next_queue.push((
                                            t.target_node_id.clone(),
                                            if t.target_handle.is_empty() {
                                                None
                                            } else {
                                                Some(t.target_handle.clone())
                                            },
                                        ));
                                    }
                                }
                            }
                        }

                        if self.graph.settings.block_delay > 0 {
                            tokio::time::sleep(Duration::from_millis(
                                self.graph.settings.block_delay,
                            ))
                            .await;
                        }
                    }
                    Err(err) => {
                        state.step_logs.push(AutomaStepLog {
                            id: uuid::Uuid::new_v4().to_string(),
                            name: node.label.clone(),
                            block_id: Some(node.id.clone()),
                            status: "error".to_string(),
                            duration_ms: Some(duration_ms),
                            message: Some(err.clone()),
                        });

                        // Handle onError policy
                        let on_error = node
                            .data
                            .pointer("/onError/toDo")
                            .and_then(|v| v.as_str())
                            .or(self.graph.settings.on_error.as_deref())
                            .unwrap_or("error");

                        let fallback_handle = format!("{}-output-fallback", node.id);
                        if let Some(pkg_out_id) = package_outputs.get(&fallback_handle) {
                            matched_package_output = Some(pkg_out_id.clone());
                            if on_error == "fallback" {
                                if let Some(targets) = self.graph.connections.get(&fallback_handle)
                                {
                                    for t in targets.iter().rev() {
                                        next_queue.push((
                                            t.target_node_id.clone(),
                                            if t.target_handle.is_empty() {
                                                None
                                            } else {
                                                Some(t.target_handle.clone())
                                            },
                                        ));
                                    }
                                }
                                continue;
                            }
                        }

                        if on_error == "fallback" {
                            if let Some(targets) = self.graph.connections.get(&fallback_handle) {
                                for t in targets.iter().rev() {
                                    next_queue.push((
                                        t.target_node_id.clone(),
                                        if t.target_handle.is_empty() {
                                            None
                                        } else {
                                            Some(t.target_handle.clone())
                                        },
                                    ));
                                }
                                continue;
                            }
                            if matched_package_output.is_some() {
                                continue;
                            }
                        } else if on_error == "continue" {
                            let def_handle = format!("{}-output-1", node.id);
                            if let Some(targets) = self.graph.connections.get(&def_handle) {
                                for t in targets.iter().rev() {
                                    next_queue.push((
                                        t.target_node_id.clone(),
                                        if t.target_handle.is_empty() {
                                            None
                                        } else {
                                            Some(t.target_handle.clone())
                                        },
                                    ));
                                }
                                continue;
                            }
                        }

                        return Err(format!(
                            "Block '{}' ({}) failed: {}",
                            node.label, node.id, err
                        ));
                    }
                }
            }

            Ok((
                state,
                matched_package_output.unwrap_or_else(|| "1".to_string()),
            ))
        })
    }

    async fn execute_block(
        &self,
        node_id: &str,
        incoming_target_handle: Option<&str>,
        label: &str,
        data: &Value,
        state: &mut ExecutionState,
        cancel_token: &CancellationToken,
    ) -> Result<(Value, String), String> {
        match label {
            "trigger" => Ok((Value::Null, "1".to_string())),
            "delay" => {
                let ms = data.get("time").and_then(|v| v.as_u64()).unwrap_or(1000);
                tokio::select! {
                    _ = tokio::time::sleep(Duration::from_millis(ms)) => Ok((Value::from(ms), "1".to_string())),
                    _ = cancel_token.cancelled() => Err("Cancelled during delay".to_string())
                }
            }
            "new-tab" | "tab-url" | "link" => {
                let url = data
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| "Missing 'url' parameter".to_string())?;
                self.backend.navigate(url).await?;
                state.active_tab_url = url.to_string();
                Ok((Value::String(url.to_string()), "1".to_string()))
            }
            "reload-tab" => {
                self.backend.reload().await?;
                Ok((Value::Null, "1".to_string()))
            }
            "go-back" => {
                self.backend.go_back().await?;
                Ok((Value::Null, "1".to_string()))
            }
            "forward-page" => {
                self.backend.go_forward().await?;
                Ok((Value::Null, "1".to_string()))
            }
            "active-tab" => {
                let url = self.backend.get_url().await.unwrap_or_default();
                state.active_tab_url = url.clone();
                Ok((Value::String(url), "1".to_string()))
            }
            "close-tab" => {
                let _ = self.backend.evaluate("window.close()").await;
                Ok((Value::Null, "1".to_string()))
            }
            "event-click" => {
                let (selector, index) = Self::parse_target_selector(data, state)?;
                self.backend.click_at_index(&selector, index).await?;
                Ok((Value::Null, "1".to_string()))
            }
            "hover-element" => {
                let (selector, index) = Self::parse_target_selector(data, state)?;
                self.backend.hover_at_index(&selector, index).await?;
                Ok((Value::Null, "1".to_string()))
            }
            "forms" => {
                let (selector, index) = Self::parse_target_selector(data, state)?;
                let ftype = data
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("text-field");
                let text = data.get("value").and_then(|v| v.as_str()).unwrap_or("");
                let clear_val = data
                    .get("clearValue")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let delay = data.get("delay").and_then(|v| v.as_u64()).unwrap_or(0);

                if ftype == "text-field" {
                    self.backend
                        .type_text_at_index(&selector, index, text, clear_val, delay == 0)
                        .await?;
                } else {
                    let sel_opt = data.get("selected").and_then(|v| v.as_bool());
                    self.backend
                        .manipulate_form_element(&selector, index, ftype, text, clear_val, sel_opt)
                        .await?;
                }
                Ok((Value::String(text.to_string()), "1".to_string()))
            }
            "press-key" => {
                let key = data.get("key").and_then(|v| v.as_str()).unwrap_or("Enter");
                self.backend.press_key(key).await?;
                Ok((Value::String(key.to_string()), "1".to_string()))
            }
            "element-scroll" => {
                let (selector, index) = Self::parse_target_selector(data, state)?;
                let into_view = data
                    .get("scrollIntoView")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let smooth = data.get("smooth").and_then(|v| v.as_bool()).unwrap_or(true);
                let sx = data.get("scrollX").and_then(|v| v.as_f64()).unwrap_or(0.0);
                let sy = data.get("scrollY").and_then(|v| v.as_f64()).unwrap_or(0.0);

                self.backend
                    .scroll_element(&selector, index, sx, sy, into_view, smooth)
                    .await?;
                Ok((Value::Null, "1".to_string()))
            }
            "get-text" => {
                let (selector, index) = Self::parse_target_selector(data, state)?;
                let multiple = data
                    .get("multiple")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let include_tags = data
                    .get("includeTags")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let val = self
                    .backend
                    .get_text(&selector, Some(index), multiple, include_tags)
                    .await?;

                Self::assign_extracted_data(data, val.clone(), state);
                Ok((val, "1".to_string()))
            }
            "attribute-value" => {
                let (selector, index) = Self::parse_target_selector(data, state)?;
                let attr = data
                    .get("attributeName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("href");
                let multiple = data
                    .get("multiple")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let val = self
                    .backend
                    .get_attribute(&selector, attr, Some(index), multiple)
                    .await?;

                Self::assign_extracted_data(data, val.clone(), state);
                Ok((val, "1".to_string()))
            }
            "element-exists" => {
                let (selector, _) = Self::parse_target_selector(data, state)?;
                let timeout_ms = data.get("timeout").and_then(|v| v.as_u64()).unwrap_or(3000);
                let exists = self
                    .backend
                    .element_exists(&selector, Duration::from_millis(timeout_ms))
                    .await?;

                let branch = if exists { "1" } else { "fallback" };
                Ok((Value::Bool(exists), branch.to_string()))
            }
            "javascript-code" => {
                let code = data.get("code").and_then(|v| v.as_str()).unwrap_or("");
                let val = self.backend.evaluate(code).await?;
                Self::assign_extracted_data(data, val.clone(), state);
                Ok((val, "1".to_string()))
            }
            "take-screenshot" => {
                let fmt = data.get("ext").and_then(|v| v.as_str()).unwrap_or("png");
                let b64 = self.backend.capture_screenshot(fmt).await?;
                Self::assign_extracted_data(data, Value::String(b64.clone()), state);
                Ok((Value::String(b64), "1".to_string()))
            }
            "cookie" => {
                let action = data
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("get-all");
                match action {
                    "get-all" => {
                        let cookies = self.backend.get_cookies(None).await?;
                        Self::assign_extracted_data(data, cookies.clone(), state);
                        Ok((cookies, "1".to_string()))
                    }
                    "set" => {
                        let c = data.get("cookie").cloned().unwrap_or(Value::Null);
                        let ok = self.backend.set_cookie(c).await?;
                        Ok((Value::Bool(ok), "1".to_string()))
                    }
                    "delete" => {
                        let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let url = data.get("url").and_then(|v| v.as_str());
                        self.backend.delete_cookies(name, url, None).await?;
                        Ok((Value::Null, "1".to_string()))
                    }
                    _ => Ok((Value::Null, "1".to_string())),
                }
            }
            "turnstile" => {
                let timeout_secs = data.get("timeout").and_then(|v| v.as_u64()).unwrap_or(20);
                let solved = self
                    .backend
                    .solve_turnstile(Duration::from_secs(timeout_secs))
                    .await?;
                let branch = if solved { "1" } else { "fallback" };
                Ok((Value::Bool(solved), branch.to_string()))
            }
            "repeat-task" => {
                let repeat_for = data.get("repeatFor").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let counter = state.repeated_tasks.entry(node_id.to_string()).or_insert(0);
                *counter += 1;

                if *counter > repeat_for {
                    state.repeated_tasks.remove(node_id);
                    Ok((Value::from(repeat_for), "1".to_string()))
                } else {
                    Ok((Value::from(*counter), "2".to_string()))
                }
            }
            "conditions" | "while-loop" => {
                let matched_branch = Self::evaluate_conditions(data, state);
                Ok((Value::Null, matched_branch))
            }
            "loop-data" => {
                let loop_id = data
                    .get("loopId")
                    .and_then(|v| v.as_str())
                    .unwrap_or(node_id);
                if let Some(cur) = state.loop_list.get_mut(loop_id) {
                    cur.index += 1;
                    let cur_idx = cur.index;
                    let val = cur.items.get(cur_idx).cloned().unwrap_or(Value::Null);
                    state.loop_data.insert(
                        loop_id.to_string(),
                        json!({
                            "data": val,
                            "$index": cur_idx
                        }),
                    );
                    Ok((val, "1".to_string()))
                } else {
                    let loop_through = data
                        .get("loopThrough")
                        .and_then(|v| v.as_str())
                        .unwrap_or("numbers");
                    let items = match loop_through {
                        "numbers" => {
                            let from = data.get("fromNumber").and_then(|v| v.as_i64()).unwrap_or(1);
                            let to = data.get("toNumber").and_then(|v| v.as_i64()).unwrap_or(5);
                            (from..=to).map(Value::from).collect()
                        }
                        "custom-data" => {
                            let raw = data
                                .get("loopData")
                                .and_then(|v| v.as_str())
                                .unwrap_or("[]");
                            serde_json::from_str::<Vec<Value>>(raw).unwrap_or_default()
                        }
                        "variable" => {
                            let vname = data
                                .get("variableName")
                                .and_then(|v| v.as_str())
                                .unwrap_or("");
                            state
                                .variables
                                .get(vname)
                                .and_then(|v| v.as_array())
                                .cloned()
                                .unwrap_or_default()
                        }
                        _ => vec![],
                    };

                    let first = items.first().cloned().unwrap_or(Value::Null);
                    state.loop_list.insert(
                        loop_id.to_string(),
                        LoopState {
                            index: 0,
                            block_id: node_id.to_string(),
                            items,
                            max_loop: data.get("maxLoop").and_then(|v| v.as_u64()).unwrap_or(0)
                                as usize,
                        },
                    );
                    state.loop_data.insert(
                        loop_id.to_string(),
                        json!({
                            "data": first,
                            "$index": 0
                        }),
                    );
                    Ok((first, "1".to_string()))
                }
            }
            "loop-elements" => {
                let loop_id = data
                    .get("loopId")
                    .and_then(|v| v.as_str())
                    .unwrap_or(node_id);
                if let Some(cur) = state.loop_list.get_mut(loop_id) {
                    cur.index += 1;
                    let cur_idx = cur.index;
                    let val = cur.items.get(cur_idx).cloned().unwrap_or(Value::Null);
                    state.loop_data.insert(
                        loop_id.to_string(),
                        json!({
                            "data": val,
                            "$index": cur_idx
                        }),
                    );
                    Ok((val, "1".to_string()))
                } else {
                    let selector = data.get("selector").and_then(|v| v.as_str()).unwrap_or("");
                    let count = self.backend.count_elements(selector).await.unwrap_or(0);
                    let max_loop =
                        data.get("maxLoop").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    let limit = if max_loop > 0 && max_loop < count {
                        max_loop
                    } else {
                        count
                    };

                    let items: Vec<Value> = (0..limit)
                        .map(|i| Value::String(format!("__specter_el_loop__:{selector}@{i}")))
                        .collect();

                    let first = items.first().cloned().unwrap_or(Value::Null);
                    state.loop_list.insert(
                        loop_id.to_string(),
                        LoopState {
                            index: 0,
                            block_id: node_id.to_string(),
                            items,
                            max_loop: limit,
                        },
                    );
                    state.loop_data.insert(
                        loop_id.to_string(),
                        json!({
                            "data": first,
                            "$index": 0
                        }),
                    );
                    Ok((first, "1".to_string()))
                }
            }
            "loop-breakpoint" => {
                let loop_id = data.get("loopId").and_then(|v| v.as_str()).unwrap_or("");
                let clear_loop = data
                    .get("clearLoop")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                if let Some(cur) = state.loop_list.get(loop_id) {
                    let has_more = cur.index + 1 < cur.items.len()
                        && (cur.max_loop == 0 || cur.index + 1 < cur.max_loop);

                    if !clear_loop && has_more {
                        // Return output branch pointing back to the loop block ID
                        let loop_block_id = cur.block_id.clone();
                        return Ok((Value::Null, loop_block_id));
                    }
                }

                state.loop_list.remove(loop_id);
                state.loop_data.remove(loop_id);
                Ok((Value::Null, "1".to_string()))
            }
            "insert-data" => {
                if let Some(arr) = data
                    .get("dataList")
                    .or_else(|| data.get("dataToInsert"))
                    .and_then(|v| v.as_array())
                {
                    for item in arr {
                        let name = item
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("column");
                        let val = item.get("value").cloned().unwrap_or(Value::Null);
                        let itype = item
                            .get("type")
                            .and_then(|v| v.as_str())
                            .unwrap_or("column");
                        if itype == "variable" {
                            state.set_variable(name, val);
                        } else {
                            state.add_data_to_column(name, val);
                        }
                    }
                }
                Ok((Value::Null, "1".to_string()))
            }
            "increase-variable" => {
                let name = data
                    .get("variableName")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let amount = data.get("value").and_then(|v| v.as_i64()).unwrap_or(1);
                let cur = state
                    .variables
                    .get(name)
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let next = cur + amount;
                state.set_variable(name, Value::from(next));
                Ok((Value::from(next), "1".to_string()))
            }
            "block-package" => {
                // Official Automa Package (`handlerBlockPackage.js`) execution:
                // 1. Resolve entry blockId from `data.inputs` using incoming `targetHandle` (`${pkgNodeId}-input-${inputId}`)
                // 2. Map internal `data.outputs[].handleId` -> external package output `output.id`
                if let Some(sub_val) = data.get("data").or_else(|| data.get("workflowData")) {
                    if let Ok(sub_graph) = WorkflowGraph::from_json(sub_val) {
                        let target_input_id = incoming_target_handle.and_then(|th| {
                            th.split("-input-").nth(1).or(if !th.is_empty() {
                                Some(th)
                            } else {
                                None
                            })
                        });

                        let entry_block_id = data
                            .get("inputs")
                            .and_then(|v| v.as_array())
                            .and_then(|inputs| {
                                if let Some(tid) = target_input_id {
                                    inputs
                                        .iter()
                                        .find(|item| {
                                            item.get("id").and_then(|v| v.as_str()) == Some(tid)
                                        })
                                        .or_else(|| inputs.first())
                                } else {
                                    inputs.first()
                                }
                            })
                            .and_then(|item| item.get("blockId").and_then(|v| v.as_str()))
                            .map(|s| s.to_string());

                        let mut package_outputs = HashMap::new();
                        if let Some(outputs) = data.get("outputs").and_then(|v| v.as_array()) {
                            for out_item in outputs {
                                if let Some(out_id) = out_item.get("id").and_then(|v| v.as_str()) {
                                    if let Some(handle_id) =
                                        out_item.get("handleId").and_then(|v| v.as_str())
                                    {
                                        if !handle_id.is_empty() {
                                            package_outputs
                                                .insert(handle_id.to_string(), out_id.to_string());
                                        } else if let Some(block_id) =
                                            out_item.get("blockId").and_then(|v| v.as_str())
                                        {
                                            package_outputs.insert(
                                                format!("{}-output-1", block_id),
                                                out_id.to_string(),
                                            );
                                        }
                                    } else if let Some(block_id) =
                                        out_item.get("blockId").and_then(|v| v.as_str())
                                    {
                                        package_outputs.insert(
                                            format!("{}-output-1", block_id),
                                            out_id.to_string(),
                                        );
                                    }
                                }
                            }
                        }

                        let sub_executor =
                            WorkflowDagExecutor::new(sub_graph, Arc::clone(&self.backend));
                        let (sub_state, out_branch) = sub_executor
                            .execute_with_io(
                                Some(state.clone()),
                                entry_block_id,
                                package_outputs,
                                cancel_token.clone(),
                            )
                            .await?;
                        state.variables = sub_state.variables;
                        state.table = sub_state.table;
                        state.prev_block_data = sub_state.prev_block_data;
                        state.active_tab_url = sub_state.active_tab_url;
                        state.step_logs = sub_state.step_logs;
                        return Ok((state.prev_block_data.clone(), out_branch));
                    }
                }
                Ok((Value::Null, "1".to_string()))
            }
            "webhook" => {
                let url = data.get("url").and_then(|v| v.as_str()).unwrap_or("");
                let method = data
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("POST");
                let body = data.get("body").cloned().unwrap_or(Value::Null);

                let client = reqwest::Client::new();
                let req = match method.to_uppercase().as_str() {
                    "GET" => client.get(url),
                    _ => client.post(url).json(&body),
                };
                let resp = req.send().await.map_err(|e| e.to_string())?;
                let text = resp.text().await.map_err(|e| e.to_string())?;
                let val: Value = serde_json::from_str(&text).unwrap_or(Value::String(text));
                Self::assign_extracted_data(data, val.clone(), state);
                Ok((val, "1".to_string()))
            }
            _ => {
                debug!("Bypassing unhandled or no-op block: {}", label);
                Ok((Value::Null, "1".to_string()))
            }
        }
    }

    fn parse_target_selector(
        data: &Value,
        state: &ExecutionState,
    ) -> Result<(String, usize), String> {
        let raw = data
            .get("selector")
            .or_else(|| data.get("elementSelector"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if raw.is_empty() {
            return Err("Missing target selector".to_string());
        }

        if let Some(stripped) = raw.strip_prefix("__specter_el_loop__:") {
            if let Some((base_sel, idx_str)) = stripped.rsplit_once('@') {
                let idx: usize = idx_str.parse().unwrap_or(0);
                return Ok((base_sel.to_string(), idx));
            }
        }

        // Check if an active loop elements context applies
        if let Some(loop_id) = data.get("loopId").and_then(|v| v.as_str()) {
            if let Some(loop_st) = state.loop_list.get(loop_id) {
                return Ok((raw.to_string(), loop_st.index));
            }
        }

        Ok((raw.to_string(), 0))
    }

    fn assign_extracted_data(data: &Value, val: Value, state: &mut ExecutionState) {
        if data
            .get("assignVariable")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            if let Some(var_name) = data.get("variableName").and_then(|v| v.as_str()) {
                if !var_name.is_empty() {
                    state.set_variable(var_name, val.clone());
                }
            }
        }
        if data
            .get("saveData")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
        {
            if let Some(col_name) = data.get("dataColumn").and_then(|v| v.as_str()) {
                if !col_name.is_empty() {
                    state.add_data_to_column(col_name, val);
                }
            }
        }
    }

    fn evaluate_conditions(data: &Value, state: &ExecutionState) -> String {
        let conds_arr = match data.get("conditions").and_then(|v| v.as_array()) {
            Some(a) => a,
            None => return "1".to_string(),
        };

        for (idx, item) in conds_arr.iter().enumerate() {
            let cond_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("==");
            let val_a = item.get("value").cloned().unwrap_or(Value::Null);
            let val_b = item
                .get("compareValue")
                .cloned()
                .unwrap_or(state.prev_block_data.clone());

            let is_match = match cond_type {
                "==" | "eq" => val_a == val_b,
                "!=" | "ne" => val_a != val_b,
                ">" | "gt" => val_a.as_f64().unwrap_or(0.0) > val_b.as_f64().unwrap_or(0.0),
                ">=" | "gte" => val_a.as_f64().unwrap_or(0.0) >= val_b.as_f64().unwrap_or(0.0),
                "<" | "lt" => val_a.as_f64().unwrap_or(0.0) < val_b.as_f64().unwrap_or(0.0),
                "<=" | "lte" => val_a.as_f64().unwrap_or(0.0) <= val_b.as_f64().unwrap_or(0.0),
                "()" | "contains" => {
                    let s_a = val_a.as_str().unwrap_or("");
                    let s_b = val_b.as_str().unwrap_or("");
                    s_a.contains(s_b)
                }
                "empty" => val_a.is_null() || val_a == Value::String(String::new()),
                _ => false,
            };

            if is_match {
                let out_id = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                return out_id.unwrap_or_else(|| (idx + 1).to_string());
            }
        }

        "fallback".to_string()
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::sync::Mutex;

    pub struct InMemoryMockBackend {
        pub url: Mutex<String>,
        pub clicks: Mutex<Vec<(String, usize)>>,
        pub typed: Mutex<Vec<(String, usize, String)>>,
    }

    impl InMemoryMockBackend {
        pub fn new() -> Self {
            Self {
                url: Mutex::new("about:blank".to_string()),
                clicks: Mutex::new(vec![]),
                typed: Mutex::new(vec![]),
            }
        }
    }

    impl Default for InMemoryMockBackend {
        fn default() -> Self {
            Self::new()
        }
    }

    #[async_trait]
    impl BrowserBackend for InMemoryMockBackend {
        async fn navigate(&self, url: &str) -> Result<(), String> {
            *self.url.lock().unwrap() = url.to_string();
            Ok(())
        }
        async fn reload(&self) -> Result<(), String> {
            Ok(())
        }
        async fn go_back(&self) -> Result<(), String> {
            Ok(())
        }
        async fn go_forward(&self) -> Result<(), String> {
            Ok(())
        }
        async fn get_url(&self) -> Result<String, String> {
            Ok(self.url.lock().unwrap().clone())
        }
        async fn click_at_index(&self, selector: &str, index: usize) -> Result<(), String> {
            self.clicks
                .lock()
                .unwrap()
                .push((selector.to_string(), index));
            Ok(())
        }
        async fn hover_at_index(&self, _selector: &str, _index: usize) -> Result<(), String> {
            Ok(())
        }
        async fn type_text_at_index(
            &self,
            selector: &str,
            index: usize,
            text: &str,
            _clear: bool,
            _direct: bool,
        ) -> Result<(), String> {
            self.typed
                .lock()
                .unwrap()
                .push((selector.to_string(), index, text.to_string()));
            Ok(())
        }
        async fn press_key(&self, _key: &str) -> Result<(), String> {
            Ok(())
        }
        async fn scroll(&self, _dx: f64, _dy: f64) -> Result<(), String> {
            Ok(())
        }
        async fn scroll_element(
            &self,
            _selector: &str,
            _index: usize,
            _dx: f64,
            _dy: f64,
            _into_view: bool,
            _smooth: bool,
        ) -> Result<bool, String> {
            Ok(true)
        }
        async fn get_text(
            &self,
            _selector: &str,
            _index: Option<usize>,
            _multiple: bool,
            _include_tags: bool,
        ) -> Result<Value, String> {
            Ok(Value::String("Scraped Text".to_string()))
        }
        async fn get_attribute(
            &self,
            _selector: &str,
            _attr: &str,
            _index: Option<usize>,
            _multiple: bool,
        ) -> Result<Value, String> {
            Ok(Value::String("https://example.com/item".to_string()))
        }
        async fn element_exists(&self, selector: &str, _timeout: Duration) -> Result<bool, String> {
            Ok(selector != "#missing")
        }
        async fn count_elements(&self, selector: &str) -> Result<usize, String> {
            if selector == ".item" {
                Ok(3)
            } else {
                Ok(1)
            }
        }
        async fn manipulate_form_element(
            &self,
            _selector: &str,
            _index: usize,
            _form_type: &str,
            _value: &str,
            _clear: bool,
            _selected: Option<bool>,
        ) -> Result<Value, String> {
            Ok(Value::Bool(true))
        }
        async fn capture_screenshot(&self, _format: &str) -> Result<String, String> {
            Ok("dGVzdF9zY3JlZW5zaG90".to_string())
        }
        async fn get_cookies(&self, _urls: Option<Vec<String>>) -> Result<Value, String> {
            Ok(json!([{"name": "test_cookie", "value": "123"}]))
        }
        async fn set_cookie(&self, _cookie: Value) -> Result<bool, String> {
            Ok(true)
        }
        async fn delete_cookies(
            &self,
            _name: &str,
            _url: Option<&str>,
            _domain: Option<&str>,
        ) -> Result<(), String> {
            Ok(())
        }
        async fn evaluate(&self, _script: &str) -> Result<Value, String> {
            Ok(Value::from(42))
        }
        async fn solve_turnstile(&self, _timeout: Duration) -> Result<bool, String> {
            Ok(true)
        }
    }

    #[tokio::test]
    async fn test_workflow_dag_linear_execution() {
        let wf_json = json!({
            "name": "Linear Test",
            "drawflow": {
                "nodes": [
                    { "id": "t1", "label": "trigger", "data": {} },
                    { "id": "n1", "label": "new-tab", "data": { "url": "https://example.com" } },
                    { "id": "c1", "label": "event-click", "data": { "selector": "button#submit" } }
                ],
                "edges": [
                    { "source": "t1", "target": "n1" },
                    { "source": "n1", "target": "c1" }
                ]
            }
        });

        let graph = WorkflowGraph::from_json(&wf_json).unwrap();
        let mock_backend = Arc::new(InMemoryMockBackend::new());
        let executor = WorkflowDagExecutor::new(graph, mock_backend.clone());

        let res = executor
            .execute(None, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(*mock_backend.url.lock().unwrap(), "https://example.com");
        let clicks = mock_backend.clicks.lock().unwrap();
        assert_eq!(clicks.len(), 1);
        assert_eq!(clicks[0].0, "button#submit");
        assert_eq!(res.step_logs.len(), 3);
    }

    #[tokio::test]
    async fn test_workflow_mustache_templating() {
        let wf_json = json!({
            "name": "Template Test",
            "nodes": [
                { "id": "t1", "label": "trigger", "data": {} },
                { "id": "f1", "label": "forms", "data": { "selector": "#search", "value": "{{variables.query}}" } }
            ],
            "edges": [
                { "source": "t1", "target": "f1" }
            ]
        });

        let graph = WorkflowGraph::from_json(&wf_json).unwrap();
        let mock_backend = Arc::new(InMemoryMockBackend::new());
        let executor = WorkflowDagExecutor::new(graph, mock_backend.clone());

        let mut initial_state = ExecutionState::new();
        initial_state.set_variable("query", Value::String("Specter Anti-Detect".to_string()));

        let _ = executor
            .execute(Some(initial_state), CancellationToken::new())
            .await
            .unwrap();
        let typed = mock_backend.typed.lock().unwrap();
        assert_eq!(typed.len(), 1);
        assert_eq!(typed[0].2, "Specter Anti-Detect");
    }

    #[tokio::test]
    async fn test_workflow_loop_elements_and_breakpoint() {
        let wf_json = json!({
            "name": "Loop Test",
            "nodes": [
                { "id": "t1", "label": "trigger", "data": {} },
                { "id": "l1", "label": "loop-elements", "data": { "loopId": "my_loop", "selector": ".item", "maxLoop": 3 } },
                { "id": "c1", "label": "event-click", "data": { "loopId": "my_loop", "selector": ".item" } },
                { "id": "b1", "label": "loop-breakpoint", "data": { "loopId": "my_loop" } }
            ],
            "edges": [
                { "source": "t1", "target": "l1" },
                { "source": "l1", "target": "c1" },
                { "source": "c1", "target": "b1" },
                { "source": "b1", "target": "l1", "sourceHandle": "b1-output-b1" }
            ]
        });

        let graph = WorkflowGraph::from_json(&wf_json).unwrap();
        let mock_backend = Arc::new(InMemoryMockBackend::new());
        let executor = WorkflowDagExecutor::new(graph, mock_backend.clone());

        let _ = executor
            .execute(None, CancellationToken::new())
            .await
            .unwrap();
        let clicks = mock_backend.clicks.lock().unwrap();
        assert_eq!(clicks.len(), 3);
        assert_eq!(clicks[0].1, 0);
        assert_eq!(clicks[1].1, 1);
        assert_eq!(clicks[2].1, 2);
    }

    #[tokio::test]
    async fn test_official_automa_block_package_multi_io() {
        let wf_json = json!({
            "name": "Official Package Multi-IO Test",
            "drawflow": {
                "nodes": [
                    { "id": "t1", "label": "trigger", "data": {} },
                    {
                        "id": "pkg_login",
                        "label": "block-package",
                        "type": "BlockPackage",
                        "data": {
                            "id": "pkg_auth_v1",
                            "name": "Auth Package",
                            "icon": "mdiPackageVariantClosed",
                            "settings": { "asBlock": true },
                            "inputs": [
                                {
                                    "id": "in_unused",
                                    "name": "Unused Entry",
                                    "blockId": "sub_unused",
                                    "handleId": "sub_unused-input-1"
                                },
                                {
                                    "id": "in_main",
                                    "name": "Main Entry",
                                    "blockId": "sub_open",
                                    "handleId": "sub_open-input-1"
                                }
                            ],
                            "outputs": [
                                {
                                    "id": "out_ok",
                                    "name": "Success",
                                    "blockId": "sub_check",
                                    "handleId": "sub_check-output-1"
                                },
                                {
                                    "id": "out_fail",
                                    "name": "Fallback",
                                    "blockId": "sub_check",
                                    "handleId": "sub_check-output-fallback"
                                }
                            ],
                            "data": {
                                "nodes": [
                                    {
                                        "id": "sub_unused",
                                        "label": "new-tab",
                                        "data": { "url": "https://unused.example.com" }
                                    },
                                    {
                                        "id": "sub_open",
                                        "label": "new-tab",
                                        "data": { "url": "https://auth.example.com" }
                                    },
                                    {
                                        "id": "sub_check",
                                        "label": "element-exists",
                                        "data": { "selector": "#missing", "tryCount": 1, "timeout": 10 }
                                    }
                                ],
                                "edges": [
                                    {
                                        "source": "sub_open",
                                        "target": "sub_check",
                                        "sourceHandle": "sub_open-output-1",
                                        "targetHandle": "sub_check-input-1"
                                    }
                                ]
                            }
                        }
                    },
                    {
                        "id": "after_ok",
                        "label": "insert-data",
                        "data": {
                            "dataList": [
                                { "type": "variable", "name": "authResult", "value": "ok" }
                            ]
                        }
                    },
                    {
                        "id": "after_fail",
                        "label": "insert-data",
                        "data": {
                            "dataList": [
                                { "type": "variable", "name": "authResult", "value": "fallback_taken" }
                            ]
                        }
                    }
                ],
                "edges": [
                    {
                        "source": "t1",
                        "target": "pkg_login",
                        "sourceHandle": "t1-output-1",
                        "targetHandle": "pkg_login-input-in_main"
                    },
                    {
                        "source": "pkg_login",
                        "target": "after_ok",
                        "sourceHandle": "pkg_login-output-out_ok",
                        "targetHandle": "after_ok-input-1"
                    },
                    {
                        "source": "pkg_login",
                        "target": "after_fail",
                        "sourceHandle": "pkg_login-output-out_fail",
                        "targetHandle": "after_fail-input-1"
                    }
                ]
            }
        });

        let graph = WorkflowGraph::from_json(&wf_json).unwrap();
        let mock_backend = Arc::new(InMemoryMockBackend::new());
        let executor = WorkflowDagExecutor::new(graph, mock_backend.clone());

        let state = executor
            .execute(None, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(
            *mock_backend.url.lock().unwrap(),
            "https://auth.example.com"
        );
        assert_eq!(
            state.variables.get("authResult"),
            Some(&Value::String("fallback_taken".to_string()))
        );
    }
}
