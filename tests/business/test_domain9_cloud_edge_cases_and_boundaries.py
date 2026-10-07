import unittest
import uuid
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from helpers import call_cloud_rpc, load_cloud_credentials

class TestDomain9CloudEdgeCasesAndBoundaries(unittest.TestCase):
    """Business Unit Tests for Cloud Edge Cases: Idempotence, Boundary Fingerprints, Auth and Nested Metadata."""

    def setUp(self):
        self.creds = load_cloud_credentials()
        self.profile_id = "c0000000-0000-0000-0000-000000000001"

    def test_01_idempotent_double_profile_release(self):
        """Rule 9.1: Calling release_browser twice consecutively is idempotent and succeeds without error."""
        # First release
        res1 = call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        self.assertTrue(res1.get("success"), f"First release failed: {res1}")

        # Second release immediately
        res2 = call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        self.assertTrue(res2.get("success"), f"Second release failed: {res2}")
        self.assertEqual(res2.get("status"), "idle")

    def test_02_boundary_fingerprint_seed_synthesis(self):
        """Rule 9.2: Boundary fingerprint seeds (0, max uint32) are clamped or safely randomized by cloud."""
        # 1. Seed = 0 (Should trigger auto-randomization of positive seed)
        res_zero = call_cloud_rpc("create_browser", {
            "p_name": f"Boundary-Zero-{uuid.uuid4().hex[:4]}",
            "p_fingerprint_seed": 0,
            "p_tenant_id": self.creds["tenant_id"]
        })
        self.assertTrue(res_zero.get("success"), f"Create with seed 0 failed: {res_zero}")
        self.assertGreater(res_zero.get("fingerprint_seed", 0), 0)

        # 2. Seed = 2147483647 (Max signed 32-bit integer)
        max_int32 = 2147483647
        res_max = call_cloud_rpc("create_browser", {
            "p_name": f"Boundary-Max-{uuid.uuid4().hex[:4]}",
            "p_fingerprint_seed": max_int32,
            "p_tenant_id": self.creds["tenant_id"]
        })
        self.assertTrue(res_max.get("success"), f"Create with max seed failed: {res_max}")
        self.assertEqual(res_max.get("fingerprint_seed"), max_int32)

    def test_03_unauthorized_device_token_mutation_rejection(self):
        """Rule 9.3: Calling acquire_browser with an invalid or spoofed device token is rejected."""
        bogus_token = "invalid_spoofed_device_token_12345"
        res = call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": bogus_token
        })

        # Must either fail success check or return an error response
        is_success = res.get("success", False)
        self.assertFalse(is_success, f"Acquire with bogus token unexpectedly succeeded: {res}")

    def test_04_deeply_nested_complex_metadata_persistence(self):
        """Rule 9.4: PostgreSQL JSONB handles deeply nested structures, Unicode, and numeric types safely."""
        complex_metadata = {
            "execution_id": f"exec_{uuid.uuid4().hex}",
            "nested_tree": {
                "layer1": {
                    "layer2": {
                        "tags": ["antidetect", "chromium", "production", 2026],
                        "active": True,
                        "ratio": 0.9998,
                        "null_field": None
                    }
                }
            },
            "vietnamese_unicode": "Hệ thống tự động hoá trình duyệt Tuquet trên VPS",
            "emoji_test": "🚀⚡🎯"
        }

        res = call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"],
            "p_metadata": complex_metadata
        })
        self.assertTrue(res.get("success"), f"Release with complex metadata failed: {res}")

    def test_05_advisory_warning_concurrent_device_inspection(self):
        """Rule 9.5: Concurrent acquire returns detailed workstation identity in advisory warning payload."""
        # Device 1 acquires profile
        call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })

        # Device 2 (simulated enrolled concurrent device)
        enroll_res = call_cloud_rpc("enroll_device", {
            "p_machine_fingerprint": f"HWID-EDGE-DEV2-{uuid.uuid4().hex[:8]}",
            "p_name": "Edge-Workstation-Beta",
            "p_os_info": "Linux Ubuntu",
            "p_cpu_cores": 4,
            "p_ram_mb": 8192,
            "p_capabilities": ["browser:automa"],
            "p_metadata": {}
        })
        self.assertIn("device_id", enroll_res, f"Enroll failed: {enroll_res}")
        dev2_id = enroll_res["device_id"]
        dev2_token = enroll_res["device_token"]

        res_dev2 = call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": dev2_id,
            "p_device_token": dev2_token
        })

        self.assertTrue(res_dev2.get("success"))
        self.assertTrue(res_dev2.get("in_use_warning"))
        warning_details = res_dev2.get("warning_details")
        self.assertIsNotNone(warning_details)
        self.assertIn("active_device_name", warning_details)
        self.assertIn("active_since", warning_details)

        # Cleanup: release profile back to idle
        call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })

if __name__ == "__main__":
    unittest.main()
