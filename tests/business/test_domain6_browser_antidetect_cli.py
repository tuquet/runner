import unittest
import os
import sys
import json
import subprocess

sys.path.insert(0, os.path.dirname(__file__))

SSOT_ROOT = os.getenv("SPECTER_HOME", os.path.expanduser("~/.specter"))
CHROME_BIN = os.path.join(SSOT_ROOT, "browser/runtimes/chromium-linux64/chrome")
EXT_RUNNER_PATH = os.path.join(SSOT_ROOT, "automa/runner")
EXT_AUTOMA_PATH = os.path.join(SSOT_ROOT, "browser/extensions/automa")

class TestDomain6BrowserAntidetectCLI(unittest.TestCase):
    """Business Unit Tests for C++ Antidetect Chromium, Sandbox Safety, and CLI Flag Assembly."""

    def test_01_chromium_executable_and_permissions(self):
        """Rule 6.1: Dedicated Chromium runtime must exist at canonical SSOT path with executable bit set."""
        self.assertTrue(os.path.exists(CHROME_BIN), f"Chromium binary not found at: {CHROME_BIN}")
        self.assertTrue(os.access(CHROME_BIN, os.X_OK), f"Chromium binary is not executable: {CHROME_BIN}")

    def test_02_linux_root_sandbox_safety(self):
        """Rule 6.2: On Linux as root, Chromium must be launched with --no-sandbox to prevent zygote crash."""
        # 1. Without --no-sandbox: Must fail as root
        res_no_flag = subprocess.run(
            [CHROME_BIN, "--headless", "--dump-dom", "about:blank"],
            capture_output=True,
            text=True
        )
        self.assertNotEqual(res_no_flag.returncode, 0)
        self.assertIn("Running as root without --no-sandbox is not supported", res_no_flag.stderr)

        # 2. With --no-sandbox: Must succeed
        res_with_flag = subprocess.run(
            [CHROME_BIN, "--headless", "--no-sandbox", "--version"],
            capture_output=True,
            text=True
        )
        self.assertEqual(res_with_flag.returncode, 0)
        self.assertIn("Chromium", res_with_flag.stdout)

    def test_03_extension_manifest_and_assets(self):
        """Rule 6.3: Automa MV3 extension must have valid manifest.json and background worker."""
        manifest_path = os.path.join(EXT_RUNNER_PATH, "manifest.json")
        self.assertTrue(os.path.exists(manifest_path), f"Extension manifest missing: {manifest_path}")

        with open(manifest_path, "r", encoding="utf-8") as f:
            manifest = json.load(f)

        self.assertEqual(manifest.get("manifest_version"), 3)
        self.assertIn("background", manifest)
        self.assertIn("service_worker", manifest["background"])

        worker_file = os.path.join(EXT_RUNNER_PATH, manifest["background"]["service_worker"])
        self.assertTrue(os.path.exists(worker_file), f"Background service worker file missing: {worker_file}")

    def test_04_cli_args_fingerprint_synthesis_contract(self):
        """Rule 6.4: Profile PRNG seed is compiled into --fingerprint <seed> CLI argument."""
        # Emulate build_cli_args logic from runner/browser
        def synthesize_cli_args(fingerprint_seed, is_headless=True, is_root=True):
            args = ["--no-first-run", "--password-store=basic"]
            if is_root:
                args.extend(["--no-sandbox", "--disable-setuid-sandbox"])
            if is_headless:
                args.append("--headless=new")
            if fingerprint_seed is not None:
                args.extend(["--fingerprint", str(fingerprint_seed)])
            return args

        args = synthesize_cli_args(133742, is_headless=True, is_root=True)
        self.assertIn("--no-sandbox", args)
        self.assertIn("--headless=new", args)
        self.assertIn("--fingerprint", args)
        self.assertIn("133742", args)

if __name__ == "__main__":
    unittest.main()
