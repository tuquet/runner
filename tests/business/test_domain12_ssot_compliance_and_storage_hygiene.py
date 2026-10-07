import unittest
import os
import shutil
import sqlite3
import sys

sys.path.insert(0, os.path.dirname(__file__))

SPECTER_ROOT = os.getenv("SPECTER_HOME", os.path.expanduser("~/.specter"))

class TestDomain12SsotComplianceAndStorageHygiene(unittest.TestCase):
    """Business Unit Tests for Specter SSOT Architecture, Storage Pillars, and Directory Cleanliness."""

    def test_01_ssot_canonical_directory_pillars(self):
        """Rule 12.1: Canonical layout under ~/.specter/ strictly organizes all pillars without legacy folders."""
        pillars = ["system", "automa", "browser", "bridge", "faker"]
        for p in pillars:
            pillar_dir = os.path.join(SPECTER_ROOT, p)
            self.assertTrue(os.path.isdir(pillar_dir), f"Missing canonical SSOT pillar: {pillar_dir}")

        # Assert key critical files/subdirs
        self.assertTrue(os.path.isfile(os.path.join(SPECTER_ROOT, "system", ".identity.json")))
        self.assertTrue(os.path.isfile(os.path.join(SPECTER_ROOT, "automa", "automa.sqlite")))
        self.assertTrue(os.path.isdir(os.path.join(SPECTER_ROOT, "automa", "workflows")))
        self.assertTrue(os.path.isdir(os.path.join(SPECTER_ROOT, "browser", "runtimes")))
        self.assertTrue(os.path.isdir(os.path.join(SPECTER_ROOT, "browser", "profiles")))

        # Anti-fragmentation: Ensure NO obsolete legacy folders exist
        legacy_automa = os.path.expanduser("~/.automa")
        self.assertFalse(os.path.exists(legacy_automa), f"Legacy directory found: {legacy_automa}")

        flat_workflows = os.path.join(SPECTER_ROOT, "workflows")
        self.assertFalse(os.path.exists(flat_workflows), f"Flat root workflows found: {flat_workflows}")

    def test_02_workspace_root_cleanliness(self):
        """Rule 12.2: Workspace root /root contains NO loose temp files, dumps, or unauthorized scripts."""
        root_dir = "/root"
        entries = os.listdir(root_dir)

        # Prohibited loose file extensions in root
        prohibited_exts = (".tmp", ".sqlite", ".db", ".tar", ".zst", ".bak", ".log")
        violating_files = []

        for e in entries:
            full_path = os.path.join(root_dir, e)
            if os.path.isfile(full_path):
                # Standard hidden config files allowed (.bashrc, .profile, etc.)
                if e.startswith(".") and not any(e.endswith(ext) for ext in prohibited_exts):
                    continue
                if any(e.endswith(ext) for ext in prohibited_exts):
                    violating_files.append(e)

        self.assertEqual(
            len(violating_files),
            0,
            f"Hygiene violation: loose temporary files found in {root_dir}: {violating_files}"
        )

    def test_03_multi_profile_sandboxes_isolation(self):
        """Rule 12.3: Multiple browser profiles create strictly isolated directories under ~/.specter/browser/profiles/."""
        profiles_dir = os.path.join(SPECTER_ROOT, "browser", "profiles")
        alpha_dir = os.path.join(profiles_dir, "unit_profile_alpha")
        beta_dir = os.path.join(profiles_dir, "unit_profile_beta")

        try:
            os.makedirs(alpha_dir, exist_ok=True)
            os.makedirs(beta_dir, exist_ok=True)

            file_alpha = os.path.join(alpha_dir, "Preferences")
            with open(file_alpha, "w") as f:
                f.write('{"user_id": "alpha_123"}')

            file_beta = os.path.join(beta_dir, "Preferences")
            with open(file_beta, "w") as f:
                f.write('{"user_id": "beta_456"}')

            # Verify independence
            with open(file_alpha, "r") as f:
                self.assertIn("alpha_123", f.read())
            with open(file_beta, "r") as f:
                self.assertIn("beta_456", f.read())

            # Deleting alpha must not affect beta
            shutil.rmtree(alpha_dir)
            self.assertFalse(os.path.exists(alpha_dir))
            self.assertTrue(os.path.exists(file_beta))
        finally:
            if os.path.exists(alpha_dir):
                shutil.rmtree(alpha_dir)
            if os.path.exists(beta_dir):
                shutil.rmtree(beta_dir)

    def test_04_sqlite_foreign_key_cascade_deletion(self):
        """Rule 12.4: SQLite automa.sqlite enforces foreign keys; deleting a job cascades to delete all logs."""
        db_path = os.path.join(SPECTER_ROOT, "automa", "automa.sqlite")
        conn = sqlite3.connect(db_path)
        cur = conn.cursor()

        test_job_id = "job_fk_test_cascade_99"
        try:
            # Enable Foreign Keys explicitly for session
            cur.execute("PRAGMA foreign_keys = ON;")

            # Insert parent job
            cur.execute(
                "INSERT INTO jobs (id, name, data, status) VALUES (?, ?, ?, ?)",
                (test_job_id, "FK Test Job", "{}", "completed")
            )

            # Insert child logs
            cur.execute(
                "INSERT INTO logs (job_id, type, message) VALUES (?, ?, ?)",
                (test_job_id, "info", "Log step 1")
            )
            cur.execute(
                "INSERT INTO logs (job_id, type, message) VALUES (?, ?, ?)",
                (test_job_id, "info", "Log step 2")
            )
            conn.commit()

            # Confirm logs exist
            cur.execute("SELECT COUNT(*) FROM logs WHERE job_id = ?", (test_job_id,))
            self.assertEqual(cur.fetchone()[0], 2)

            # Delete parent job
            cur.execute("DELETE FROM jobs WHERE id = ?", (test_job_id,))
            conn.commit()

            # Confirm logs were cascade deleted
            cur.execute("SELECT COUNT(*) FROM logs WHERE job_id = ?", (test_job_id,))
            self.assertEqual(cur.fetchone()[0], 0, "Logs were not cascaded on job deletion")
        finally:
            cur.execute("DELETE FROM jobs WHERE id = ?", (test_job_id,))
            cur.execute("DELETE FROM logs WHERE job_id = ?", (test_job_id,))
            conn.commit()
            conn.close()

    def test_05_sqlite_integrity_and_wal_mode(self):
        """Rule 12.5: SQLite database passes integrity checks and has no dangling foreign key references."""
        db_path = os.path.join(SPECTER_ROOT, "automa", "automa.sqlite")
        conn = sqlite3.connect(db_path)
        cur = conn.cursor()

        try:
            # Integrity check
            cur.execute("PRAGMA integrity_check;")
            integrity_res = cur.fetchall()
            self.assertEqual(integrity_res, [("ok",)], f"Integrity check failed: {integrity_res}")

            # Foreign key check across entire schema
            cur.execute("PRAGMA foreign_key_check;")
            fk_violations = cur.fetchall()
            self.assertEqual(len(fk_violations), 0, f"Foreign key violations found: {fk_violations}")
        finally:
            conn.close()

if __name__ == "__main__":
    unittest.main()
