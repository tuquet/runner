use crate::drivers::{ExecutionContext, ExecutionDriver};
use crate::protocol::schema::{Job, JobId, JobResult, LogChannel};
use async_trait::async_trait;
use reqwest::Client;
use std::time::{Duration, Instant};

pub struct HttpDriver {
    client: Client,
}

impl HttpDriver {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
        }
    }
}

#[async_trait]
impl ExecutionDriver for HttpDriver {
    fn name(&self) -> &'static str {
        "http"
    }

    fn can_handle(&self, job: &Job) -> bool {
        matches!(job.driver, crate::protocol::schema::DriverType::Http)
    }

    async fn execute(&self, job: Job, ctx: ExecutionContext) -> Result<JobResult, String> {
        let start_time = Instant::now();

        let url = job
            .payload
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "Http driver requires 'url' string in payload".to_string())?;

        let method = job
            .payload
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("GET")
            .to_uppercase();

        ctx.emit_log(LogChannel::System, &format!("HTTP {} {}", method, url));

        let mut req = match method.as_str() {
            "POST" => self.client.post(url),
            "PUT" => self.client.put(url),
            "DELETE" => self.client.delete(url),
            _ => self.client.get(url),
        };

        if let Some(headers) = job.payload.get("headers").and_then(|v| v.as_object()) {
            for (k, v) in headers {
                if let Some(val_str) = v.as_str() {
                    req = req.header(k, val_str);
                }
            }
        }

        if let Some(body) = job.payload.get("body") {
            req = req.json(body);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        let status = resp.status();
        let status_code = status.as_u16() as i32;
        ctx.emit_log(LogChannel::System, &format!("HTTP Response Status: {}", status));

        let text = resp
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {}", e))?;

        ctx.emit_log(LogChannel::Stdout, &text);
        let elapsed = start_time.elapsed().as_millis() as u64;

        if status.is_success() {
            let json_output: Option<serde_json::Value> = serde_json::from_str(&text).ok();
            Ok(JobResult::success(job.id, elapsed, json_output))
        } else {
            Ok(JobResult::failure(
                job.id,
                Some(status_code),
                elapsed,
                format!("HTTP error: status {}", status_code),
            ))
        }
    }

    async fn cancel(&self, _job_id: &JobId) -> Result<(), String> {
        Ok(())
    }
}
