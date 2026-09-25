use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceIdentity {
    pub device_id: String,
    pub tenant_id: String,
    pub device_token: String,
    pub name: String,
    pub cloud_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub enrolled_at: String,
}

impl DeviceIdentity {
    const IDENTITY_FILENAME: &'static str = ".identity.json";

    /// Check if a valid identity exists locally
    pub fn is_enrolled(config_dir: &Path) -> bool {
        let file_path = config_dir.join(Self::IDENTITY_FILENAME);
        file_path.exists()
    }

    /// Load identity from config/.identity.json
    pub fn load(config_dir: &Path) -> Result<Option<Self>, Box<dyn std::error::Error + Send + Sync>> {
        let file_path = config_dir.join(Self::IDENTITY_FILENAME);
        if !file_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(file_path)?;
        let identity: Self = serde_json::from_str(&content)?;
        Ok(Some(identity))
    }

    /// Save identity to config/.identity.json
    pub fn save(&self, config_dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        fs::create_dir_all(config_dir)?;
        let file_path = config_dir.join(Self::IDENTITY_FILENAME);
        let content = serde_json::to_string_pretty(self)?;
        fs::write(file_path, content)?;
        Ok(())
    }

    /// Purge identity file upon revocation / cloud deletion
    pub fn purge(config_dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let file_path = config_dir.join(Self::IDENTITY_FILENAME);
        if file_path.exists() {
            fs::remove_file(file_path)?;
        }
        Ok(())
    }
}
