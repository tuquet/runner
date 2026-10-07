use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for an execution job
pub type JobId = String;

/// Driver category to execute the job
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DriverType {
    Shell,
    Agent,
    Automa,
    Browser,
    Http,
    #[serde(untagged)]
    Custom(String),
}

impl std::fmt::Display for DriverType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DriverType::Shell => write!(f, "shell"),
            DriverType::Agent => write!(f, "agent"),
            DriverType::Automa => write!(f, "automa"),
            DriverType::Browser => write!(f, "browser"),
            DriverType::Http => write!(f, "http"),
            DriverType::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// Execution status of a job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JobStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    TimedOut,
}

/// Channel source of a log chunk
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogChannel {
    Stdout,
    Stderr,
    System,
}

/// Individual streaming log line or text chunk
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogChunk {
    pub job_id: JobId,
    pub timestamp: i64,
    pub channel: LogChannel,
    pub text: String,
}

impl LogChunk {
    pub fn new(job_id: impl Into<JobId>, channel: LogChannel, text: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
            timestamp: Utc::now().timestamp_millis(),
            channel,
            text: text.into(),
        }
    }
}

/// Universal job specification dispatchable to any driver
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub driver: DriverType,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: Option<HashMap<String, String>>,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: u64,
    #[serde(default)]
    pub created_at: Option<DateTime<Utc>>,
}

fn default_timeout_ms() -> u64 {
    300_000 // 5 minutes default
}

/// Final outcome of a job execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub job_id: JobId,
    pub status: JobStatus,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub output: Option<serde_json::Value>,
}

impl JobResult {
    pub fn success(job_id: impl Into<JobId>, duration_ms: u64, output: Option<serde_json::Value>) -> Self {
        Self {
            job_id: job_id.into(),
            status: JobStatus::Completed,
            exit_code: Some(0),
            duration_ms,
            error: None,
            output,
        }
    }

    pub fn failure(job_id: impl Into<JobId>, exit_code: Option<i32>, duration_ms: u64, error: impl Into<String>) -> Self {
        Self {
            job_id: job_id.into(),
            status: JobStatus::Failed,
            exit_code,
            duration_ms,
            error: Some(error.into()),
            output: None,
        }
    }

    pub fn timed_out(job_id: impl Into<JobId>, duration_ms: u64) -> Self {
        Self {
            job_id: job_id.into(),
            status: JobStatus::TimedOut,
            exit_code: Some(124),
            duration_ms,
            error: Some("Execution timed out".to_string()),
            output: None,
        }
    }

    pub fn cancelled(job_id: impl Into<JobId>, duration_ms: u64) -> Self {
        Self {
            job_id: job_id.into(),
            status: JobStatus::Cancelled,
            exit_code: Some(130),
            duration_ms,
            error: Some("Execution cancelled".to_string()),
            output: None,
        }
    }
}

impl Job {
    pub fn new(id: impl Into<JobId>, driver: DriverType, payload: serde_json::Value) -> Self {
        Self {
            id: id.into(),
            driver,
            payload,
            cwd: None,
            env: None,
            timeout_ms: default_timeout_ms(),
            created_at: Some(Utc::now()),
        }
    }

    /// Attempt to parse payload into strongly-typed AutomaJobPayload
    pub fn as_automa_payload(&self) -> Result<AutomaJobPayload, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }

    /// Attempt to parse payload into strongly-typed BrowserJobPayload
    pub fn as_browser_payload(&self) -> Result<BrowserJobPayload, serde_json::Error> {
        serde_json::from_value(self.payload.clone())
    }
}

fn default_true() -> bool {
    true
}

fn default_window_width() -> u32 {
    1280
}

fn default_window_height() -> u32 {
    720
}

/// Window dimensions specification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowSizeConfig {
    #[serde(default = "default_window_width")]
    pub width: u32,
    #[serde(default = "default_window_height")]
    pub height: u32,
}

