import unittest
import time
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from helpers import call_runner_api

WORKFLOW_ID = "w0000000-0000-0000-0000-000000000001"

class TestDomain5JobLifecycle(unittest.TestCase):
    """Business Unit Tests for Runner Daemon REST API, Job State Machine, and Telemetry."""

    def test_01_job_submission_queued_state(self):
        """Rule 5.1: POST /api/v1/jobs accepts valid payload and returns initial queued state."""
        status_code, body = call_runner_api("POST", "/api/v1/jobs", {
            "workflowId": WORKFLOW_ID,
            "options": {
                "browserId": "unit_worker_01",
                "headless": True,
                "closeBrowserOnFinish": True
            }
        })

        self.assertEqual(status_code, 200)
        self.assertIn("jobId", body)
        self.assertEqual(body.get("status"), "queued")

    def test_02_job_execution_lifecycle_and_completion(self):
        """Rule 5.2 & 5.4: Job transitions from queued -> running -> completed."""
        _, submit_body = call_runner_api("POST", "/api/v1/jobs", {
            "workflowId": WORKFLOW_ID,
            "options": {
                "browserId": "unit_worker_lifecycle",
                "headless": True,
                "closeBrowserOnFinish": True
            }
        })
        job_id = submit_body["jobId"]

        # Poll status until terminal state
        completed = False
        final_status = None
        for _ in range(15):
            time.sleep(1)
            code, status_body = call_runner_api("GET", f"/api/v1/jobs/{job_id}/status")
            if code == 200:
                final_status = status_body.get("status")
                if final_status == "completed":
                    completed = True
                    break
                elif final_status in ("failed", "cancelled"):
                    break

        self.assertTrue(completed, f"Job did not complete successfully. Final status: {final_status}")

    def test_03_job_telemetry_history_logs(self):
        """Rule 5.3: Completed job records granular step logs with block names, durations, and timestamps."""
        _, submit_body = call_runner_api("POST", "/api/v1/jobs", {
            "workflowId": WORKFLOW_ID,
            "options": {
                "browserId": "unit_worker_telemetry",
                "headless": True,
                "closeBrowserOnFinish": True
            }
        })
        job_id = submit_body["jobId"]

        # Wait for finish
        for _ in range(15):
            time.sleep(1)
            _, status_body = call_runner_api("GET", f"/api/v1/jobs/{job_id}/status")
            if status_body.get("status") == "completed":
                break

        # Fetch history logs
        code, history_body = call_runner_api("GET", f"/api/v1/history/{job_id}/logs")
        self.assertEqual(code, 200)

        job_meta = history_body.get("job", {})
        self.assertEqual(job_meta.get("status"), "completed")
        self.assertIn("createdAt", job_meta)
        self.assertIn("endedAt", job_meta)

        logs = history_body.get("logs", [])
        self.assertGreaterEqual(len(logs), 3, "Execution must record at least 3 block steps")

        # Verify step block names
        block_names = [l.get("name") for l in logs]
        self.assertIn("trigger", block_names)
        self.assertIn("new-tab", block_names)
        self.assertIn("delay", block_names)

    def test_04_invalid_workflow_submission_rejection(self):
        """Rule 5.6: Submitting non-existent workflow returns 404 or bad request status."""
        code, body = call_runner_api("POST", "/api/v1/jobs", {
            "workflowId": "w9999999-9999-9999-9999-999999999999",
            "options": {"headless": True}
        })
        self.assertIn(code, (400, 404, 500))
        self.assertNotEqual(body.get("status"), "queued")

if __name__ == "__main__":
    unittest.main()
