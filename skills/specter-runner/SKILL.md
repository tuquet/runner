---
name: specter-runner
description: >
  Kernel-level Win32 Job Object supervisor & daemon controller (port 8765).
  Trigger: /specter-runner, "specter runner", "start runner", "stop runner", "runner logs", "runner status", "zero zombie".
argument-hint: "[status|start|stop|restart|logs|probe]"
license: MIT
---

# Specter Process Supervisor (`specter-runner`)

Zero-zombie kernel supervisor. Spawns, monitors, and terminates background worker daemons on port 8765 bound to Win32 Job Objects.

## Code & Specification Pointers (Code-as-Docs)

Read native source definitions on demand:
- Kernel Supervisor (Win32 Job Object): [runner/src/core/supervisor.rs](../../src/core/supervisor.rs)
- Runner Contract Schema: [runner/src/protocol/schema.rs](../../src/protocol/schema.rs)
- Runner Execution Engine: [runner/src/core/engine.rs](../../src/core/engine.rs)
- Handshake & Capabilities: [runner/src/protocol/handshake.rs](../../src/protocol/handshake.rs)

## Native Commands

```powershell
# 1. Health Probe: Check daemon status on port 8765
specter runner status

# 2. Start Daemon: Launch background worker bound to Win32 Job Object tree
specter runner start -d

# 3. Stream Logs: Inspect runtime traces
specter runner logs --lines 50 -f

# 4. Graceful Shutdown: Atomically terminate supervisor and child process tree
specter runner stop

# 5. Hardware Capabilities & Driver Probe
specter runner probe
```

## Output Contract

Report status strictly:
```text
[RUNNER] Endpoint: http://127.0.0.1:8765 (<ONLINE|OFFLINE>)
• Daemon PID:  <pid> (Detached Worker)
• Driver:      mv3_extension_worker (CDP Bridge)
• Supervisor:  Win32 Job Object Tree (Zero Zombie)
• Log Path:    ~/.specter/logs/runner.log
Verdict: Worker ready for job execution. / Stopped: run 'specter runner start -d'.
```

## Boundaries

Scope: Process lifecycle supervision, background daemon control, and log streaming. Do NOT author workflow DAGs. Use `/specter-automa` for workflows. One-shot execution.