impl Default for WindowSizeConfig {
    fn default() -> Self {
        Self {
            width: default_window_width(),
            height: default_window_height(),
        }
    }
}

/// Deterministic PRNG hardware fingerprint emulation options for Antidetect Chromium
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FingerprintConfig {
    /// Deterministic 32-bit PRNG seed (--fingerprint=<seed>)
    #[serde(default)]
    pub seed: Option<u32>,
    /// Target OS platform (--fingerprint-platform: windows, macos, linux)
    #[serde(default)]
    pub platform: Option<String>,
    /// Operating system version string (--fingerprint-platform-version: "10.0.0", "15.2.0")
    #[serde(default, alias = "platformVersion")]
    pub platform_version: Option<String>,
    /// Browser brand name (--fingerprint-brand: Chrome, Edge, Opera)
    #[serde(default)]
    pub brand: Option<String>,
    /// Browser brand version string (--fingerprint-brand-version: "148.0.7778.215")
    #[serde(default, alias = "brandVersion")]
    pub brand_version: Option<String>,
    /// CPU core count override (--fingerprint-hardware-concurrency: 8, 16)
    #[serde(default, alias = "hardwareConcurrency")]
    pub hardware_concurrency: Option<u32>,
    /// Timezone override (--timezone: "Asia/Ho_Chi_Minh", "UTC")
    #[serde(default)]
    pub timezone: Option<String>,
    /// UI language locale (--lang: "vi-VN", "en-US")
    #[serde(default)]
    pub lang: Option<String>,
    /// HTTP Accept-Language header (--accept-lang: "vi-VN,vi,en-US,en")
    #[serde(default, alias = "acceptLang")]
    pub accept_lang: Option<String>,
}

/// Proxy network routing options for Tuquet Bridge & Chromium
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Proxy server address (e.g. "socks5://127.0.0.1:1080" or "http://127.0.0.1:8118")
    pub server: String,
    /// Optional authentication username
    #[serde(default)]
    pub username: Option<String>,
    /// Optional authentication password
    #[serde(default)]
    pub password: Option<String>,
    /// Disable non-proxied UDP to eliminate WebRTC STUN leaks (default: true)
    #[serde(default = "default_true", alias = "disableUdp")]
    pub disable_udp: bool,
}

/// Standard browser engine category (Chromium, Firefox, or extensible Custom)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BrowserType {
    /// Chromium-based engines (Chromium, Chrome, Antidetect, Edge, Brave) using Chrome DevTools Protocol (CDP)
    #[serde(alias = "chrome", alias = "stealth", alias = "edge", alias = "brave")]
    #[default]
    Chromium,
    /// Firefox / Gecko engines using Marionette or WebDriver BiDi
    #[serde(alias = "gecko", alias = "waterfox")]
    Firefox,
    /// Extensible custom browser binary or engine
    #[serde(untagged)]
    Custom(String),
}

impl std::fmt::Display for BrowserType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrowserType::Chromium => write!(f, "chromium"),
            BrowserType::Firefox => write!(f, "firefox"),
            BrowserType::Custom(s) => write!(f, "{}", s),
        }
    }
}

impl BrowserType {
    /// Checks if engine is Chromium-based
    pub fn is_chromium(&self) -> bool {
        matches!(self, BrowserType::Chromium)
    }

    /// Checks if engine is Firefox / Gecko-based
    pub fn is_firefox(&self) -> bool {
        matches!(self, BrowserType::Firefox)
    }

    /// Returns the standard headless argument for this engine
    pub fn default_headless_flag(&self) -> &'static str {
        match self {
            BrowserType::Firefox => "--headless",
            _ => "--headless=new",
        }
    }
}

