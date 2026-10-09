use std::path::Path;
use runner::core::enrollment::EnrollmentClient;
use runner::core::environments::EnvironmentRegistry;
use runner::core::fingerprint::FingerprintEngine;
use runner::core::identity::DeviceIdentity;
use runner::core::Notify;

pub async fn handle_enroll(
    env: String,
    url: Option<String>,
    key: Option<String>,
    token: Option<String>,
    config_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let env_config = EnvironmentRegistry::get(&env, config_dir);
    let target_url = url.or_else(|| env_config.as_ref().map(|c| c.url.clone()))
        .unwrap_or_default();
    let target_key = key.or_else(|| env_config.as_ref().and_then(|c| c.api_key.clone()));

    if target_url.is_empty() {
        Notify::error(format!("Environment '{}' has no URL configured.", env));
        if env.to_lowercase() == "prod" {
            eprintln!("Configure production endpoint first: runner env set prod --url <URL> --key <KEY>");
        }
        std::process::exit(1);
    }

    Notify::header("Runner - Device Enrollment");
    let specs = FingerprintEngine::collect(config_dir);
    println!(" Environment:   {} ({})", env.to_uppercase(), env_config.as_ref().map(|c| c.label.as_str()).unwrap_or("Custom"));
    println!(" Fingerprint:   {}", specs.fingerprint);
    println!(" Hostname:      {}", specs.hostname);
    println!(" Specs:         {} cores, {} MB RAM ({})", specs.cpu_cores, specs.ram_mb, specs.os_info);
    println!(" Capabilities:  {:?}", specs.capabilities);
    println!(" Connecting to: {}", target_url);
    Notify::divider();

    let client = EnrollmentClient::new();
    match client.enroll(&target_url, target_key.as_deref(), token, &env, config_dir).await {
        Ok(identity) => {
            Notify::success(runner::constants::MSG_ENROLLMENT_SUCCESS);
            println!(" Environment:   {}", identity.env.to_uppercase());
            println!(" Device ID:     {}", identity.device_id);
            println!(" Tenant ID:     {}", identity.tenant_id);
            println!(" Device Name:   {}", identity.name);
            println!(" Identity Path: {}", config_dir.join(runner::constants::FILE_IDENTITY_JSON).display());
            println!("\n👉 Run 'runner worker' to start processing cloud jobs.");
        }
        Err(e) => {
            Notify::error(format!("Enrollment failed: {}", e));
            std::process::exit(1);
        }
    }

    Ok(())
}

pub fn handle_purge(config_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    match DeviceIdentity::purge(config_dir) {
        Ok(_) => {
            Notify::success(format!("Local device identity purged from {}", config_dir.join(runner::constants::FILE_IDENTITY_JSON).display()));
            println!("Workstation is now in a clean, unenrolled state. Run 'runner env switch <dev|local>' to re-register.");
        }
        Err(e) => {
            Notify::error(format!("Failed to purge identity: {}", e));
            std::process::exit(1);
        }
    }
    Ok(())
}

pub fn handle_info(config_dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let specs = FingerprintEngine::collect(config_dir);
    let identity = DeviceIdentity::load(config_dir).ok().flatten();

    Notify::header("Runner System Diagnostics");
    Notify::key_val("Version", env!("CARGO_PKG_VERSION"));
    Notify::key_val("Hostname", &specs.hostname);
    Notify::key_val("Fingerprint", &specs.fingerprint);
    Notify::key_val("Architecture", &specs.os_info);
    Notify::key_val("Resources", format!("{} CPU Cores | {} MB RAM", specs.cpu_cores, specs.ram_mb));
    Notify::key_val("Capabilities", format!("{:?}", specs.capabilities));
    Notify::key_val("Supervision", "Kernel Process Sandboxing (Zero-Zombie)");
    Notify::key_val("Config Path", config_dir.display());
    Notify::divider();
    if let Some(id) = identity {
        Notify::key_val("Enrollment", "ENROLLED");
        Notify::key_val("Environment", format!("{} ({})", id.env.to_uppercase(), id.cloud_url));
        Notify::key_val("Device ID", id.device_id);
        Notify::key_val("Tenant ID", id.tenant_id);
        Notify::key_val("Device Name", id.name);
        Notify::key_val("Enrolled At", id.enrolled_at);
    } else {
        Notify::key_val("Enrollment", "NOT ENROLLED (Run 'runner env switch <dev|local>' to connect)");
    }
    Notify::divider();
    Ok(())
}
