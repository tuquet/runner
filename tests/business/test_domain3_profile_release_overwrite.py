import unittest
import uuid
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from helpers import call_cloud_rpc, load_cloud_credentials

class TestDomain3ProfileReleaseOverwrite(unittest.TestCase):
    """Business Unit Tests for Profile Release, Storage Snapshots, and Last-Write-Wins Overwrite."""

    def setUp(self):
        self.creds = load_cloud_credentials()
        self.profile_id = "c0000000-0000-0000-0000-000000000001" # FB-Ad-Spender-01

    def test_01_release_normal_idle_reset(self):
        """Rule 3.1: Releasing an active profile resets status to idle and clears device locks."""
        # 1. Acquire
        acq = call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        self.assertTrue(acq.get("success"))

        # 2. Release
        rel = call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        self.assertNotIn("error_http_code", rel)
        self.assertTrue(rel.get("success"), f"Release failed: {rel}")
        self.assertEqual(rel.get("status"), "idle")
        self.assertFalse(rel.get("overwritten_concurrently"))
        self.assertIn("released_at", rel)

    def test_02_release_storage_snapshot_metadata(self):
        """Rule 3.2: Release updates storage snapshot path, size, hash, and cookies count."""
        call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })

        test_hash = "11223344556677889900aabbccddeeff11223344556677889900aabbccddeeff"
        test_path = f"profiles/{self.profile_id}/unit_test_snapshot.tar.zst"
        test_size = 5242880

        rel = call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"],
            "p_storage_path": test_path,
            "p_storage_size_bytes": test_size,
            "p_storage_hash": test_hash,
            "p_cookies_count": 42,
            "p_metadata": {"tested_by": "domain3_snapshot"}
        })
        self.assertTrue(rel.get("success"))

        # Verify by acquiring and checking browser attributes
        acq_check = call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })
        browser = acq_check.get("browser", {})
        self.assertEqual(browser.get("storage_path"), test_path)
        self.assertEqual(browser.get("storage_size_bytes"), test_size)
        self.assertEqual(browser.get("storage_hash"), test_hash)
        self.assertEqual(browser.get("cookies_count"), 42)

        # Cleanup
        call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })

    def test_03_release_last_write_wins_and_concurrent_flag(self):
        """Rule 3.3: Concurrent release flags overwritten_concurrently=True if another device holds the lock."""
        # 1. Device A acquires profile
        call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })

        # 2. Enroll Device B and acquire the same profile
        dev_b = call_cloud_rpc("enroll_device", {
            "p_machine_fingerprint": f"HWID-DEV-C-{uuid.uuid4().hex[:8]}",
            "p_name": "Device-Concurrent-Worker",
            "p_os_info": "Linux",
            "p_cpu_cores": 4,
            "p_ram_mb": 8192,
            "p_capabilities": ["browser:automa"],
            "p_metadata": {}
        })
        dev_b_id = dev_b["device_id"]
        dev_b_token = dev_b["device_token"]

        call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": dev_b_id,
            "p_device_token": dev_b_token
        })

        # 3. Device A releases while Device B currently holds the lock
        rel_a = call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"],
            "p_storage_path": f"profiles/{self.profile_id}/storage_dev_a.tar.zst",
            "p_storage_hash": "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069",
            "p_storage_size_bytes": 1048576,
            "p_metadata": {"author": "dev_a"}
        })
        self.assertTrue(rel_a.get("success"))
        self.assertTrue(rel_a.get("overwritten_concurrently"), "Device A should detect concurrent lock from Dev B")

        # 4. Device B releases afterwards (Last-Write-Wins)
        rel_b = call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": dev_b_id,
            "p_device_token": dev_b_token,
            "p_storage_path": f"profiles/{self.profile_id}/storage_dev_b_final.tar.zst",
            "p_storage_hash": "a1b2c3d4e5f60718293a4b5c6d7e8f901234567890abcdef1234567890abcdef",
            "p_storage_size_bytes": 2097152,
            "p_metadata": {"author": "dev_b_final"}
        })
        self.assertTrue(rel_b.get("success"))
        self.assertEqual(rel_b.get("status"), "idle")

    def test_04_storage_hash_length_constraint(self):
        """Rule 3.4: VARCHAR(64) column rejects storage_hash exceeding 64 characters."""
        call_cloud_rpc("acquire_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"]
        })

        # Prefix 'sha256:' makes length 71 chars (>64)
        invalid_hash = "sha256:7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
        res = call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"],
            "p_storage_hash": invalid_hash
        })
        self.assertEqual(res.get("error_http_code"), 400)
        self.assertIn("value too long for type character varying(64)", str(res.get("error_body")))

        # Release cleanly with valid 64-hex
        call_cloud_rpc("release_browser", {
            "p_browser_id": self.profile_id,
            "p_device_id": self.creds["device_id"],
            "p_device_token": self.creds["device_token"],
            "p_storage_hash": "7f83b1657ff1fc53b92dc18148a1d65dfc2d4b1fa3d677284addd200126d9069"
        })

if __name__ == "__main__":
    unittest.main()