/// Comprehensive browser execution and sandbox environment configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BrowserConfig {
    /// Standard browser engine category (Chromium, Firefox, or Custom)
    #[serde(default, alias = "type", alias = "browserType")]
    pub browser_type: Option<BrowserType>,
    /// Isolated browser profile identifier (sandbox directory name under ~/.specter/browser/profiles/)
    #[serde(default, alias = "profile", alias = "profileId", alias = "browserId")]
    pub browser_id: Option<String>,
    /// Specific Antidetect Chromium version ("148", "lts", "148.0.7778.215")
    #[serde(default)]
    pub version: Option<String>,
    /// Launch in headless mode (default: true)
    #[serde(default)]
    pub headless: Option<bool>,
    /// Custom user data directory (overrides default ~/.specter/browser/profiles/<id>)
    #[serde(default, alias = "userDataDir")]
    pub user_data_dir: Option<String>,
    /// Chrome DevTools Protocol debugging port
    #[serde(default, alias = "debuggingPort", alias = "port")]
    pub debugging_port: Option<u16>,
    /// Deterministic stealth hardware fingerprint settings
    #[serde(default)]
    pub fingerprint: Option<FingerprintConfig>,
    /// Network proxy configuration (e.g. SOCKS5 1080)
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    /// Extension directory paths to unpack and load
    #[serde(default, alias = "extensionPaths", alias = "loadExtensions")]
    pub extensions: Vec<String>,
    /// Custom raw Chromium command-line arguments
    #[serde(default, alias = "args", alias = "customArgs")]
    pub custom_args: Vec<String>,
    /// Window dimensions (width, height)
    #[serde(default, alias = "windowSize")]
    pub window_size: Option<WindowSizeConfig>,
    /// Gracefully close browser process tree upon task completion
    #[serde(default = "default_true", alias = "closeBrowserOnFinish")]
    pub close_browser_on_finish: bool,
}

/// Convenience enum allowing `browser` to be specified as either a structured `BrowserConfig`
/// or a standard `BrowserType` (e.g. "chromium", "firefox", "stealth")
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(clippy::large_enum_variant)]
pub enum BrowserConfigOrName {
    Config(BrowserConfig),
    Type(BrowserType),
}

impl Default for BrowserConfigOrName {
    fn default() -> Self {
        BrowserConfigOrName::Config(BrowserConfig::default())
    }
}

/// Typed specification for a workflow to be executed by the Automa driver
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkflowConfig {
    /// Path or ID of the workflow file to execute (e.g. "workflows/scrape.json")
    #[serde(default, alias = "file", alias = "workflowPath", alias = "id")]
    pub path: String,
    /// Dynamic runtime variables passed into workflow interpolation ({{ variables.key }})
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "params", alias = "vars")]
    pub variables: Option<HashMap<String, serde_json::Value>>,
    /// Optional inline workflow JSON graph / DAG specification
    #[serde(default, skip_serializing_if = "Option::is_none", alias = "workflowData", alias = "flow", alias = "data")]
    pub data: Option<serde_json::Value>,
}

impl WorkflowConfig {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            variables: None,
            data: None,
        }
    }

    pub fn with_variables(mut self, vars: HashMap<String, serde_json::Value>) -> Self {
        self.variables = Some(vars);
        self
    }
}

/// Allows either a structured `WorkflowConfig` object (with `path: string` and `variables`)
/// or a raw path string for backward-compatibility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WorkflowTarget {
    Config(WorkflowConfig),
    Path(String),
}

impl WorkflowTarget {
    pub fn path(&self) -> &str {
        match self {
            WorkflowTarget::Config(c) => &c.path,
            WorkflowTarget::Path(p) => p.as_str(),
        }
    }
}

impl std::ops::Deref for WorkflowTarget {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.path()
    }
}

impl From<&str> for WorkflowTarget {
    fn from(s: &str) -> Self {
        WorkflowTarget::Path(s.to_string())
    }
}

impl From<String> for WorkflowTarget {
    fn from(s: String) -> Self {
        WorkflowTarget::Path(s)
    }
}

impl From<WorkflowConfig> for WorkflowTarget {
    fn from(cfg: WorkflowConfig) -> Self {
        WorkflowTarget::Config(cfg)
    }
}

