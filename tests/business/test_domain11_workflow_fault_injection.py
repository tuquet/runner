import unittest
import subprocess
import json
import tempfile
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from helpers import call_runner_api

WORKFLOW_ID = "w0000000-0000-0000-0000-000000000001"

class TestDomain11WorkflowFaultInjection(unittest.TestCase):
    """Business Unit Tests for Automa DAG Fault Injection, Error Recovery, and Storage Vault Lifecycle."""

    def test_01_invalid_navigation_scheme_resilience(self):
        """Rule 11.1: Workflow navigating to invalid URL scheme executes gracefully without panicking runner."""
        wf = {
            "name": "Invalid Scheme Test",
            "description": "Navigates to invalid URL scheme",
            "drawflow": {
                "nodes": [
                    {"id": "node-trigger", "label": "trigger", "data": {"type": "manual"}},
                    {"id": "node-new-tab", "label": "new-tab", "data": {"url": "invalid-scheme://never-land.test"}}
                ],
                "edges": [
                    {
                        "id": "edge-1",
                        "source": "node-trigger",
                        "sourceHandle": "node-trigger-output-1",
                        "target": "node-new-tab",
                        "targetHandle": "node-new-tab-input-1"
                    }
                ]
            }
        }
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
            json.dump(wf, f)
            tmp_path = f.name

        try:
            res = subprocess.run(
                ["tuquet", "automa", "run", tmp_path, "--headless"],
                capture_output=True,
                text=True,
                timeout=15
            )
            self.assertEqual(res.returncode, 0, f"Automa run crashed on invalid scheme: {res.stderr}")
            self.assertIn("Run completed successfully", res.stdout)
            self.assertIn("node-new-tab", res.stdout)
        finally:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)

    def test_02_javascript_runtime_error_event_capture(self):
        """Rule 11.2: Workflow with JS execution error captures error block event without aborting runner daemon."""
        wf = {
            "name": "JS Error Capture Test",
            "description": "Executes invalid JS block",
            "drawflow": {
                "nodes": [
                    {"id": "node-trigger", "label": "trigger", "data": {"type": "manual"}},
                    {"id": "node-js", "label": "javascript-code", "data": {"code": "throw new Error('Deliberate JS test error');"}}
                ],
                "edges": [
                    {
                        "id": "edge-1",
                        "source": "node-trigger",
                        "sourceHandle": "node-trigger-output-1",
                        "target": "node-js",
                        "targetHandle": "node-js-input-1"
                    }
                ]
            }
        }
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
            json.dump(wf, f)
            tmp_path = f.name

        try:
            res = subprocess.run(
                ["tuquet", "automa", "run", tmp_path, "--headless"],
                capture_output=True,
                text=True,
                timeout=15
            )
            self.assertEqual(res.returncode, 0)
            self.assertIn('"type":"error"', res.stdout)
            self.assertIn("node-js", res.stdout)
            self.assertIn("workflow_finished", res.stdout)
        finally:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)

    def test_03_nonexistent_workflow_id_api_rejection(self):
        """Rule 11.3: Submitting job with nonexistent workflow ID returns HTTP 404 with descriptive error message."""
        status_code, body = call_runner_api("POST", "/api/v1/jobs", {
            "workflowId": "w9999999-9999-9999-9999-999999999999",
            "options": {"headless": True}
        })
        self.assertEqual(status_code, 404)
        self.assertEqual(body.get("status"), "error")
        self.assertIn("not found in database or vault", body.get("message", ""))

    def test_04_workflow_export_import_roundtrip_integrity(self):
        """Rule 11.4: Workflow export produces valid JSON AST and re-importing under new ID preserves structure."""
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
            export_path = f.name

        test_id = "w-roundtrip-test-01"
        try:
            # 1. Export
            exp_res = subprocess.run(
                ["tuquet", "automa", "workflow", "export", WORKFLOW_ID, "-o", export_path],
                capture_output=True,
                text=True
            )
            self.assertEqual(exp_res.returncode, 0, f"Export failed: {exp_res.stderr}")

            # Verify exported file
            with open(export_path, "r") as f:
                data = json.load(f)
            self.assertIn("drawflow", data)
            self.assertIn("nodes", data["drawflow"])

            # 2. Import under new ID
            imp_res = subprocess.run(
                ["tuquet", "automa", "workflow", "import", export_path, "--id", test_id, "-n", "Roundtrip Workflow"],
                capture_output=True,
                text=True
            )
            self.assertEqual(imp_res.returncode, 0, f"Import failed: {imp_res.stderr}")
            self.assertIn("Workflow Successfully Imported", imp_res.stdout)

            # 3. Inspect imported workflow
            info_res = subprocess.run(
                ["tuquet", "automa", "workflow", "info", test_id],
                capture_output=True,
                text=True
            )
            self.assertEqual(info_res.returncode, 0)
            self.assertIn("VALID WORKFLOW", info_res.stdout)
            self.assertIn(test_id, info_res.stdout)
        finally:
            if os.path.exists(export_path):
                os.remove(export_path)
            subprocess.run(["tuquet", "automa", "workflow", "delete", test_id, "--vault"], capture_output=True)

    def test_05_workflow_deletion_and_vault_purge(self):
        """Rule 11.5: Deleting workflow with --vault purges both SQLite row and vault JSON file."""
        test_id = "w-del-purge-test-02"
        wf_minimal = {
            "name": "Purge Test Workflow",
            "description": "Created to verify vault purge",
            "drawflow": {
                "nodes": [{"id": "node-trigger", "label": "trigger", "data": {"type": "manual"}}],
                "edges": []
            }
        }
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
            json.dump(wf_minimal, f)
            tmp_path = f.name

        try:
            # Import workflow
            imp_res = subprocess.run(
                ["tuquet", "automa", "workflow", "import", tmp_path, "--id", test_id],
                capture_output=True,
                text=True
            )
            self.assertEqual(imp_res.returncode, 0)

            specter_root = os.getenv("SPECTER_HOME", os.path.expanduser("~/.specter"))
            vault_file = os.path.join(specter_root, "automa", "workflows", f"{test_id}.workflow.json")
            self.assertTrue(os.path.exists(vault_file), f"Vault file was not created: {vault_file}")

            # Delete with --vault
            del_res = subprocess.run(
                ["tuquet", "automa", "workflow", "delete", test_id, "--vault"],
                capture_output=True,
                text=True
            )
            self.assertEqual(del_res.returncode, 0)
            self.assertIn("deleted successfully", del_res.stdout)

            # Verify file is removed from disk
            self.assertFalse(os.path.exists(vault_file), "Vault file was not deleted from disk")

            # Verify subsequent delete reports idempotent not-found message
            del_again = subprocess.run(
                ["tuquet", "automa", "workflow", "delete", test_id, "--vault"],
                capture_output=True,
                text=True
            )
            self.assertIn("was not found in database or vault", del_again.stdout)
        finally:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)

    def test_06_execution_timeout_boundary(self):
        """Rule 11.6: Workflow with 4s delay executed under --timeout 1 aborts within the deadline."""
        wf = {
            "name": "Timeout Workflow",
            "description": "Delay 4000ms",
            "drawflow": {
                "nodes": [
                    {"id": "node-trigger", "label": "trigger", "data": {"type": "manual"}},
                    {"id": "node-delay", "label": "delay", "data": {"delay": 4000}}
                ],
                "edges": [
                    {
                        "id": "edge-1",
                        "source": "node-trigger",
                        "sourceHandle": "node-trigger-output-1",
                        "target": "node-delay",
                        "targetHandle": "node-delay-input-1"
                    }
                ]
            }
        }
        with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
            json.dump(wf, f)
            tmp_path = f.name

        try:
            start = os.times().elapsed
            # Run with --timeout 1
            res = subprocess.run(
                ["tuquet", "automa", "run", tmp_path, "--headless", "--timeout", "1"],
                capture_output=True,
                text=True,
                timeout=10
            )
            elapsed = os.times().elapsed - start
            # Verify execution completed around timeout and did not hang for 4+ seconds
            self.assertLess(elapsed, 4.5, f"Execution hung beyond expected timeout: {elapsed}s")
        finally:
            if os.path.exists(tmp_path):
                os.remove(tmp_path)

if __name__ == "__main__":
    unittest.main()
