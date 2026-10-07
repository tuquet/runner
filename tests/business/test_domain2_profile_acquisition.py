import unittest
import uuid
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from helpers import call_cloud_rpc, load_cloud_credentials

class TestDomain2ProfileAcquisition(unittest.TestCase):
    """Business Unit Tests for Browser Profile Acquisition, PRNG Specs, and Advisory Warnings."""

    def setUp(self):
        self.creds = load_cloud_credentials()
        self.profile_id = "c0000000-0000-0000-0000-000000000001" # FB-Ad-Spender-01

    def test_01_acquire_idle_profile_clean(self):
        """Rule 2.1: Acquiring an idle profile succeeds with clean status and in_use_warning=False."""
        # Ensure profile is idle first
        call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })

        res = call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })

        self.assertNotIn("error_http_code", res)
        self.assertTrue(res.get("success"), f"Acquire failed: {res}")
        self.assertFalse(res.get("in_use_warning"), "Clean acquire should not have in_use_warning")
        self.assertIsNone(res.get("warning_details"))
        self.assertEqual(res.get("device_name"), self.creds.get("name", "vps-master-runner"))
        self.assertIn("launched_at", res)

    def test_02_fingerprint_spec_integrity(self):
        """Rule 2.2: Acquired profile contains complete deterministic C++ PRNG hardware specs."""
        res = call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        self.assertTrue(res.get("success"))
        browser = res.get("browser", {})

        # Verify Core Deterministic PRNG Attributes
        self.assertEqual(browser.get("id"), self.profile_id)
        self.assertEqual(browser.get("name"), "FB-Ad-Spender-01")
        self.assertIsInstance(browser.get("fingerprint_seed"), int)
        self.assertEqual(browser.get("fingerprint_seed"), 133742)
        self.assertEqual(browser.get("os_platform"), "windows")
        self.assertEqual(browser.get("browser_brand"), "Chrome")
        self.assertTrue(str(browser.get("engine_version", "")).startswith("148."))
        self.assertGreaterEqual(browser.get("cpu_cores", 0), 2)
        self.assertGreaterEqual(browser.get("ram_gb", 0), 4)
        self.assertEqual(browser.get("timezone"), "Asia/Ho_Chi_Minh")
        self.assertEqual(browser.get("locale"), "vi-VN")

    def test_03_nonexistent_profile_error_handling(self):
        """Rule 2.4: Attempting to acquire non-existent profile returns structured error without crash."""
        res = call_cloud_rpc("acquire_browser", {
            "p_browser_id": "00000000-0000-0000-0000-000000000000",
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        self.assertFalse(res.get("success", True))
        self.assertIn("not found", res.get("error", "").lower())

    def test_04_advisory_in_use_warning_concurrent_devices(self):
        """Rule 2.5: Concurrent acquisition by another device must NOT hard-block; returns in_use_warning=True."""
        # 1. Device A acquires profile
        dev_a_res = call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        self.assertTrue(dev_a_res.get("success"))

        # 2. Enroll a secondary mock device B
        dev_b_enroll = call_cloud_rpc("enroll_device", {
            "p_machine_fingerprint": f"HWID-DEV-B-{uuid.uuid4().hex[:8]}",
            "p_name": "Device-Beta-Colleague",
            "p_os_info": "macOS Sonoma",
            "p_cpu_cores": 10,
            "p_ram_mb": 32768,
            "p_capabilities": ["browser:automa"],
            "p_metadata": {}
        })
        dev_b_id = dev_b_enroll["device_id"]
        dev_b_token = dev_b_enroll["device_token"]

        # 3. Device B attempts to acquire the profile currently active on Device A
        dev_b_res = call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": dev_b_id,
            "p_device_token": dev_b_token
        })

        # MUST SUCCEED (Optimistic Concurrency - No Hard Lock)
        self.assertTrue(dev_b_res.get("success"), "Secondary device should NOT be blocked!")
        # MUST WARN
        self.assertTrue(dev_b_res.get("in_use_warning"), "Secondary device MUST receive advisory warning!")
        details = dev_b_res.get("warning_details", {})
        self.assertEqual(details.get("active_device_name"), self.creds.get("name", "vps-master-runner"))
        self.assertIn("active_since", details)

        # Cleanup: Release both
        call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": dev_b_id,
            "p_device_token": dev_b_token
        })
        call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })

if __name__ == "__main__":
    unittest.main()