/// Typed specification for Automa workflow execution jobs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutomaJobPayload {
    /// Workflow configuration (object with `path: string` and `variables`, or path string)
    #[serde(default, alias = "workflowPath", alias = "workflowId", alias = "file")]
    pub workflow: Option<WorkflowTarget>,
    /// Inline workflow graph / DAG specification (nodes, edges, drawflow)
    #[serde(default, alias = "workflowData", alias = "flow")]
    pub workflow_data: Option<serde_json::Value>,
    /// Dynamic runtime variables passed into workflow interpolation ({{ variables.key }})
    #[serde(default, alias = "params", alias = "vars")]
    pub variables: Option<HashMap<String, serde_json::Value>>,
    /// Dedicated browser execution environment configuration (struct or standard browser type)
    #[serde(default)]
    pub browser: Option<BrowserConfigOrName>,
    // --- Flat convenience aliases for browser configuration ---
    #[serde(default, alias = "browserId")]
    pub browser_id: Option<String>,
    #[serde(default, alias = "browserType", alias = "engine")]
    pub browser_type: Option<BrowserType>,
    #[serde(default)]
    pub headless: Option<bool>,
    #[serde(default, alias = "closeBrowserOnFinish")]
    pub close_browser_on_finish: Option<bool>,
    #[serde(default)]
    pub proxy: Option<ProxyConfig>,
    #[serde(default)]
    pub fingerprint: Option<FingerprintConfig>,
    // --- Automa execution settings ---
    /// Enable debug mode for verbose step-by-step logs
    #[serde(default, alias = "debugMode")]
    pub debug: Option<bool>,
    /// Target execution CLI binary name ("tuquet", "automa", "automa-core")
    #[serde(default)]
    pub target: Option<String>,
    /// Timeout in milliseconds (if overriding job-level timeout)
    #[serde(default, alias = "timeoutMs")]
    pub timeout_ms: Option<u64>,
}

impl AutomaJobPayload {
    /// Returns the workflow path if configured
    pub fn workflow_path(&self) -> Option<&str> {
        match &self.workflow {
            Some(WorkflowTarget::Config(cfg)) => {
                if cfg.path.is_empty() {
                    None
                } else {
                    Some(cfg.path.as_str())
                }
            }
            Some(WorkflowTarget::Path(p)) => {
                if p.is_empty() {
                    None
                } else {
                    Some(p.as_str())
                }
            }
            None => None,
        }
    }

    /// Resolves the workflow target into a structured WorkflowConfig
    pub fn resolved_workflow_config(&self) -> WorkflowConfig {
        match &self.workflow {
            Some(WorkflowTarget::Config(cfg)) => {
                let mut c = cfg.clone();
                if c.variables.is_none() && self.variables.is_some() {
                    c.variables = self.variables.clone();
                }
                if c.data.is_none() && self.workflow_data.is_some() {
                    c.data = self.workflow_data.clone();
                }
                c
            }
            Some(WorkflowTarget::Path(p)) => WorkflowConfig {
                path: p.clone(),
                variables: self.variables.clone(),
                data: self.workflow_data.clone(),
            },
            None => WorkflowConfig {
                path: String::new(),
                variables: self.variables.clone(),
                data: self.workflow_data.clone(),
            },
        }
    }

    /// Resolves effective browser configuration by combining the `browser` block with flat shortcut fields
    pub fn resolved_browser_config(&self) -> BrowserConfig {
        let mut cfg = match &self.browser {
            Some(BrowserConfigOrName::Config(c)) => c.clone(),
            Some(BrowserConfigOrName::Type(t)) => BrowserConfig {
                browser_type: Some(t.clone()),
                ..Default::default()
            },
            None => BrowserConfig::default(),
        };
        if let Some(ref bid) = self.browser_id {
            cfg.browser_id = Some(bid.clone());
        }
        if let Some(ref btype) = self.browser_type {
            cfg.browser_type = Some(btype.clone());
        }
        if let Some(h) = self.headless {
            cfg.headless = Some(h);
        }
        if let Some(c) = self.close_browser_on_finish {
            cfg.close_browser_on_finish = c;
        }
        if let Some(ref p) = self.proxy {
            cfg.proxy = Some(p.clone());
        }
        if let Some(ref f) = self.fingerprint {
            cfg.fingerprint = Some(f.clone());
        }
        cfg
    }

