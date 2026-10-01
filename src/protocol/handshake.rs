use serde::{Deserialize, Serialize};

/// Authentication health status reported by the agent during probe
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAuthStatus {
    pub valid: bool,
    pub email: String,
    pub provider: String,
}

/// Capability manifest returned by an AI agent during handshake discovery
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentManifest {
    pub protocol: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub engine: String,
    pub auth: AgentAuthStatus,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub models: Vec<String>,
    #[serde(default)]
    pub proxy_ready: bool,
}

impl AgentManifest {
    /// Validates whether the agent matches tuquet.agent.v1 protocol specifications
    pub fn is_compatible(&self) -> bool {
        self.protocol.starts_with("tuquet.agent.")
    }
}

/// Capability manifest returned by Tuquet CLI browser automation plugin
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutomaManifest {
    pub protocol: String,
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub engine: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub plugin_type: String,
}

impl AutomaManifest {
    /// Validates whether the plugin matches tuquet.automa.v1 protocol specifications
    pub fn is_compatible(&self) -> bool {
        self.protocol.starts_with("tuquet.automa.")
    }
}
