import unittest
import os
import sys
import json
import sqlite3

sys.path.insert(0, os.path.dirname(__file__))

SSOT_ROOT = os.getenv("SPECTER_HOME", os.path.expanduser("~/.specter"))
VAULT_DIR = os.path.join(SSOT_ROOT, "automa", "workflows")
SQLITE_DB = os.path.join(SSOT_ROOT, "automa", "automa.sqlite")
FIXTURE_PATH = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", "fixtures", "test_browser_workflow.json"))
WORKFLOW_ID = "w0000000-0000-0000-0000-000000000001"

class TestDomain4WorkflowVault(unittest.TestCase):
    """Business Unit Tests for Automa Workflow DAG Structure, Vault Resolution, and SQLite DB."""

    def test_01_fixture_dag_structure_validation(self):
        """Rule 4.1: Automa workflow JSON must conform to the standard drawflow DAG specification."""
        self.assertTrue(os.path.exists(FIXTURE_PATH), f"Fixture workflow missing: {FIXTURE_PATH}")
        with open(FIXTURE_PATH, "r", encoding="utf-8") as f:
            wf = json.load(f)

        self.assertIn("name", wf)
        self.assertIn("drawflow", wf)
        drawflow = wf["drawflow"]
        self.assertIn("nodes", drawflow)
        self.assertIn("edges", drawflow)

        nodes = drawflow["nodes"]
        self.assertGreaterEqual(len(nodes), 1)

        # Trigger block must exist
        trigger_nodes = [n for n in nodes if n.get("label") == "trigger" or n.get("id") == "node-trigger"]
        self.assertGreaterEqual(len(trigger_nodes), 1, "Workflow must contain a trigger node")

        # Edges must connect existing node IDs
        node_ids = {n["id"] for n in nodes}
        for edge in drawflow["edges"]:
            self.assertIn(edge["source"], node_ids, f"Edge source {edge['source']} does not exist in nodes")
            self.assertIn(edge["target"], node_ids, f"Edge target {edge['target']} does not exist in nodes")

    def test_02_vault_file_resolution(self):
        """Rule 4.2: Workflows in vault must follow canonical naming ~/.specter/automa/workflows/<id>.workflow.json."""
        vault_file = os.path.join(VAULT_DIR, f"{WORKFLOW_ID}.workflow.json")
        self.assertTrue(os.path.exists(vault_file), f"Vault file not found at: {vault_file}")

        with open(vault_file, "r", encoding="utf-8") as f:
            vault_wf = json.load(f)

        self.assertEqual(vault_wf.get("name"), "E2E Browser Test Workflow")
        self.assertIn("drawflow", vault_wf)

    def test_03_sqlite_automa_db_records(self):
        """Rule 4.3: SQLite database (~/.specter/automa/automa.sqlite) stores workflow metadata."""
        self.assertTrue(os.path.exists(SQLITE_DB), f"Automa SQLite DB missing: {SQLITE_DB}")
        conn = sqlite3.connect(SQLITE_DB)
        cursor = conn.cursor()

        cursor.execute("SELECT id, name, version FROM workflows WHERE id = ?", (WORKFLOW_ID,))
        row = cursor.fetchone()
        conn.close()

        self.assertIsNotNone(row, f"Workflow {WORKFLOW_ID} not found in SQLite workflows table")
        self.assertEqual(row[0], WORKFLOW_ID)
        self.assertEqual(row[1], "E2E Browser Test Workflow")

    def test_04_variable_interpolation_contract(self):
        """Rule 4.4: Dynamic execution variables dictionary format contract."""
        sample_vars = {
            "target_url": "https://example.com/checkout",
            "retry_limit": 3,
            "is_headless": True
        }
        # Variables must be JSON serializable key-value map
        dumped = json.dumps(sample_vars)
        reloaded = json.loads(dumped)
        self.assertEqual(reloaded["target_url"], "https://example.com/checkout")
        self.assertEqual(reloaded["retry_limit"], 3)
        self.assertIs(reloaded["is_headless"], True)

if __name__ == "__main__":
    unittest.main()