    /// Converts this payload into CLI arguments for `tuquet automa run`
    pub fn build_cli_args(&self) -> Vec<String> {
        let mut args = vec!["run".to_string()];
        let wf_cfg = self.resolved_workflow_config();

        if !wf_cfg.path.is_empty() {
            args.push("--workflow".to_string());
            args.push(wf_cfg.path.clone());
        } else if let Some(ref data) = wf_cfg.data {
            args.push("--workflow-json".to_string());
            args.push(serde_json::to_string(data).unwrap_or_default());
        }

        let browser_cfg = self.resolved_browser_config();
        let headless = browser_cfg.headless.unwrap_or(true);
        if headless {
            if let Some(ref btype) = browser_cfg.browser_type {
                args.push(btype.default_headless_flag().to_string());
            } else {
                args.push("--headless".to_string());
            }
        }

        if let Some(ref btype) = browser_cfg.browser_type {
            args.push("--browser".to_string());
            args.push(btype.to_string());
        }

        if let Some(ref bid) = browser_cfg.browser_id {
            args.push("--browser-id".to_string());
            args.push(bid.clone());
        }

        if let Some(ref vars) = wf_cfg.variables {
            for (k, v) in vars {
                let val_str = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                args.push("-p".to_string());
                args.push(format!("{}={}", k, val_str));
            }
        }

        args
    }
}

/// Target action for standalone browser operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BrowserAction {
    /// Launch a persistent or disposable browser profile
    Launch,
    /// Inspect status of installed browser runtimes and active version
    #[default]
    Status,
    /// Delete browser cache and temporary sandboxes to reclaim disk space
    Clean,
    /// Output the absolute executable path of the browser runtime
    Path,
    /// Probe browser capabilities and release manifest
    Probe,
    /// List locally installed runtimes
    List,
}

impl std::fmt::Display for BrowserAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrowserAction::Launch => write!(f, "launch"),
            BrowserAction::Status => write!(f, "status"),
            BrowserAction::Clean => write!(f, "clean"),
            BrowserAction::Path => write!(f, "path"),
            BrowserAction::Probe => write!(f, "probe"),
            BrowserAction::List => write!(f, "list"),
        }
    }
}

/// Typed specification for standalone browser jobs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowserJobPayload {
    /// Target browser operation (default: status)
    #[serde(default)]
    pub action: BrowserAction,
    /// Browser environment and antidetect configuration
    #[serde(default)]
    pub browser: BrowserConfig,
    /// Initial URL to navigate upon launch (if action is Launch)
    #[serde(default)]
    pub url: Option<String>,
    /// Target CLI executable ("tuquet", "tuquet-browser", "browser")
    #[serde(default)]
    pub target: Option<String>,
}

/// Step execution log captured during an Automa workflow run
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomaStepLog {
    pub id: String,
    pub name: String,
    #[serde(default, alias = "blockId")]
    pub block_id: Option<String>,
    pub status: String,
    #[serde(default, alias = "durationMs")]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub message: Option<String>,
}

/// Structured outcome payload for completed Automa workflow jobs
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutomaJobOutput {
    pub workflow_id: Option<String>,
    pub status: String,
    pub duration_ms: u64,
    #[serde(default)]
    pub tables: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub variables: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub step_logs: Option<Vec<AutomaStepLog>>,
    #[serde(default)]
    pub artifacts: Option<Vec<String>>,
}

