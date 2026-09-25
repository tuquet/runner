use crate::core::fingerprint::FingerprintEngine;
use crate::core::identity::DeviceIdentity;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize)]
struct EnrollRequest {
    p_machine_fingerprint: String,
    p_name: String,
    p_os_info: String,
    p_cpu_cores: usize,
    p_ram_mb: u64,
    p_capabilities: Vec<String>,
    p_metadata: serde_json::Value,
    p_enrollment_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EnrollResponse {
    pub device_id: String,
    pub tenant_id: String,
    pub device_token: String,
    pub name: String,
    pub status: String,
    pub config: serde_json::Value,
}

pub struct EnrollmentClient {
    http_client: reqwest::Client,
}

impl Default for EnrollmentClient {
    fn default() -> Self {
        Self::new()
    }
}

impl EnrollmentClient {
    pub fn new() -> Self {
        Self {
            http_client: reqwest::Client::new(),
        }
    }

    /// Enroll this machine with Tuquet Cloud via Supabase RPC runners.enroll_device
    pub async fn enroll(
        &self,
        cloud_url: &str,
        api_key: Option<&str>,
        enrollment_token: Option<String>,
        config_dir: &Path,
    ) -> Result<DeviceIdentity, Box<dyn std::error::Error>> {
        let specs = FingerprintEngine::collect(config_dir);

        let endpoint = format!("{}/rest/v1/rpc/enroll_device", cloud_url.trim_end_matches('/'));

        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(key) = api_key {
            if let Ok(val) = HeaderValue::from_str(key) {
                headers.insert("apikey", val.clone());
            }
            if let Ok(bearer) = HeaderValue::from_str(&format!("Bearer {}", key)) {
                headers.insert("Authorization", bearer);
            }
        }

        let body = EnrollRequest {
            p_machine_fingerprint: specs.fingerprint,
            p_name: specs.hostname,
            p_os_info: specs.os_info,
            p_cpu_cores: specs.cpu_cores,
            p_ram_mb: specs.ram_mb,
            p_capabilities: specs.capabilities,
            p_metadata: serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "client": "tqr"
            }),
            p_enrollment_token: enrollment_token,
        };

        let res = self
            .http_client
            .post(&endpoint)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        if !res.status().is_success() {
            let status = res.status();
            let err_text = res.text().await.unwrap_or_default();
            return Err(format!("Cloud enrollment failed ({status}): {err_text}").into());
        }

        let enroll_res: EnrollResponse = res.json().await?;

        let identity = DeviceIdentity {
            device_id: enroll_res.device_id,
            tenant_id: enroll_res.tenant_id,
            device_token: enroll_res.device_token,
            name: enroll_res.name,
            cloud_url: cloud_url.to_string(),
            enrolled_at: chrono::Utc::now().to_rfc3339(),
        };

        identity.save(config_dir)?;

        Ok(identity)
    }
}
