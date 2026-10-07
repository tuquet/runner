import unittest
import time
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from helpers import call_runner_api

WORKFLOW_ID = "w0000000-0000-0000-0000-000000000001"

class TestDomain8JobControlAndCancellation(unittest.TestCase):
    """Business Unit Tests for Job Control (Pause, Resume, Kill/Cancel) and History Store Operations."""

    def test_01_active_jobs_pagination_and_search(self):
        """Rule 8.1: GET /api/v1/jobs supports pagination (limit, offset) and search filtering."""
        code, body = call_runner_api("GET", "/api/v1/jobs?limit=5&offset=0")
        self.assertEqual(code, 200)
        self.assertIsInstance(body, list)

        # Search for non-existent job ID
        code_search, body_search = call_runner_api("GET", "/api/v1/jobs?search=nonexistent_job_id_xyz")
        self.assertEqual(code_search, 200)
        self.assertEqual(len(body_search), 0)

    def test_02_job_pause_state_transition(self):
        """Rule 8.2: POST /api/v1/jobs/{job_id}/pause transitions active job to paused."""
        code, submit_body = call_runner_api("POST", "/api/v1/jobs", {
            "workflowId": WORKFLOW_ID,
            "options": {
                "browserId": "unit_worker_pause_test",
                "headless": True,
                "closeBrowserOnFinish": False
            }
        })
        self.assertEqual(code, 200)
        job_id = submit_body["jobId"]

        # Wait briefly until worker picks up the job
        time.sleep(1)

        pause_code, pause_body = call_runner_api("POST", f"/api/v1/jobs/{job_id}/pause")
        self.assertEqual(pause_code, 200)
        self.assertTrue(pause_body.get("success"))
        self.assertEqual(pause_body.get("status"), "paused")

        # Verify status endpoint reflects paused
        s_code, s_body = call_runner_api("GET", f"/api/v1/jobs/{job_id}/status")
        self.assertEqual(s_code, 200)
        self.assertEqual(s_body.get("status"), "paused")

        # Cleanup: terminate the paused job
        call_runner_api("DELETE", f"/api/v1/jobs/{job_id}")

    def test_03_job_resume_state_transition(self):
        """Rule 8.3: POST /api/v1/jobs/{job_id}/resume transitions paused job back to running."""
        code, submit_body = call_runner_api("POST", "/api/v1/jobs", {
            "workflowId": WORKFLOW_ID,
            "options": {
                "browserId": "unit_worker_resume_test",
                "headless": True,
                "closeBrowserOnFinish": False
            }
        })
        self.assertEqual(code, 200)
        job_id = submit_body["jobId"]

        time.sleep(1)
        # Pause first
        call_runner_api("POST", f"/api/v1/jobs/{job_id}/pause")

        # Resume
        res_code, res_body = call_runner_api("POST", f"/api/v1/jobs/{job_id}/resume")
        self.assertEqual(res_code, 200)
        self.assertTrue(res_body.get("success"))
        self.assertEqual(res_body.get("status"), "running")

        # Verify status endpoint reflects running
        s_code, s_body = call_runner_api("GET", f"/api/v1/jobs/{job_id}/status")
        self.assertEqual(s_code, 200)
        self.assertEqual(s_body.get("status"), "running")

        # Cleanup
        call_runner_api("DELETE", f"/api/v1/jobs/{job_id}")

    def test_04_job_cancellation_mid_execution(self):
        """Rule 8.4: DELETE /api/v1/jobs/{job_id} cancels active job and removes from active jobs list."""
        code, submit_body = call_runner_api("POST", "/api/v1/jobs", {
            "workflowId": WORKFLOW_ID,
            "options": {
                "browserId": "unit_worker_kill_test",
                "headless": True,
                "closeBrowserOnFinish": True
            }
        })
        self.assertEqual(code, 200)
        job_id = submit_body["jobId"]

        time.sleep(0.5)

        # Cancel / Kill the job
        del_code, _ = call_runner_api("DELETE", f"/api/v1/jobs/{job_id}")
        self.assertEqual(del_code, 200)

        # Verify job is evicted from active jobs
        _, active_list = call_runner_api("GET", "/api/v1/jobs")
        active_ids = [j.get("job_id") for j in active_list]
        self.assertNotIn(job_id, active_ids)

    def test_05_control_nonexistent_job_rejection(self):
        """Rule 8.5: Operations on non-existent jobs return 404 Not Found."""
        fake_id = "00000000-dead-beef-0000-000000000000"

        c_pause, _ = call_runner_api("POST", f"/api/v1/jobs/{fake_id}/pause")
        self.assertEqual(c_pause, 404)

        c_resume, _ = call_runner_api("POST", f"/api/v1/jobs/{fake_id}/resume")
        self.assertEqual(c_resume, 404)

        c_kill, _ = call_runner_api("DELETE", f"/api/v1/jobs/{fake_id}")
        self.assertEqual(c_kill, 404)

    def test_06_history_item_deletion_and_purge(self):
        """Rule 8.6: DELETE /api/v1/history/{job_id} removes job history record from SQLite."""
        # 1. Fetch any existing history item
        code, hist_items = call_runner_api("GET", "/api/v1/history?limit=1")
        self.assertEqual(code, 200)

        if hist_items and len(hist_items) > 0:
            target_id = hist_items[0].get("id")
            # Delete this history item
            del_code, del_body = call_runner_api("DELETE", f"/api/v1/history/{target_id}")
            self.assertEqual(del_code, 200)
            self.assertTrue(del_body.get("success"))

            # Verify logs for this deleted item returns job: None or error
            log_code, log_body = call_runner_api("GET", f"/api/v1/history/{target_id}/logs")
            self.assertEqual(log_code, 200)
            self.assertIsNone(log_body.get("job"))

if __name__ == "__main__":
    unittest.main()
