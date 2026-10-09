use std::path::Path;
use runner::core::enrollment::EnrollmentClient;
use runner::core::environments::{EnvironmentConfig, EnvironmentRegistry};
use runner::core::identity::DeviceIdentity;
use runner::core::Notify;

use crate::cli::EnvCommands;

pub async fn handle_env(
    action: EnvCommands,
    config_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let loaded_identity = DeviceIdentity::load(config_dir).ok().flatten();
    let current_env = loaded_identity.as_ref().map(|i| i.env.to_lowercase()).unwrap_or_default();

    match action {
        EnvCommands::List => {
            println!("================================================================================");
            println!(" Runner Environments");
            println!("================================================================================");
            println!(" {:<8} {:<10} {:<38} DESCRIPTION", "STATUS", "ENV", "URL");
            println!("--------------------------------------------------------------------------------");

            for e in EnvironmentRegistry::list(config_dir) {
                let is_active = e.name.to_lowercase() == current_env;
                let status_str = if is_active {
                    "\x1b[32m* ACTIVE\x1b[0m"
                } else {
                    "  IDLE  "
                };
                let url_display = if e.url.is_empty() {
                    "<not configured>"
                } else {
                    &e.url
                };
                println!(" {:<17} {:<10} {:<38} {}", status_str, e.name.to_uppercase(), url_display, e.label);
            }
            println!("================================================================================");
            if current_env.is_empty() {
                println!("👉 Run 'runner env switch <dev|local>' to connect.");
            } else {
                println!("👉 Active environment: \x1b[32m{}\x1b[0m. Switch anytime with: runner env switch <target>", current_env.to_uppercase());
            }
        }
        EnvCommands::Switch { name } => {
            let target_name = name.to_lowercase();
            let target_env = EnvironmentRegistry::get(&target_name, config_dir);

            let env_cfg = match target_env {
                Some(cfg) if !cfg.url.is_empty() => cfg,
                Some(_) => {
                    Notify::error(format!("Environment '{}' has not been configured with a URL yet.", target_name));
                    eprintln!("Use 'runner env set {} --url <URL> --key <KEY>' first.", target_name);
                    std::process::exit(1);
                }
                None => {
                    Notify::error(format!("Unknown environment: '{}'. Available: dev, local, prod", target_name));
                    std::process::exit(1);
                }
            };

            Notify::header("Runner - Switching Environment");
            if !current_env.is_empty() {
                Notify::key_val("Current Env", current_env.to_uppercase());
            }
            Notify::key_val("Target Env", format!("{} ({})", env_cfg.name.to_uppercase(), env_cfg.url));
            Notify::info("Purging old credentials...");
            let _ = DeviceIdentity::purge(config_dir);

            Notify::info(format!("Enrolling into {}...", env_cfg.name.to_uppercase()));
            let client = EnrollmentClient::new();
            match client.enroll(&env_cfg.url, env_cfg.api_key.as_deref(), None, &env_cfg.name, config_dir).await {
                Ok(identity) => {
                    Notify::success(format!("Successfully switched and enrolled into {}!", env_cfg.name.to_uppercase()));
                    Notify::key_val("Environment", identity.env.to_uppercase());
                    Notify::key_val("Device ID", identity.device_id);
                    Notify::key_val("Cloud URL", identity.cloud_url);
                    Notify::key_val("Identity Path", config_dir.join(runner::constants::FILE_IDENTITY_JSON).display());
                    println!("\n👉 Run 'runner worker' to start processing jobs on this environment.");
                }
                Err(e) => {
                    Notify::error(format!("Enrollment into '{}' failed: {}", env_cfg.name, e));
                    std::process::exit(1);
                }
            }
        }
        EnvCommands::Set { name, url, key, label } => {
            let target_name = name.to_lowercase();
            let lbl = label.unwrap_or_else(|| format!("Custom {} environment", target_name));
            let cfg = EnvironmentConfig {
                name: target_name.clone(),
                label: lbl,
                url: url.clone(),
                api_key: Some(key),
            };

            match EnvironmentRegistry::set(cfg, config_dir) {
                Ok(_) => {
                    Notify::success(format!("Environment '{}' configured successfully!", target_name.to_uppercase()));
                    Notify::key_val("URL", url);
                    println!("Run 'runner env switch {}' to switch to it.", target_name);
                }
                Err(e) => {
                    Notify::error(format!("Failed to save environment: {}", e));
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}
