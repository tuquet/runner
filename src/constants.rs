//! Enterprise Constants for Specter Distributed Execution Engine & Runner Daemon

/// Protocol identifiers
pub const PROTOCOL_AUTOMA_V1: &str = "specter.automa.v1";
pub const PROTOCOL_BROWSER_V1: &str = "specter.browser.v1";
pub const PROTOCOL_AGENT_V1: &str = "specter.agent.v1";
pub const PROTOCOL_AUTOMA_LEGACY: &str = "specter.automa.legacy";
pub const PROTOCOL_AGENT_LEGACY: &str = "specter.agent.legacy";

/// Protocol prefixes for handshake negotiation
pub const PROTOCOL_PREFIX_AUTOMA: &str = "specter.automa.";
pub const PROTOCOL_PREFIX_BROWSER: &str = "specter.browser.";
pub const PROTOCOL_PREFIX_AGENT: &str = "specter.agent.";

/// Default network ports
pub const DEFAULT_RUNNER_PORT: u16 = 8765;
pub const DEFAULT_LOCAL_DOCKER_SUPABASE_PORT: u16 = 54321;

/// Default central WebSocket control plane hub
pub const DEFAULT_WEBSOCKET_HUB: &str = "wss://hub.specter.dev/api/v1/runner/ws";

/// Default cloud endpoints
pub const DEFAULT_LOCAL_SUPABASE_URL: &str = "http://127.0.0.1:54321";
pub const DEFAULT_DEV_SUPABASE_URL: &str = "https://dswhacsoaxgpfnkaxnhz.supabase.co";

/// Default API keys for environments
pub const DEFAULT_LOCAL_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZS1kZW1vIiwicm9sZSI6ImFub24iLCJleHAiOjE5ODM4MTI5OTZ9.CRXP1A7WOeoJeXxjNni43kdQwgnWNReilDMblYTn_I0";
pub const DEFAULT_DEV_ANON_KEY: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImRzd2hhY3NvYXhncGZua2F4bmh6Iiwicm9sZSI6ImFub24iLCJpYXQiOjE3OTAyOTEzMzcsImV4cCI6MjEwNTg2NzMzN30.QRdxE3CPCF8CtliOtSUcSFO-jbKi99uM2AKlJgKt6RQ";

/// Canonical SSOT directory and filenames
pub const DEFAULT_SSOT_DIR_NAME: &str = ".specter";
pub const FILE_IDENTITY_JSON: &str = ".identity.json";
pub const FILE_ENVIRONMENTS_JSON: &str = "environments.json";

/// Target binary names
pub const TARGET_SPECTER_BIN: &str = "specter";

/// Default job execution timeouts and intervals
pub const DEFAULT_JOB_TIMEOUT_MS: u64 = 60_000;
pub const DEFAULT_HEARTBEAT_INTERVAL_SECS: u64 = 20;
pub const DEFAULT_MAX_CONCURRENCY: usize = 4;

/// Environment variable keys
pub const ENV_SPECTER_HOME: &str = "SPECTER_HOME";
pub const ENV_SPECTER_ENV: &str = "SPECTER_ENV";
pub const ENV_SPECTER_CLOUD_URL: &str = "SPECTER_CLOUD_URL";
pub const ENV_SPECTER_API_KEY: &str = "SPECTER_API_KEY";
pub const ENV_SPECTER_ENROLLMENT_TOKEN: &str = "SPECTER_ENROLLMENT_TOKEN";
pub const ENV_SPECTER_SERVER: &str = "SPECTER_SERVER";
pub const ENV_SPECTER_TOKEN: &str = "SPECTER_TOKEN";
pub const ENV_SPECTER_RUNNER_ID: &str = "SPECTER_RUNNER_ID";

/// Standard Message Catalog for Runner Daemon
pub const MSG_JOB_CANCELLED: &str = "Received Ctrl+C signal. Safely terminating job execution...";
pub const MSG_ENROLLMENT_SUCCESS: &str = "Workstation successfully enrolled with Specter Cloud!";
pub const MSG_SHUTDOWN_SIGNAL: &str = "Received termination signal (Ctrl+C). Initiating graceful shutdown...";
pub const MSG_SHUTDOWN_CLEAN: &str = "Runner worker daemon exited cleanly.";
pub const MSG_REVOCATION_DETECTED: &str = "Cloud device identity revocation detected! Purging credentials...";
pub const MSG_IDENTITY_PURGED: &str = "Local device identity purged.";
