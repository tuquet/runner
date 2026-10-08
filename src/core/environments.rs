use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub use crate::constants::{DEFAULT_DEV_ANON_KEY, DEFAULT_DEV_SUPABASE_URL as DEFAULT_DEV_URL, DEFAULT_LOCAL_ANON_KEY};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentConfig {
    pub name: String,
    pub label: String,
    pub url: String,
    pub api_key: Option<String>,
}

pub struct EnvironmentRegistry;

impl EnvironmentRegistry {
    const ENV_FILENAME: &'static str = crate::constants::FILE_ENVIRONMENTS_JSON;

    /// Get default built-in environments
    pub fn default_environments() -> HashMap<String, EnvironmentConfig> {
        let mut map = HashMap::new();
        map.insert(
            "local".to_string(),
            EnvironmentConfig {
                name: "local".to_string(),
                label: format!("Local Docker Supabase (127.0.0.1:{})", crate::constants::DEFAULT_LOCAL_DOCKER_SUPABASE_PORT),
                url: crate::constants::DEFAULT_LOCAL_SUPABASE_URL.to_string(),
                api_key: Some(DEFAULT_LOCAL_ANON_KEY.to_string()),
            },
        );
        map.insert(
            "dev".to_string(),
            EnvironmentConfig {
                name: "dev".to_string(),
                label: "Supabase Cloud Dev (dswhacsoaxgpfnkaxnhz)".to_string(),
                url: DEFAULT_DEV_URL.to_string(),
                api_key: Some(DEFAULT_DEV_ANON_KEY.to_string()),
            },
        );
        map.insert(
            "prod".to_string(),
            EnvironmentConfig {
                name: "prod".to_string(),
                label: "Supabase Cloud Production (Unset - configure with 'runner env set prod')".to_string(),
                url: "".to_string(),
                api_key: None,
            },
        );
        map
    }

    /// Load all environments merging built-in and user-custom overrides
    pub fn load_all(config_dir: &Path) -> HashMap<String, EnvironmentConfig> {
        let mut envs = Self::default_environments();
        let file_path = config_dir.join(Self::ENV_FILENAME);
        if file_path.exists() {
            if let Ok(content) = fs::read_to_string(&file_path) {
                if let Ok(custom) = serde_json::from_str::<HashMap<String, EnvironmentConfig>>(&content) {
                    for (k, v) in custom {
                        envs.insert(k, v);
                    }
                }
            }
        }
        envs
    }

    /// Get specific environment by name (case-insensitive)
    pub fn get(name: &str, config_dir: &Path) -> Option<EnvironmentConfig> {
        let envs = Self::load_all(config_dir);
        envs.get(&name.to_lowercase()).cloned()
    }

    /// Set or update an environment configuration
    pub fn set(env: EnvironmentConfig, config_dir: &Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        fs::create_dir_all(config_dir)?;
        let mut envs = Self::load_all(config_dir);
        let name = env.name.to_lowercase();
        envs.insert(name, env);

        let file_path = config_dir.join(Self::ENV_FILENAME);
        let content = serde_json::to_string_pretty(&envs)?;
        fs::write(file_path, content)?;
        Ok(())
    }

    /// List all known environments sorted by name
    pub fn list(config_dir: &Path) -> Vec<EnvironmentConfig> {
        let envs = Self::load_all(config_dir);
        let mut list: Vec<EnvironmentConfig> = envs.into_values().collect();
        list.sort_by(|a, b| {
            let order_key = |name: &str| match name {
                "local" => 0,
                "dev" => 1,
                "prod" => 2,
                _ => 3,
            };
            order_key(&a.name).cmp(&order_key(&b.name))
        });
        list
    }
}
