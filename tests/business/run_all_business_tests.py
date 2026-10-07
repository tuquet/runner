#!/usr/bin/env python3
"""
Master Business Unit Test Runner for Tuquet Runner & Cloud Architecture.
Runs all business test suites across Domain 1 through Domain 6 with a clean scoreboard.
"""

import os
import sys
import time
import unittest

def main():
    test_dir = os.path.dirname(os.path.abspath(__file__))
    sys.path.insert(0, test_dir)

    print("=" * 70)
    print(" 🚀 TUQUET BUSINESS-LEVEL UNIT TEST SUITE (13 DOMAINS)")
    print("=" * 70)

    domains = [
        ("Domain 1: Fleet Enrollment & Identity", "test_domain1_device_enrollment.py"),
        ("Domain 2: Browser Profile & Advisory Warning", "test_domain2_profile_acquisition.py"),
        ("Domain 3: Profile Release & Overwrite", "test_domain3_profile_release_overwrite.py"),
        ("Domain 4: Automa Workflow Vault & DAG", "test_domain4_workflow_vault.py"),
        ("Domain 5: Runner Job Lifecycle & Telemetry", "test_domain5_job_lifecycle.py"),
        ("Domain 6: Browser Antidetect & CLI Synthesis", "test_domain6_browser_antidetect_cli.py"),
        ("Domain 7: Antidetect Bot Bypass Verification", "test_domain7_bot_detection_bypass.py"),
        ("Domain 8: Job Control & Cancellation Extremes", "test_domain8_job_control_and_cancellation.py"),
        ("Domain 9: Cloud Boundaries & Security Edge Cases", "test_domain9_cloud_edge_cases_and_boundaries.py"),
        ("Domain 10: DAG Integrity & Process Resilience", "test_domain10_dag_validation_and_process_resilience.py"),
        ("Domain 11: Workflow Fault Injection & Vault Recovery", "test_domain11_workflow_fault_injection.py"),
        ("Domain 12: SSOT Compliance & Storage Hygiene", "test_domain12_ssot_compliance_and_storage_hygiene.py"),
        ("Domain 13: Cloud Multi-Tenant & Stress Boundaries", "test_domain13_cloud_multi_tenant_and_stress.py"),
    ]

    total_tests = 0
    total_failures = 0
    total_errors = 0
    start_all = time.time()
    scoreboard = []

    for name, filename in domains:
        module_name = filename[:-3]
        print(f"\n▶ Running {name} ({filename})...")
        loader = unittest.TestLoader()
        suite = loader.loadTestsFromName(module_name)
        runner = unittest.TextTestRunner(verbosity=1)
        start_t = time.time()
        result = runner.run(suite)
        elapsed = time.time() - start_t

        num_tests = result.testsRun
        fails = len(result.failures)
        errs = len(result.errors)
        status = "PASSED" if (fails == 0 and errs == 0) else "FAILED"

        total_tests += num_tests
        total_failures += fails
        total_errors += errs

        scoreboard.append((name, num_tests, fails, errs, elapsed, status))

    total_elapsed = time.time() - start_all

    print("\n" + "=" * 70)
    print(" 📊 BUSINESS UNIT TEST SCOREBOARD REPORT")
    print("=" * 70)
    print(f"{'DOMAIN':<45} | {'TESTS':<5} | {'TIME':<6} | {'STATUS'}")
    print("-" * 70)
    for name, num_tests, fails, errs, elapsed, status in scoreboard:
        status_icon = "✔ PASS" if status == "PASSED" else "✖ FAIL"
        print(f"{name:<45} | {num_tests:<5} | {elapsed:.2f}s  | {status_icon}")
    print("=" * 70)
    print(f" TOTAL: {total_tests} Tests | Failures: {total_failures} | Errors: {total_errors} | Time: {total_elapsed:.2f}s")

    if total_failures == 0 and total_errors == 0:
        print(" 🎉 ALL BUSINESS UNIT TESTS COMPLETED SUCCESSFULLY (100% GREEN)!")
        print("=" * 70)
        sys.exit(0)
    else:
        print(" ❌ SOME UNIT TESTS FAILED. CHECK LOGS ABOVE.")
        print("=" * 70)
        sys.exit(1)

if __name__ == "__main__":
    main()
