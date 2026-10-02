<div align="center">
  <img src="./assets/logo.svg" width="76" height="76" alt="Runner Logo" />
  <h1>Runner</h1>
  <p><strong>High-Performance Distributed Process Supervision Engine in Rust with Win32 Job Object Sandboxing</strong></p>

  <p>
    <a href="https://github.com/tuquet/scoop-bucket"><img src="https://img.shields.io/badge/Scoop-tuquet%2Ftqr-brightgreen.svg" alt="Scoop" /></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/License-MIT-blue.svg" alt="License" /></a>
    <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/Rust-Tokio-orange.svg" alt="Rust" /></a>
    <img src="https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg" alt="Platform" />
    <img src="https://img.shields.io/badge/CLI-tqr-orange.svg" alt="CLI" />
  </p>
</div>

---

## 🏗️ System Architecture: Dumb Runner, Smart Cloud

`tqr` decouples the **Control Plane** (Cloud / Supabase `runners` plugin) from the **Distributed Execution Plane** (Workstations / Edge Nodes).

```mermaid
flowchart TD
    subgraph Cloud["Cloud Control Plane"]
        Dashboard["Web Command Center"]
        PluginRunners["Plugin: runners (PostgreSQL 15+)"]
        TableDevices[("runners.devices<br/>- Hardware Fingerprint<br/>- Specs: CPU, RAM, OS<br/>- Capabilities JSON<br/>- Live State: idle, busy")]
        RpcEnroll["RPC: runners.enroll_device(...)"]
        WSGateway["WebSocket Gateway"]
        
        Dashboard <--> PluginRunners
        PluginRunners --- TableDevices
        TableDevices --- RpcEnroll
        TableDevices --- WSGateway
    end

    subgraph Node["Workstation / Edge Node (tqr in Rust)"]
        FP["Hardware Fingerprint Engine<br/>(Win32 WMI BIOS UUID)"]
        Identity["Identity Anchor<br/>config/.identity.json"]
        RAM["RAM Execution Hub<br/>(Dynamic Cloud Config)"]
        Supervisor["ProcessSupervisor<br/>(Win32 Job Objects)"]
        Drivers{"Driver Router"}
        
        FP -->|tqr enroll| RpcEnroll
        RpcEnroll -->|Issue device_token| Identity
        Identity -->|Outbound Connect| WSGateway
        WSGateway -->|Push Jobs & Config| RAM
        RAM --> Supervisor
        Supervisor --> Drivers
        Drivers --> Agent["AgentDriver (claude-agy)"]
        Drivers --> Shell["ShellDriver (PowerShell / Bash)"]
        Drivers --> Http["HttpDriver (Webhooks)"]
    end
```

---

## ✨ Key Features

- **Lightning-Fast (3-Letter CLI `tqr`)**: Optimized developer ergonomics (`tqr info`, `tqr enroll`, `tqr exec`, `tqr worker`).
- **Zero-Touch Cloud Enrollment (`tqr enroll`)**: Automatically probes BIOS UUID (`Win32_ComputerSystemProduct`), hardware specs (CPU cores, RAM size), and local capabilities (`claude-agy`), enrolling the machine into the Cloud Control Plane in sub-second time.
- **Dumb Runner, Smart Cloud Architecture**:
  - No static YAML drift! Workstations only store a minimal identity anchor in `config/.identity.json` (`device_id` + `device_token`).
  - Runtime configuration (concurrency, timeouts, driver toggles) lives in Cloud PostgreSQL (`runners.devices`) and streams dynamically into the runner's RAM.
