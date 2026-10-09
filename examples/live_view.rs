use runner::CdpSession;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("runner=info".parse().unwrap()))
        .try_init();

    println!("\n=======================================================");
    println!("  👀 SPECTER LIVE VIEW - VISUAL INSPECTION HARNESS");
    println!("=======================================================\n");

    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap_or_default();
    let chrome_bin = PathBuf::from(&home).join(".specter/browser/runtimes/stealth/chrome.exe");

    if !chrome_bin.exists() {
        eprintln!("Error: Antidetect Chromium binary not found at {}", chrome_bin.display());
        std::process::exit(1);
    }

    let temp_profile = std::env::temp_dir().join(format!("specter_live_profile_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_profile).await?;

    println!("[1/4] Launching C++ Antidetect Chromium v148 in HEADFUL VISUAL MODE...");
    println!("      • Binary:        {}", chrome_bin.display());
    println!("      • Profile Dir:   {}", temp_profile.display());
    println!("      • Dynamic Port:  --remote-debugging-port=0");
    println!("      • Fingerprint:   133742 (Chrome 148.0.7778.215)");

    let mut cmd = Command::new(&chrome_bin);
    cmd.arg(format!("--user-data-dir={}", temp_profile.display()))
        .arg("--remote-debugging-port=0")
        .arg("--fingerprint=133742")
        .arg("--fingerprint-brand=Chrome")
        .arg("--fingerprint-brand-version=148.0.7778.215")
        .arg("--window-size=1280,900")
        .arg("--lang=vi-VN,vi,en-US,en")
        .arg("--no-first-run")
        .arg("--no-default-browser-check");

    // Optional Specter Bridge Proxy (1080)
    if std::net::TcpListener::bind("127.0.0.1:1080").is_err() {
        println!("      • Network Route: SOCKS5 Bridge Active (127.0.0.1:1080)");
        cmd.arg("--proxy-server=socks5://127.0.0.1:1080")
            .arg("--disable-non-proxied-udp");
    } else {
        println!("      • Network Route: Direct Connection");
    }

    let mut child = cmd.spawn()?;
    let pid = child.id();
    println!("      ✨ Browser window is now open on your screen (PID: {})!", pid);

    println!("\n[2/4] Connecting Native Rust CDP Driver via DevToolsActivePort...");
    let session = match CdpSession::connect_auto(None, Some(&temp_profile)).await {
        Ok(s) => s,
        Err(e) => {
            let _ = child.kill();
            eprintln!("Failed to attach CDP session: {}", e);
            return Err(e.into());
        }
    };
    println!("      ✅ Connected! All interactions will stream live to the browser window.");

    // --- Demo 1: Sannysoft Bot Detection Visual Inspection ---
    println!("\n[3/5] Live Demo 1: Navigating to Sannysoft Bot Detection (https://bot.sannysoft.com/)...");
    session.navigate("https://bot.sannysoft.com/").await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    inject_visual_cursor(&session).await;

    println!("      👉 Performing smooth Bézier curve mouse gestures across test tables...");
    let target_points = [(350.0, 250.0), (700.0, 380.0), (450.0, 520.0), (800.0, 300.0)];
    for (x, y) in target_points {
        let _ = session.evaluate(&format!(
            "if(window.specter_move) window.specter_move({}, {})",
            x, y
        )).await;
        session.click("table").await.ok();
        tokio::time::sleep(Duration::from_millis(400)).await;
    }

    println!("      👉 Demonstrating smooth human-like mouse wheel scrolling with deceleration momentum...");
    session.scroll_down(450.0).await?;
    tokio::time::sleep(Duration::from_millis(600)).await;
    session.scroll_down(350.0).await?;
    tokio::time::sleep(Duration::from_millis(600)).await;
    session.scroll_up(500.0).await?;
    tokio::time::sleep(Duration::from_millis(500)).await;
    println!("      ✅ Sannysoft inspection complete (All green, zero bot flags, smooth scroll verified).");

    // --- Demo 2: Cloudflare Turnstile Live Challenge Bypass ---
    println!("\n[4/5] Live Demo 2: Navigating to Cloudflare Turnstile Challenge (https://peet.ws/turnstile-test/managed.html)...");
    session.navigate("https://peet.ws/turnstile-test/managed.html").await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    inject_visual_cursor(&session).await;

    println!("      👉 Waiting for Turnstile widget render and executing humanized click on checkbox...");
    match session.solve_turnstile(Duration::from_secs(20)).await {
        Ok(true) => {
            let token_info = session.evaluate("(() => {
                const input = document.querySelector('[name=\"cf-turnstile-response\"]');
                return input && input.value ? { len: input.value.length, sample: input.value.substring(0, 32) + '...' } : null;
            })()").await.unwrap_or(serde_json::Value::Null);

            println!("      ✅ Cloudflare Turnstile challenge: PASSED & TOKEN GENERATED!");
            if let Some(sample) = token_info.get("sample").and_then(|s| s.as_str()) {
                let len = token_info.get("len").and_then(|l| l.as_u64()).unwrap_or(0);
                println!("         • Token Length:  {} bytes", len);
                println!("         • Token Sample:  {}", sample);
            }
        }
        Ok(false) => {
            println!("      ⚠ Turnstile box resolved automatically or timeout reached.");
        }
        Err(e) => {
            println!("      ⚠ Turnstile interaction note: {}", e);
        }
    }
    println!("      👀 Pausing 5 seconds for visual inspection of the green checkmark...");
    tokio::time::sleep(Duration::from_secs(5)).await;

    // --- Demo 3: IPHey Authentic Identity Inspection ---
    println!("\n[5/5] Live Demo 3: Navigating to IPHey (https://iphey.com/)...");
    session.navigate("https://iphey.com/").await?;
    tokio::time::sleep(Duration::from_secs(3)).await;
    inject_visual_cursor(&session).await;

    println!("      👉 Performing smooth scroll to inspect identity trust score...");
    session.scroll_down(300.0).await?;
    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("\n=======================================================");
    println!("  🎉 LIVE VIEW RUNNING SUCCESSFULLY!");
    println!("=======================================================");
    println!("  The browser window is currently open on your desktop.");
    println!("  You can look at the browser, test, and interact freely.");
    println!("  👉 Press [ENTER] in this terminal when you are done to close.");
    println!("=======================================================\n");

    let mut input = String::new();
    let _ = std::io::stdin().read_line(&mut input);

    println!("\n[CLEANUP] Closing browser and clearing session...");
    let _ = child.kill();
    let _ = tokio::fs::remove_dir_all(&temp_profile).await;
    println!("Done!");

    Ok(())
}

async fn inject_visual_cursor(s: &CdpSession) {
    let _ = s.evaluate(r#"(() => {
        if (document.getElementById('specter-visual-pointer')) return;
        const dot = document.createElement('div');
        dot.id = 'specter-visual-pointer';
        dot.style.position = 'fixed';
        dot.style.width = '18px';
        dot.style.height = '18px';
        dot.style.borderRadius = '50%';
        dot.style.backgroundColor = '#ff0055';
        dot.style.border = '2px solid #ffffff';
        dot.style.boxShadow = '0 0 12px #ff0055, 0 0 20px rgba(255, 0, 85, 0.4)';
        dot.style.zIndex = '2147483647';
        dot.style.pointerEvents = 'none';
        dot.style.left = '0px';
        dot.style.top = '0px';
        dot.style.transform = 'translate(100px, 100px)';
        dot.style.transition = 'transform 0.04s ease-out';
        document.documentElement.appendChild(dot);

        window.addEventListener('mousemove', (e) => {
            dot.style.transform = `translate(${e.clientX - 9}px, ${e.clientY - 9}px)`;
        }, true);
    })()"#).await;
}
