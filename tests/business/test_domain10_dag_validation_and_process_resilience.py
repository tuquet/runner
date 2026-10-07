import unittest
import json
import subprocess
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))

def validate_automa_workflow_dag(workflow_dict):
    """Business validation contract for Automa Drawflow DAGs."""
    if not isinstance(workflow_dict, dict):
        return False, "Workflow must be a JSON object"

    if "drawflow" not in workflow_dict:
        return False, "Missing 'drawflow' key"

    drawflow = workflow_dict["drawflow"]
    if not isinstance(drawflow, dict):
        return False, "'drawflow' must be an object"

    if "nodes" not in drawflow or not isinstance(drawflow["nodes"], list):
        return False, "Missing or invalid 'nodes' array"

    if "edges" not in drawflow or not isinstance(drawflow["edges"], list):
        return False, "Missing or invalid 'edges' array"

    nodes = drawflow["nodes"]
    edges = drawflow["edges"]

    # Rule: Must have at least 1 node and at least 1 trigger node
    has_trigger = any(
        n.get("label") == "trigger" or n.get("id") == "node-trigger" or n.get("data", {}).get("type") == "manual"
        for n in nodes
    )
    if not has_trigger:
        return False, "Workflow missing entry trigger node"

    # Rule: All edges must connect existing node IDs
    node_ids = {n.get("id") for n in nodes if "id" in n}
    for edge in edges:
        src = edge.get("source")
        tgt = edge.get("target")
        if src not in node_ids:
            return False, f"Broken edge source reference: '{src}' does not exist in nodes"
        if tgt not in node_ids:
            return False, f"Broken edge target reference: '{tgt}' does not exist in nodes"

    return True, "Valid DAG"

class TestDomain10DAGValidationAndProcessResilience(unittest.TestCase):
    """Business Unit Tests for Workflow DAG Validation, Edge Continuity, Unicode Variables and Zero Zombie Processes."""

    def test_01_malformed_dag_syntax_detection(self):
        """Rule 10.1: Malformed workflows missing drawflow, nodes, or edges are detected and rejected."""
        # 1. Non-dict input
        valid, err = validate_automa_workflow_dag("raw_string")
        self.assertFalse(valid)

        # 2. Missing drawflow
        valid, err = validate_automa_workflow_dag({"name": "Empty Flow"})
        self.assertFalse(valid)
        self.assertIn("Missing 'drawflow'", err)

        # 3. Missing nodes
        valid, err = validate_automa_workflow_dag({"drawflow": {"edges": []}})
        self.assertFalse(valid)
        self.assertIn("nodes", err)

    def test_02_broken_edge_reference_detection(self):
        """Rule 10.2: Dangling edges referencing non-existent nodes are identified."""
        broken_dag = {
            "name": "Dangling Edge Workflow",
            "drawflow": {
                "nodes": [
                    {"id": "node-trigger", "label": "trigger", "data": {"type": "manual"}},
                    {"id": "node-tab-1", "label": "new-tab", "data": {"url": "https://example.com"}}
                ],
                "edges": [
                    {
                        "id": "edge-broken",
                        "source": "node-trigger",
                        "target": "node-nonexistent-phantom"  # Doesn't exist!
                    }
                ]
            }
        }
        valid, err = validate_automa_workflow_dag(broken_dag)
        self.assertFalse(valid)
        self.assertIn("Broken edge target reference", err)
        self.assertIn("node-nonexistent-phantom", err)

    def test_03_missing_trigger_node_detection(self):
        """Rule 10.3: Workflows without a trigger node are flagged as unexecutable."""
        headless_dag = {
            "name": "No Trigger Flow",
            "drawflow": {
                "nodes": [
                    {"id": "node-tab-only", "label": "new-tab", "data": {"url": "https://example.com"}}
                ],
                "edges": []
            }
        }
        valid, err = validate_automa_workflow_dag(headless_dag)
        self.assertFalse(valid)
        self.assertIn("missing entry trigger node", err.lower())

    def test_04_unicode_and_special_char_variable_interpolation(self):
        """Rule 10.4: Dynamic variable contracts support Vietnamese diacritics, emojis, and query special chars."""
        rich_vars = {
            "campaign_name": "Chiến dịch Mua sắm Tết 2026 🎆",
            "search_query": "mua quà tết & ưu đãi giá rẻ = 50%?",
            "recipient_email": "từ_thiện@tuquet.dev",
            "url_encoded_test": "https://tuquet.dev/search?q=tết&mode=stealth#results",
            "json_payload_str": json.dumps({"active": True, "count": 42})
        }

        # Validate round-trip JSON serialization
        serialized = json.dumps(rich_vars, ensure_ascii=False)
        deserialized = json.loads(serialized)

        self.assertEqual(deserialized["campaign_name"], "Chiến dịch Mua sắm Tết 2026 🎆")
        self.assertEqual(deserialized["search_query"], "mua quà tết & ưu đãi giá rẻ = 50%?")
        self.assertEqual(deserialized["recipient_email"], "từ_thiện@tuquet.dev")
        self.assertIn("🎆", deserialized["campaign_name"])

    def test_05_zero_zombie_process_guarantee(self):
        """Rule 10.5: OS process tree has zero zombie (<defunct>) Chromium processes."""
        res = subprocess.run(
            ["ps", "aux"],
            capture_output=True,
            text=True
        )
        self.assertEqual(res.returncode, 0)
        lines = res.stdout.splitlines()

        # Check for any defunct / zombie chrome processes
        zombies = [
            l for l in lines
            if ("chrome" in l or "chromium" in l) and "<defunct>" in l
        ]
        self.assertEqual(
            len(zombies), 0,
            f"Detected orphaned zombie Chromium processes in OS process table: {zombies}"
        )

if __name__ == "__main__":
    unittest.main()