- **Kernel-Level Zero-Zombie Guarantee**: Uses Win32 **Job Objects** (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`) on Windows to atomically wipe out entire process trees upon cancellation or timeout in under 0.1ms.
- **Pluggable Driver SPI**:
  - `AgentDriver`: Handshakes with `claude-agy` via `tuquet.agent.v1` protocol, validating Antigravity tokens before executing AI prompts.
  - `ShellDriver`: High-throughput PowerShell, Bash, CMD, and Python streaming.
  - `HttpDriver`: Native webhook dispatching and REST automation.
- **Zero-Leak Secret Masking**: SIMD-accelerated regex scanner automatically sanitizes sensitive tokens (`ya29...`, `sk-ant-...`, bearer secrets) from stdout/stderr streams before sending telemetry to the Cloud.

---

## 📦 Installation

### 1. Windows via Scoop (Recommended)

```powershell
# Add Tuquet bucket
scoop bucket add tuquet https://github.com/tuquet/scoop-bucket

# Install tuquet-runner (supports both 'tuquet-runner' and 'tqr' commands)
scoop install tuquet-runner
```

*Note: Scoop automatically persists `config/` (`.identity.json`, `.machine_id`) and `logs/` across version updates.*

### 2. From Source (Cargo)

```bash
git clone https://github.com/tuquet/runner.git
cd runner
cargo build --release
```
The compiled binary will be placed at `target/release/tuquet-runner.exe`.

---

## ⚡ Quick Start

### 1. System Diagnostics & Capability Inspection
Inspect your machine's hardware fingerprint, CPU/RAM, probed capabilities, and Cloud enrollment status:
```powershell
tqr info
```

Example Output:
```text
============================================================
 Runner (tqr) System Diagnostics
============================================================
 Version:      0.1.0
 Hostname:     WORKSTATION-01
 Fingerprint:  4C4C4544-0053-5810-8047-B2C04F484234
 Architecture: windows-x86_64
 Resources:    16 CPU Cores | 16070 MB RAM
 Capabilities: ["shell:pwsh", "agent:claude-agy"]
 Supervision:  Win32 Job Objects (Kernel-Level Zero-Zombie)
 Config Path:  ~/.tuquet/config
------------------------------------------------------------
 Enrollment:   ENROLLED (Cloud Active)
 Device ID:    2a9cb9d8-dac6-4017-9214-69b65a458467
 Tenant ID:    b0000000-0000-0000-0000-000000000001
 Device Name:  WORKSTATION-01
 Cloud URL:    https://api.tuquet.dev
============================================================
```

### 2. Flexible Environment Management & Switching
`tqr` supports out-of-the-box multi-environment management (`dev`, `local`, `prod`):

```powershell
# List available environments and see which is active
tqr env list

# Switch to Cloud Dev (Supabase Cloud project dswhacsoaxgpfnkaxnhz)
tqr env switch dev

# Switch to Local Docker Supabase (127.0.0.1:54321)
tqr env switch local

# Configure or override a custom environment (e.g. prod)
tqr env set prod --url "https://<your-prod-ref>.supabase.co" --key "<PROD_ANON_KEY>"
tqr env switch prod
```

### 3. Zero-Touch Cloud Enrollment
Register your workstation with the Cloud Control Plane for a specific environment:
```powershell
# Enroll directly into Local Docker Supabase (default)
tqr enroll

# Enroll explicitly into Cloud Dev or Custom Environment
tqr enroll --env dev
tqr enroll --env prod

# Override URL and key on the fly
tqr enroll --url "https://custom.supabase.co" --key "<ANON_KEY>"
```

### 4. Standalone Ad-Hoc Execution (CLI Mode)
Execute commands locally with real-time log streaming:
```powershell
# Execute a PowerShell command
tqr exec -d shell -c "Get-Process | Select-Object -First 5"

# Execute an AI agent prompt via claude-agy
tqr exec -d agent -p "Explain Rust ownership in 3 bullet points"
```

### 5. Background Worker Daemon Mode
Connect outbound to the Cloud Command Center to receive jobs:
```powershell
# Uses enrolled identity automatically
tqr worker

# Or specify custom gateway endpoint
tqr worker --server wss://hub.tuquet.dev/api/v1/runner/ws --tags "windows,gpu,ai"
```

---

## 🔧 CLI Command Reference

| Command | Shorthand | Description |
| :--- | :--- | :--- |
| `tqr info` | — | Display hardware fingerprint, specs, drivers, and active environment |
| `tqr env list` | — | List all configured environments and show active environment |
| `tqr env switch <name>` | — | Switch active environment (`dev`, `local`, `prod`) and re-enroll |
| `tqr env set <name>` | `--url, --key` | Configure or override a custom environment endpoint |
| `tqr enroll` | `--env, -u, -k` | Enroll workstation with Cloud Dev, Local Docker, or custom endpoint |
| `tqr purge` | — | Purge local enrollment credentials (`.identity.json`) |
| `tqr exec` | `-d, -c, -p` | Execute ad-hoc prompt or command locally (`shell`, `agent`, `http`) |
| `tqr run` | `<file>` | Execute a local job specification file (`.yaml` or `.json`) |
| `tqr worker` | `-s, -t` | Start worker daemon listening for remote jobs from Cloud Control Plane |

### Environment Variables

| Variable | Description | Default |
| :--- | :--- | :--- |
| `TUQUET_ENV` | Target environment for enrollment (`dev`, `local`, `prod`) | `dev` |
| `TUQUET_CLOUD_URL` | Cloud / Supabase REST API endpoint | `https://dswhacsoaxgpfnkaxnhz.supabase.co` |
| `TUQUET_API_KEY` | Supabase publishable / anon public key | Loaded from environment registry |
| `TUQUET_ENROLLMENT_TOKEN` | Optional workspace registration pairing token | — |
| `TUQUET_SERVER` | WebSocket control plane gateway URL | `wss://hub.tuquet.dev/api/v1/runner/ws` |
| `TUQUET_TOKEN` | Runner authentication device token | Loaded from `config/.identity.json` |

---

## 🌐 Ecosystem

Part of the **Automation & Agent Ecosystem**:

- [Automa](https://github.com/tuquet/automa) — Native Chrome/Edge Desktop UI Automation Browser.
- [Runner](https://github.com/tuquet/runner) — High-Performance Distributed Process Supervision Engine in Rust.
- [Browser](https://github.com/tuquet/browser) — High-Performance Headless Web Scraping & Stealth Automation Core.
- [Cloud](https://github.com/tuquet/cloud) — Enterprise Orchestration & Real-time Task Control Plane.
- [CLI](https://github.com/tuquet/cli) — Developer Ergonomic CLI & Unified Command Center.
- [Lib](https://github.com/tuquet/lib) — Monorepo for Shared Enterprise UI & Utilities (`vue-ui`, `vue-table`, `md-export`, `extension-runner`, `lunar`).
- [Scoop Bucket](https://github.com/tuquet/scoop-bucket) — Official Windows Scoop Distribution Channel.

---

## 📜 License

Licensed under the [MIT License](LICENSE).

---

<div align="center">
  <samp>
    <a href="https://tuquet.github.io">Portfolio</a> •
    <a href="https://tuquet.github.io/cv">CV &amp; Resume</a> •
    <a href="https://tuquet.github.io/automa">Automa Studio</a> •
    <a href="https://tuquet.github.io/lib">Component Lab</a> •
    <a href="https://github.com/tuquet/scoop-bucket">Scoop Bucket</a>
  </samp>
</div>
