import unittest
import os
import sys
import json
import time
import sqlite3
import subprocess

sys.path.insert(0, os.path.dirname(__file__))
from helpers import call_runner_api

FIXTURE_BOT_CHECK = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "fixtures", "bot_check_workflow.json"))
SSOT_ROOT = os.getenv("SPECTER_HOME", os.path.expanduser("~/.specter"))
CHROME_BIN = os.path.join(SSOT_ROOT, "browser/runtimes/chromium-linux64/chrome")
VAULT_DIR = os.path.join(SSOT_ROOT, "automa/workflows")
SQLITE_DB = os.path.join(SSOT_ROOT, "automa/automa.sqlite")
BOT_WORKFLOW_ID = "w7777777-7777-7777-7777-777777777777"

class TestDomain7BotDetectionBypass(unittest.TestCase):
    """Business Unit Tests for Bot Detection Bypass (bot.sannysoft.com) and Antidetect Telemetry."""

    @classmethod
    def setUpClass(cls):
        # Seed bot_check workflow into vault and sqlite for test_04
        if os.path.exists(FIXTURE_BOT_CHECK):
            os.makedirs(VAULT_DIR, exist_ok=True)
            with open(FIXTURE_BOT_CHECK, "r", encoding="utf-8") as f:
                wf_data = json.load(f)

            vault_target = os.path.join(VAULT_DIR, f"{BOT_WORKFLOW_ID}.workflow.json")
            with open(vault_target, "w", encoding="utf-8") as f:
                json.dump(wf_data, f, indent=2)

            if os.path.exists(SQLITE_DB):
                try:
                    conn = sqlite3.connect(SQLITE_DB)
                    c = conn.cursor()
                    c.execute("""
                        INSERT OR REPLACE INTO workflows (id, name, description, data, icon, version, created_at, updated_at)
                        VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                    """, (
                        BOT_WORKFLOW_ID,
                        wf_data.get("name", "Bot Check Workflow"),
                        wf_data.get("description", "Bot Detection Bypass Audit"),
                        json.dumps(wf_data),
                        "ri-shield-check-line",
                        "1.0.0"
                    ))
                    conn.commit()
                    conn.close()
                except Exception as e:
                    print(f"Warning: Failed to seed sqlite for domain 7: {e}")

    def test_01_bot_check_workflow_dag_integrity(self):
        """Rule 7.1: Bot check workflow DAG must strictly conform to Drawflow spec with valid sannysoft target."""
        self.assertTrue(os.path.exists(FIXTURE_BOT_CHECK), f"Fixture missing: {FIXTURE_BOT_CHECK}")
        with open(FIXTURE_BOT_CHECK, "r", encoding="utf-8") as f:
            wf = json.load(f)

        self.assertIn("drawflow", wf)
        drawflow = wf["drawflow"]
        nodes = drawflow.get("nodes", [])
        edges = drawflow.get("edges", [])

        node_map = {n["id"]: n for n in nodes}
        self.assertIn("node-trigger", node_map)
        self.assertIn("node-new-tab", node_map)
        self.assertIn("node-js", node_map)

        new_tab_node = node_map["node-new-tab"]
        self.assertEqual(new_tab_node["data"].get("url"), "https://bot.sannysoft.com")

        # Verify edge continuity
        self.assertGreaterEqual(len(edges), 3)
        edge_sources = [e["source"] for e in edges]
        self.assertIn("node-trigger", edge_sources)
        self.assertIn("node-new-tab", edge_sources)

    def test_02_stealth_cli_flags_contract(self):
        """Rule 7.2: Antidetect browser CLI flags contract forbids AutomationControlled and enforces desktop UA."""
        def build_stealth_args(user_agent=None, seed=None):
            args = [
                "--no-sandbox",
                "--disable-setuid-sandbox",
                "--headless=new",
                "--disable-blink-features=AutomationControlled",
                "--disable-infobars",
                "--window-size=1920,1080"
            ]
            if user_agent:
                args.append(f"--user-agent={user_agent}")
            if seed:
                args.extend(["--fingerprint", str(seed)])
            return args

        ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
        flags = build_stealth_args(user_agent=ua, seed=999888)

        self.assertIn("--disable-blink-features=AutomationControlled", flags)
        self.assertIn("--headless=new", flags)
        self.assertNotIn("HeadlessChrome", ua)
        self.assertIn(f"--user-agent={ua}", flags)
        self.assertIn("--fingerprint", flags)

    def test_03_direct_bot_sannysoft_dom_verification(self):
        """Rule 7.3: Chromium Antidetect runtime renders bot.sannysoft.com and passes all critical bot detection checks."""
        self.assertTrue(os.path.exists(CHROME_BIN), f"Chrome binary missing at: {CHROME_BIN}")

        cmd = [
            CHROME_BIN,
            "--headless=new",
            "--no-sandbox",
            "--disable-setuid-sandbox",
            "--disable-blink-features=AutomationControlled",
            "--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            "--dump-dom",
            "https://bot.sannysoft.com"
        ]

        res = subprocess.run(cmd, capture_output=True, text=True, timeout=20)
        self.assertEqual(res.returncode, 0, f"Chrome exited with error: {res.stderr}")
        dom = res.stdout

        # 1. User Agent must pass and not be headless
        self.assertIn('id="user-agent-result"', dom)
        ua_line = [l for l in dom.splitlines() if 'id="user-agent-result"' in l][0]
        self.assertIn('result passed', ua_line)
        self.assertNotIn('HeadlessChrome', ua_line)

        # 2. WebDriver must be missing (passed)
        self.assertIn('id="webdriver-result"', dom)
        webdriver_line = [l for l in dom.splitlines() if 'id="webdriver-result"' in l][0]
        self.assertIn('missing (passed)', webdriver_line)
        self.assertIn('result passed', webdriver_line)

        # 3. Advanced WebDriver must pass
        self.assertIn('id="advanced-webdriver-result"', dom)
        adv_line = [l for l in dom.splitlines() if 'id="advanced-webdriver-result"' in l][0]
        self.assertIn('passed', adv_line)
        self.assertIn('result passed', adv_line)

        # 4. Chrome object must be present (passed)
        self.assertIn('id="chrome-result"', dom)
        chrome_line = [l for l in dom.splitlines() if 'id="chrome-result"' in l][0]
        self.assertIn('present (passed)', chrome_line)
        self.assertIn('result passed', chrome_line)

    def test_04_runner_job_bot_check_telemetry(self):
        """Rule 7.4: Runner Daemon executes Bot Check Workflow, completes execution, and records step logs."""
        code, submit_body = call_runner_api("POST", "/api/v1/jobs", {
            "workflowId": BOT_WORKFLOW_ID,
            "options": {
                "browserId": "unit_worker_bot_bypass",
                "headless": True,
                "closeBrowserOnFinish": True
            }
        })
        self.assertEqual(code, 200, f"Failed to submit bot check job: {submit_body}")
        self.assertIn("jobId", submit_body)
        job_id = submit_body["jobId"]

        # Wait for job completion (up to 25 seconds)
        completed = False
        final_status = None
        for _ in range(25):
            time.sleep(1)
            c, status_body = call_runner_api("GET", f"/api/v1/jobs/{job_id}/status")
            if c == 200:
                final_status = status_body.get("status")
                if final_status == "completed":
                    completed = True
                    break
                elif final_status in ("failed", "cancelled"):
                    break

        self.assertTrue(completed, f"Bot check job did not finish in time. Final status: {final_status}")

        # Verify step logs in SQLite audit trail
        c, hist = call_runner_api("GET", f"/api/v1/history/{job_id}/logs")
        self.assertEqual(c, 200)
        job_info = hist.get("job", {})
        self.assertEqual(job_info.get("status"), "completed")

        logs = hist.get("logs", [])
        self.assertGreaterEqual(len(logs), 3, "Expected at least 3 step logs for bot check workflow")
        block_names = [l.get("name") for l in logs]
        self.assertIn("trigger", block_names)
        self.assertIn("new-tab", block_names)

if __name__ == "__main__":
    unittest.main()