/// Structured outcome payload for completed Browser operations
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BrowserJobOutput {
    pub browser_id: Option<String>,
    pub pid: Option<u32>,
    pub debugging_port: Option<u16>,
    pub ws_endpoint: Option<String>,
    pub user_data_dir: Option<String>,
    pub runtime_version: Option<String>,
    pub executable_path: Option<String>,
    #[serde(default)]
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_driver_type_display_and_serde() {
        assert_eq!(DriverType::Browser.to_string(), "browser");
        assert_eq!(DriverType::Automa.to_string(), "automa");

        let json = serde_json::to_string(&DriverType::Browser).unwrap();
        assert_eq!(json, "\"browser\"");

        let parsed: DriverType = serde_json::from_str("\"browser\"").unwrap();
        assert_eq!(parsed, DriverType::Browser);
    }

    #[test]
    fn test_automa_payload_nested_browser_and_aliases() {
        let json = serde_json::json!({
            "workflowPath": "./fixtures/test_browser_workflow.json",
            "debugMode": true,
            "browser": {
                "browserType": "chromium",
                "browserId": "test_profile",
                "headless": false,
                "proxy": {
                    "server": "socks5://127.0.0.1:1080",
                    "disableUdp": true
                },
                "fingerprint": {
                    "seed": 133742,
                    "platform": "windows",
                    "timezone": "Asia/Ho_Chi_Minh"
                }
            },
            "variables": {
                "keyword": "tuquet_runner"
            }
        });

        let payload: AutomaJobPayload = serde_json::from_value(json).unwrap();
        assert_eq!(payload.workflow.as_deref(), Some("./fixtures/test_browser_workflow.json"));
        assert_eq!(payload.debug, Some(true));

        let browser_cfg = payload.resolved_browser_config();
        assert_eq!(browser_cfg.browser_type, Some(BrowserType::Chromium));
        assert_eq!(browser_cfg.browser_id.as_deref(), Some("test_profile"));
        assert_eq!(browser_cfg.headless, Some(false));

        let proxy = browser_cfg.proxy.unwrap();
        assert_eq!(proxy.server, "socks5://127.0.0.1:1080");
        assert!(proxy.disable_udp);

        let fp = browser_cfg.fingerprint.unwrap();
        assert_eq!(fp.seed, Some(133742));
        assert_eq!(fp.timezone.as_deref(), Some("Asia/Ho_Chi_Minh"));

        let args = payload.build_cli_args();
        assert!(args.contains(&"--workflow".to_string()));
        assert!(args.contains(&"./fixtures/test_browser_workflow.json".to_string()));
        assert!(args.contains(&"--browser".to_string()));
        assert!(args.contains(&"chromium".to_string()));
        assert!(args.contains(&"--browser-id".to_string()));
        assert!(args.contains(&"test_profile".to_string()));
    }

    #[test]
    fn test_automa_payload_flat_shortcuts() {
        let json = serde_json::json!({
            "workflow": "my_flow",
            "headless": true,
            "browserId": "flat_worker",
            "browser": "stealth"
        });

        let payload: AutomaJobPayload = serde_json::from_value(json).unwrap();
        let browser_cfg = payload.resolved_browser_config();
        assert_eq!(browser_cfg.browser_id.as_deref(), Some("flat_worker"));
        assert_eq!(browser_cfg.browser_type, Some(BrowserType::Chromium));
        assert_eq!(browser_cfg.headless, Some(true));

        let args = payload.build_cli_args();
        assert!(args.contains(&"--browser".to_string()));
        assert!(args.contains(&"chromium".to_string()));
    }

    #[test]
    fn test_automa_payload_firefox() {
        let json = serde_json::json!({
            "workflow": "firefox_flow.json",
            "browser": "firefox",
            "headless": true
        });

        let payload: AutomaJobPayload = serde_json::from_value(json).unwrap();
        let browser = payload.resolved_browser_config();
        assert_eq!(browser.browser_type, Some(BrowserType::Firefox));
        assert!(browser.browser_type.as_ref().unwrap().is_firefox());
        assert!(!browser.browser_type.as_ref().unwrap().is_chromium());

        let args = payload.build_cli_args();
        assert!(args.contains(&"--browser".to_string()));
        assert!(args.contains(&"firefox".to_string()));
        assert!(args.contains(&"--headless".to_string()));
    }

    #[test]
    fn test_browser_type_standard_and_custom() {
        let c: BrowserType = serde_json::from_str("\"chromium\"").unwrap();
        assert_eq!(c, BrowserType::Chromium);
        assert!(c.is_chromium());
        assert_eq!(c.to_string(), "chromium");

        let f: BrowserType = serde_json::from_str("\"firefox\"").unwrap();
        assert_eq!(f, BrowserType::Firefox);
        assert!(f.is_firefox());
        assert_eq!(f.to_string(), "firefox");

        let custom: BrowserType = serde_json::from_str("\"webkit\"").unwrap();
        assert_eq!(custom, BrowserType::Custom("webkit".to_string()));
        assert_eq!(custom.to_string(), "webkit");
    }

    #[test]
    fn test_browser_payload_action() {
        let json = serde_json::json!({
            "action": "launch",
            "url": "https://iphey.com",
            "browser": {
                "browserId": "stealth_sandbox",
                "headless": false,
                "version": "148"
            }
        });

        let payload: BrowserJobPayload = serde_json::from_value(json).unwrap();
        assert_eq!(payload.action, BrowserAction::Launch);
        assert_eq!(payload.url.as_deref(), Some("https://iphey.com"));
        assert_eq!(payload.browser.browser_id.as_deref(), Some("stealth_sandbox"));
        assert_eq!(payload.browser.version.as_deref(), Some("148"));
    }

    #[test]
    fn test_job_as_automa_and_browser_payload() {
        let job = Job {
            id: "job-101".to_string(),
            driver: DriverType::Automa,
            payload: serde_json::json!({
                "workflow": "test.json",
                "headless": true
            }),
            cwd: None,
            env: None,
            timeout_ms: 10_000,
            created_at: None,
        };

        let automa_payload = job.as_automa_payload().unwrap();
        assert_eq!(automa_payload.workflow.as_deref(), Some("test.json"));
    }

    #[test]
    fn test_fixture_automa_with_browser() {
        let fixture_str = include_str!("../../fixtures/job_automa_with_browser.json");
        let job: Job = serde_json::from_str(fixture_str).unwrap();
        assert_eq!(job.driver, DriverType::Automa);
        assert_eq!(job.id, "job-automa-stealth-001");

        let automa = job.as_automa_payload().unwrap();
        assert_eq!(automa.workflow.as_deref(), Some("./fixtures/test_browser_workflow.json"));
        assert_eq!(automa.workflow_path(), Some("./fixtures/test_browser_workflow.json"));
        let wf_cfg = automa.resolved_workflow_config();
        assert_eq!(wf_cfg.path, "./fixtures/test_browser_workflow.json");
        let wf_vars = wf_cfg.variables.unwrap();
        assert_eq!(wf_vars.get("target_domain").and_then(|v| v.as_str()), Some("example.com"));
        assert_eq!(wf_vars.get("max_retries").and_then(|v| v.as_i64()), Some(3));
        assert_eq!(automa.debug, Some(true));

        let browser = automa.resolved_browser_config();
        assert_eq!(browser.browser_type, Some(BrowserType::Chromium));
        assert_eq!(browser.version.as_deref(), Some("148"));
        assert_eq!(browser.browser_id.as_deref(), Some("sandbox_worker_01"));
        assert_eq!(browser.headless, Some(true));

        let proxy = browser.proxy.unwrap();
        assert_eq!(proxy.server, "socks5://127.0.0.1:1080");
        assert!(proxy.disable_udp);

        let fp = browser.fingerprint.unwrap();
        assert_eq!(fp.seed, Some(133742));
        assert_eq!(fp.platform.as_deref(), Some("windows"));
        assert_eq!(fp.brand.as_deref(), Some("Chrome"));
        assert_eq!(fp.brand_version.as_deref(), Some("148.0.7778.215"));
        assert_eq!(fp.hardware_concurrency, Some(8));
        assert_eq!(fp.timezone.as_deref(), Some("Asia/Ho_Chi_Minh"));
        assert_eq!(fp.lang.as_deref(), Some("vi-VN"));

        let window = browser.window_size.unwrap();
        assert_eq!(window.width, 1920);
        assert_eq!(window.height, 1080);
    }

    #[test]
    fn test_automa_payload_workflow_object() {
        let json = serde_json::json!({
            "workflow": {
                "path": "./workflows/data_miner.json",
                "variables": {
                    "depth": 3,
                    "target": "https://api.tuquet.dev"
                }
            },
            "browser": "chromium"
        });

        let payload: AutomaJobPayload = serde_json::from_value(json).unwrap();
        assert_eq!(payload.workflow_path(), Some("./workflows/data_miner.json"));

        let wf_cfg = payload.resolved_workflow_config();
        assert_eq!(wf_cfg.path, "./workflows/data_miner.json");
        let vars = wf_cfg.variables.unwrap();
        assert_eq!(vars.get("depth").and_then(|v| v.as_i64()), Some(3));
        assert_eq!(vars.get("target").and_then(|v| v.as_str()), Some("https://api.tuquet.dev"));

        let args = payload.build_cli_args();
        assert!(args.contains(&"--workflow".to_string()));
        assert!(args.contains(&"./workflows/data_miner.json".to_string()));
        assert!(args.contains(&"-p".to_string()));
        assert!(args.contains(&"depth=3".to_string()));
        assert!(args.contains(&"target=https://api.tuquet.dev".to_string()));
    }

    #[test]
    fn test_fixture_browser_standalone() {
        let fixture_str = include_str!("../../fixtures/job_browser_standalone.json");
        let job: Job = serde_json::from_str(fixture_str).unwrap();
        assert_eq!(job.driver, DriverType::Browser);

        let browser_payload = job.as_browser_payload().unwrap();
        assert_eq!(browser_payload.action, BrowserAction::Status);
    }

    #[test]
    fn test_fixture_automa_with_firefox() {
        let fixture_str = include_str!("../../fixtures/job_automa_with_firefox.json");
        let job: Job = serde_json::from_str(fixture_str).unwrap();
        assert_eq!(job.driver, DriverType::Automa);
        assert_eq!(job.id, "job-automa-firefox-001");

        let automa = job.as_automa_payload().unwrap();
        assert_eq!(automa.workflow.as_deref(), Some("./fixtures/test_browser_workflow.json"));
        let wf_cfg = automa.resolved_workflow_config();
        assert_eq!(wf_cfg.path, "./fixtures/test_browser_workflow.json");
        let vars = wf_cfg.variables.unwrap();
        assert_eq!(vars.get("browser_engine").and_then(|v| v.as_str()), Some("gecko"));

        let browser = automa.resolved_browser_config();
        assert_eq!(browser.browser_type, Some(BrowserType::Firefox));
        assert!(browser.browser_type.as_ref().unwrap().is_firefox());
        assert_eq!(browser.browser_id.as_deref(), Some("firefox_sandbox_01"));
        assert_eq!(browser.headless, Some(true));

        let proxy = browser.proxy.unwrap();
        assert_eq!(proxy.server, "socks5://127.0.0.1:1080");

        let window = browser.window_size.unwrap();
        assert_eq!(window.width, 1440);
        assert_eq!(window.height, 900);

        let args = automa.build_cli_args();
        assert!(args.contains(&"--browser".to_string()));
        assert!(args.contains(&"firefox".to_string()));
        assert!(args.contains(&"--headless".to_string()));
    }
}


