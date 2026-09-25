# ⚡ Tuquet Runner (`tqr` / `tuquet-runner`)

> **Ultra-high performance universal distributed execution engine in Rust**. Orchestrates AI Agents (`claude-agy`), native shell scripts, webhooks, and browser automations across distributed nodes with sub-10MB RAM footprint, zero cold-start latency, Win32 kernel-level process supervision, and zero-touch cloud enrollment.

[![Platform](https://img.shields.io/badge/Platform-Windows%20%7C%20Linux%20%7C%20macOS-lightgrey.svg)](#-installation)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Scoop](https://img.shields.io/badge/Scoop-tuquet%2Ftuquet--runner-brightgreen.svg)](#1-windows-via-scoop-recommended)
[![Command: tqr](https://img.shields.io/badge/CLI-tqr-orange.svg)](#-quick-start)

---

## 🏗️ System Architecture: Dumb Runner, Smart Cloud

`tqr` decouples the **Control Plane** (Tuquet Cloud / Supabase `runners` plugin) from the **Distributed Execution Plane** (Workstations / Edge Nodes).

```mermaid
flowchart TD
    subgraph Cloud["Tuquet Cloud (Control Plane)"]
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

- **Lightning-Fast (3-Letter CLI `tqr`)**: Optimized developer ergonomics (`tqr info`, `tqr enroll`, `tqr exec`, `tqr worker`). Alias `tuquet-runner` is preserved for formal CI/CD pipelines.
- **Zero-Touch Cloud Enrollment (`tqr enroll`)**: Automatically probes BIOS UUID (`Win32_ComputerSystemProduct`), hardware specs (CPU cores, RAM size), and local capabilities (`claude-agy`), enrolling the machine into Tuquet Cloud in sub-second time.
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
scoop bucket add tuquet https://github.com/tuquet/tuquet-scoop-bucket

# Install tuquet-runner (registers both 'tqr' and 'tuquet-runner' shims)
scoop install tuquet-runner
```

*Note: Scoop automatically persists `config/` (`.identity.json`, `.machine_id`) and `logs/` across version updates.*

### 2. From Source (Cargo)

```bash
git clone https://github.com/tuquet/tuquet-runner.git
cd tuquet-runner
cargo build --release
```
The compiled binary will be placed at `target/release/tqr.exe`.

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
 Tuquet Runner (tqr) System Diagnostics
============================================================
 Version:      0.1.0
 Hostname:     HNDW-NDTU6
 Fingerprint:  4C4C4544-0053-5810-8047-B2C04F484234
 Architecture: windows-x86_64
 Resources:    16 CPU Cores | 16070 MB RAM
 Capabilities: ["shell:pwsh", "agent:claude-agy"]
 Supervision:  Win32 Job Objects (Kernel-Level Zero-Zombie)
 Config Path:  C:\Users\ndtu6\.tuquet\config
------------------------------------------------------------
 Enrollment:   ENROLLED (Cloud Active)
 Device ID:    2a9cb9d8-dac6-4017-9214-69b65a458467
 Tenant ID:    b0000000-0000-0000-0000-000000000001
 Device Name:  HNDW-NDTU6
 Cloud URL:    https://api.tuquet.dev
============================================================
```

### 2. Zero-Touch Cloud Enrollment
Register your workstation with Tuquet Cloud:
```powershell
# Production (Cloud SaaS)
tqr enroll --token <WORKSPACE_ENROLLMENT_TOKEN>

# Self-Hosted / Local Development
tqr enroll --url "http://127.0.0.1:54321" --key "<SUPABASE_ANON_KEY>"
```

### 3. Standalone Ad-Hoc Execution (CLI Mode)
Execute commands locally with real-time log streaming:
```powershell
# Execute a PowerShell command
tqr exec -d shell -c "Get-Process | Select-Object -First 5"

# Execute an AI agent prompt via claude-agy
tqr exec -d agent -p "Explain Rust ownership in 3 bullet points"
```

### 4. Background Worker Daemon Mode
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
| `tqr info` | — | Display hardware fingerprint, specs, drivers, and enrollment state |
| `tqr enroll` | — | Enroll workstation with Tuquet Cloud via Supabase RPC |
| `tqr exec` | `-d, -c, -p` | Execute ad-hoc prompt or command locally (`shell`, `agent`, `http`) |
| `tqr run` | `<file>` | Execute a local job specification file (`.yaml` or `.json`) |
| `tqr worker` | `-s, -t` | Start worker daemon listening for remote jobs from Cloud Control Plane |

### Environment Variables

| Variable | Description | Default |
| :--- | :--- | :--- |
| `TUQUET_CLOUD_URL` | Tuquet Cloud / Supabase REST API endpoint | `https://api.tuquet.dev` |
| `TUQUET_API_KEY` | Supabase publishable / anon public key | — |
| `TUQUET_ENROLLMENT_TOKEN` | Optional workspace registration pairing token | — |
| `TUQUET_SERVER` | WebSocket control plane gateway URL | `wss://hub.tuquet.dev/api/v1/runner/ws` |
| `TUQUET_TOKEN` | Runner authentication device token | Loaded from `config/.identity.json` |

---

## 📜 License

Licensed under the [MIT License](LICENSE).
