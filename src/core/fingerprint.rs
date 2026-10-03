use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSpecs {
    pub fingerprint: String,
    pub hostname: String,
    pub os_info: String,
    pub cpu_cores: usize,
    pub ram_mb: u64,
    pub capabilities: Vec<String>,
}

pub struct FingerprintEngine;

impl FingerprintEngine {
    /// Collect the complete hardware fingerprint, specs, and capabilities of this machine
    pub fn collect(config_dir: &Path) -> DeviceSpecs {
        let fingerprint = Self::get_or_create_machine_id(config_dir);
        let hostname = Self::get_hostname();
        let os_info = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
        let cpu_cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let ram_mb = Self::get_ram_mb();
        let capabilities = Self::probe_capabilities();

        DeviceSpecs {
            fingerprint,
            hostname,
            os_info,
            cpu_cores,
            ram_mb,
            capabilities,
        }
    }

    /// Read persistent machine_id from config/.machine_id, or generate and persist a new one
    pub fn get_or_create_machine_id(config_dir: &Path) -> String {
        let id_file = config_dir.join(".machine_id");
        if let Ok(content) = fs::read_to_string(&id_file) {
            let trimmed = content.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }

        // Try querying WMI BIOS UUID on Windows
        let raw_id = Self::query_wmi_uuid().unwrap_or_else(|| {
            format!("HWID-{}-{}", std::env::consts::OS, uuid::Uuid::new_v4().simple())
        });

        // Ensure config directory exists
        let _ = fs::create_dir_all(config_dir);
        let _ = fs::write(&id_file, raw_id.trim());

        raw_id.trim().to_string()
    }

    fn get_hostname() -> String {
        std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "node".to_string())
    }

    #[cfg(windows)]
    fn get_ram_mb() -> u64 {
        use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
        unsafe {
            let mut status: MEMORYSTATUSEX = std::mem::zeroed();
            status.dwLength = std::mem::size_of::<MEMORYSTATUSEX>() as u32;
            if GlobalMemoryStatusEx(&mut status) != 0 {
                status.ullTotalPhys / (1024 * 1024)
            } else {
                4096
            }
        }
    }

    #[cfg(not(windows))]
    fn get_ram_mb() -> u64 {
        4096
    }

    #[cfg(windows)]
    fn query_wmi_uuid() -> Option<String> {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", "Get-CimInstance -ClassName Win32_ComputerSystemProduct | Select-Object -ExpandProperty UUID"])
            .output()
            .ok()?;

        if output.status.success() {
            let val = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !val.is_empty() && val.contains('-') {
                return Some(val);
            }
        }
        None
    }

    #[cfg(not(windows))]
    fn query_wmi_uuid() -> Option<String> {
        None
    }

    /// Probe local worker capabilities (claude-agy, shell)
    pub fn probe_capabilities() -> Vec<String> {
        let mut caps = Vec::new();

        // 1. Shell capability is always present
        if cfg!(windows) {
            caps.push("shell:pwsh".to_string());
        } else {
            caps.push("shell:bash".to_string());
        }

        // 2. Probe claude-agy capability
        if Self::probe_agent() {
            caps.push("agent:claude-agy".to_string());
        }

        // 3. Probe automa-core browser automation plugin capability
        if Self::probe_automa() {
            caps.push("automa:core".to_string());
        }

        caps
    }

    fn probe_agent() -> bool {
        let cmd_name = if cfg!(windows) { "claude-agy.cmd" } else { "claude-agy" };
        let mut cmd = Command::new(cmd_name);
        cmd.arg("probe");

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.contains("tuquet.agent.v1");
            }
        }

        // Fallback: check plain 'claude-agy' executable
        if let Ok(output) = Command::new("claude-agy").arg("probe").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.contains("tuquet.agent.v1");
            }
        }

        false
    }

    fn probe_automa() -> bool {
        // 1. Primary: check 'tuquet' unified CLI binary
        let tuquet_name = if cfg!(windows) { "tuquet.exe" } else { "tuquet" };
        if let Ok(output) = Command::new(tuquet_name).arg("probe").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.contains("tuquet.automa.v1");
            }
        }

        // 2. Standalone fallback: check 'automa' binary
        let cmd_name = if cfg!(windows) { "automa.exe" } else { "automa" };
        let mut cmd = Command::new(cmd_name);
        cmd.arg("probe");

        if let Ok(output) = cmd.output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.contains("tuquet.automa.v1");
            }
        }

        // 3. Fallback: check 'automa-core' binary
        let core_name = if cfg!(windows) { "automa-core.exe" } else { "automa-core" };
        if let Ok(output) = Command::new(core_name).arg("probe").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.contains("tuquet.automa.v1");
            }
        }

        false
    }
}
