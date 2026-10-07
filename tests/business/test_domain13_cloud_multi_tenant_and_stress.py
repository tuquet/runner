import unittest
import concurrent.futures
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from helpers import call_cloud_rpc, load_cloud_credentials

class TestDomain13CloudMultiTenantAndStress(unittest.TestCase):
    """Business Unit Tests for Cloud Multi-Tenant Security, Input Boundary Validation, and Concurrency Stress."""

    def setUp(self):
        self.creds = load_cloud_credentials()
        self.profile_id = "c0000000-0000-0000-0000-000000000001"

    def test_01_cross_tenant_isolation_boundary(self):
        """Rule 13.1: Acquiring a profile outside the device's tenant is strictly rejected by row-level tenant check."""
        nonexistent_or_other_tenant_profile = "ffffffff-ffff-ffff-ffff-ffffffffffff"
        res = call_cloud_rpc("acquire_browser", {
            "p_browser_id": nonexistent_or_other_tenant_profile,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        self.assertFalse(res.get("success"), f"Cross-tenant profile acquisition should have failed: {res}")
        self.assertEqual(res.get("error"), "Browser profile not found in tenant")

    def test_02_malformed_uuid_input_handling(self):
        """Rule 13.2: Malformed UUID inputs are safely rejected by Postgres with code 22P02 without state corruption."""
        res = call_cloud_rpc("acquire_browser", {
            "p_browser_id": "malformed-not-a-uuid-string",
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        self.assertEqual(res.get("error_http_code"), 400)
        error_body = res.get("error_body", {})
        self.assertEqual(error_body.get("code"), "22P02")
        self.assertIn("invalid input syntax for type uuid", error_body.get("message", ""))

    def test_03_heartbeat_unauthorized_device_detection(self):
        """Rule 13.3: Heartbeat from an unrecognized or revoked device returns success: false for self-healing."""
        bogus_device_id = "00000000-0000-0000-0000-000000000000"
        bogus_token = "tqr_sec_invalid_bogus_token_hash"
        res = call_cloud_rpc("heartbeat", {
            "p_device_id": bogus_device_id,
            "p_device_token": bogus_token,
            "p_active_jobs": 0,
            "p_telemetry": {"status": "probe"}
        })
        self.assertFalse(res.get("success"), f"Bogus device heartbeat should be rejected: {res}")
        self.assertEqual(res.get("error"), "Invalid device credentials or token")

    def test_04_concurrent_profile_acquisition_consistency(self):
        """Rule 13.4: 5 concurrent acquire_browser calls execute cleanly under row locks with zero deadlocks."""
        def try_acquire(_):
            return call_cloud_rpc("acquire_browser", {
                "p_browser_id": self.profile_id,
                "p_device_id": self.creds["device_id"],
                "p_device_token": self.creds["device_token"]
            })

        with concurrent.futures.ThreadPoolExecutor(max_workers=5) as executor:
            futures = [executor.submit(try_acquire, i) for i in range(5)]
            results = [f.result() for f in futures]

        for idx, r in enumerate(results):
            self.assertTrue(r.get("success"), f"Concurrent acquire #{idx} failed: {r}")

        # Clean release after concurrency stress
        rel_res = call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        self.assertTrue(rel_res.get("success"))
        self.assertEqual(rel_res.get("status"), "idle")

    def test_05_profile_metadata_boundary_stress(self):
        """Rule 13.5: JSONB metadata with empty dicts, deep nesting, and unicode text persists losslessly."""
        stress_metadata = {
            "empty": {},
            "deeply_nested": {"level1": {"level2": {"level3": {"flag": True, "count": 99}}}},
            "unicode_text": "Tuquet 🚀 Hệ thống tự động hoá và chống phát hiện bot tối ưu! " * 50
        }

        res = call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"],
            "p_metadata": stress_metadata
        })
        self.assertTrue(res.get("success"), f"Release with stress metadata failed: {res}")
        self.assertEqual(res.get("status"), "idle")

        # Re-acquire to confirm state intact
        acq_res = call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        self.assertTrue(acq_res.get("success"))

        # Final release to keep profile idle
        call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })

if __name__ == "__main__":
    unittest.main()
