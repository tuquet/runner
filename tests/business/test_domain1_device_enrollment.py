import unittest
import uuid
import time
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from helpers import call_cloud_rpc, load_cloud_credentials

class TestDomain1DeviceEnrollment(unittest.TestCase):
    """Business Unit Tests for Fleet Enrollment and Device Identity Authentication."""

    def setUp(self):
        self.creds = load_cloud_credentials()
        self.tenant_id = self.creds["tenant_id"]
        self.unique_suffix = str(uuid.uuid4())[:8]

    def test_01_enroll_fresh_device(self):
        """Rule 1.1: New device enrollment must generate device_id and device_token."""
        hwid = f"HWID-UNIT-{self.unique_suffix}-1"
        res = call_cloud_rpc("enroll_device", {
            "p_machine_fingerprint": hwid,
            "p_name": f"Unit-Tester-Node-{self.unique_suffix}",
            "p_os_info": "Linux x86_64 Ubuntu",
            "p_cpu_cores": 4,
            "p_ram_mb": 8192,
            "p_capabilities": ["browser:automa"],
            "p_metadata": {"tested_by": "domain1_unit_test"}
        })

        self.assertNotIn("error_http_code", res, f"RPC call failed: {res}")
        self.assertIn("device_id", res, "Response missing device_id")
        self.assertIn("device_token", res, "Response missing device_token")
        self.assertTrue(res["device_token"].startswith("tqr_sec_"), "Invalid token format")
        self.assertEqual(res.get("status"), "idle")

    def test_02_enroll_idempotent_reissue(self):
        """Rule 1.2: Re-enrolling with existing fingerprint must update specs and preserve identity."""
        hwid = f"HWID-IDEMPOTENT-{self.unique_suffix}"
        res1 = call_cloud_rpc("enroll_device", {
            "p_machine_fingerprint": hwid,
            "p_name": "Idempotent-Initial",
            "p_os_info": "Linux",
            "p_cpu_cores": 2,
            "p_ram_mb": 4096,
            "p_capabilities": ["shell:bash"],
            "p_metadata": {"revision": 1}
        })
        dev_id1 = res1.get("device_id")
        token1 = res1.get("device_token")
        self.assertIsNotNone(dev_id1)

        # Re-enroll with upgraded RAM/CPU
        res2 = call_cloud_rpc("enroll_device", {
            "p_machine_fingerprint": hwid,
            "p_name": "Idempotent-Upgraded",
            "p_os_info": "Linux",
            "p_cpu_cores": 8,
            "p_ram_mb": 16384,
            "p_capabilities": ["shell:bash", "browser:automa"],
            "p_metadata": {"revision": 2}
        })
        dev_id2 = res2.get("device_id")
        token2 = res2.get("device_token")

        self.assertEqual(dev_id1, dev_id2, "Device ID must remain consistent across re-enrollments")
        self.assertIsNotNone(token2)

    def test_03_reject_invalid_device_token(self):
        """Rule 1.4: Protected RPC calls must reject forged or mismatched device tokens."""
        res = call_cloud_rpc("acquire_browser", {
            "p_browser_id": "c0000000-0000-0000-0000-000000000001",
            "p_device_id": self.creds["device_id"],
            "p_device_token": "forged_invalid_secret_token_12345"
        })
        self.assertFalse(res.get("success", True), "Invalid token must not succeed")
        self.assertIn("Invalid device credentials", res.get("error", ""))

    def test_04_device_heartbeat_telemetry(self):
        """Rule 1.5: Enrolled device sends periodic heartbeats and updates telemetry."""
        res = call_cloud_rpc("heartbeat", {
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"],
            "p_active_jobs": 1,
            "p_telemetry": {
                "cpu_load_pct": 22.4,
                "ram_usage_mb": 2048,
                "epoch": int(time.time())
            }
        })
        self.assertNotIn("error_http_code", res)
        self.assertTrue(res.get("success"), f"Heartbeat failed: {res}")

if __name__ == "__main__":
    unittest.main()
