"""WI079 — prove actual opt-in local verification invocation and limits.

This fixture exercises the real RepoPact verification CLI against a disposable
Git repository. It does NOT prove that Git, a remote PR, or canonical work-item
completion invokes this optional profile without separate enforcement.
"""

from __future__ import annotations

import io
import json
import os
import shutil
import subprocess
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

from repopact import self_audit, verify_cli


@unittest.skipUnless(shutil.which("git"), "Git is required")
class OptionalSelfAuditVerificationBoundaryTests(unittest.TestCase):
    def setUp(self) -> None:
        temp = tempfile.TemporaryDirectory(prefix="repopact-wi079-local-boundary-")
        self.addCleanup(temp.cleanup)
        self.parent = Path(temp.name).resolve()
        self.root = self.parent / "candidate-repo"
        self.root.mkdir()
        self.report_path = self.parent / "synthetic-self-review.json"
        (self.root / "governance").mkdir()
        config = {
            "$schema": "../schemas/verification-profile.schema.json",
            "version": 1,
            "default_profile": "optional-self-review",
            "execution_policy": {
                "local_primary": True,
                "hosted_ci_default": False,
                "hosted_cd_default": False,
            },
            "profiles": {
                "optional-self-review": {
                    "description": "WI079 disposable required freshness invocation",
                    "coverage": "host",
                    "steps": [
                        {
                            "id": "self-report-freshness",
                            "argv": [
                                "{python}",
                                "-m",
                                "repopact.self_audit",
                                "check",
                                "--root",
                                "{root}",
                                "--report",
                                str(self.report_path),
                            ],
                            "required": True,
                            "timeout_seconds": 120,
                        }
                    ],
                }
            },
        }
        (self.root / "governance" / "verification.json").write_text(
            json.dumps(config, indent=2) + "\n", encoding="utf-8"
        )
        (self.root / "alpha.txt").write_text("baseline\n", encoding="utf-8")
        self.git("init", "-q")
        self.git("config", "user.name", "WI079 Fixture")
        self.git("config", "user.email", "wi079@example.invalid")
        self.git("add", ".")
        self.git("commit", "-qm", "fixture baseline")
        self.base = self.git("rev-parse", "HEAD").strip()
        (self.root / "alpha.txt").write_text("changed after baseline\n", encoding="utf-8")

    @staticmethod
    def sanitized_env() -> dict[str, str]:
        return {
            **{key: value for key, value in os.environ.items() if not key.startswith("GIT_")},
            "GIT_TERMINAL_PROMPT": "0",
            "GIT_OPTIONAL_LOCKS": "0",
        }

    def git(self, *args: str) -> str:
        return subprocess.run(
            ["git", *args],
            cwd=self.root,
            env=self.sanitized_env(),
            capture_output=True,
            text=True,
            check=True,
            timeout=30,
        ).stdout

    def write_report(self, *, complete: bool = True, bogus_approval: bool = False) -> None:
        record = self_audit.report_template(self.root, self.base)
        if complete:
            record["review"].update(
                outcome="pass",
                actor="disposable-fixture",
                reviewed_paths=record["candidate"]["changed_paths"],
                limitations=["synthetic structural check; no real review performed"],
            )
        if bogus_approval:
            record["review"]["independent_approval"] = True
        self.report_path.write_text(json.dumps(record, indent=2), encoding="utf-8")

    def run_actual_verify_cli(self) -> tuple[int, dict]:
        # Child Python imports this exact working-source package, even when a
        # separate editable installation is absent on a developer host.
        source_root = str(Path(__file__).resolve().parents[1])
        env = self.sanitized_env()
        current_path = env.get("PYTHONPATH")
        env["PYTHONPATH"] = os.pathsep.join(
            part for part in (source_root, current_path) if part
        )
        stdout = io.StringIO()
        with patch.dict(os.environ, env, clear=True), redirect_stdout(stdout):
            exit_code = verify_cli.main(
                ["optional-self-review", "--root", str(self.root), "--json"]
            )
        return exit_code, json.loads(stdout.getvalue())

    def assert_denied_by_required_step(self, *, message: str | None = None) -> None:
        exit_code, record = self.run_actual_verify_cli()
        self.assertEqual(1, exit_code)
        self.assertEqual("fail", record["status"])
        # Coverage means the required step was attempted; it can still fail.
        self.assertTrue(record["coverage"]["satisfied"])
        self.assertEqual(1, record["coverage"]["required_total"])
        self.assertEqual(1, record["coverage"]["required_executed"])
        self.assertEqual(1, record["coverage"]["required_failed"])
        self.assertEqual("self-report-freshness", record["steps"][0]["id"])
        self.assertTrue(record["steps"][0]["required"])
        self.assertEqual("failed", record["steps"][0]["status"])
        # The helper emits 2 for an incomplete/stale report; the profile runner
        # maps a required nonzero subprocess to overall FAIL/exit 1.
        self.assertEqual(2, record["steps"][0]["exit_code"])
        if message:
            self.assertIn(message, record["steps"][0]["stdout"])

    def test_missing_report_is_actually_invoked_and_denied(self) -> None:
        self.assertFalse(self.report_path.exists())
        self.assert_denied_by_required_step(message="Review report missing")

    def test_incomplete_self_report_is_denied_by_actual_profile(self) -> None:
        self.write_report(complete=False)
        self.assert_denied_by_required_step(message="INCOMPLETE")

    def test_stale_report_after_untracked_change_is_denied(self) -> None:
        self.write_report()
        exit_code, record = self.run_actual_verify_cli()
        self.assertEqual(0, exit_code)
        self.assertEqual("pass", record["status"])
        (self.root / "new-untracked.txt").write_text("new\n", encoding="utf-8")
        self.assert_denied_by_required_step(message="STALE")

    def test_independent_approval_assertion_cannot_be_laundered(self) -> None:
        self.write_report(bogus_approval=True)
        self.assert_denied_by_required_step(message="approval assertion")

    def test_structurally_current_self_report_passes_only_local_profile(self) -> None:
        self.write_report()
        exit_code, record = self.run_actual_verify_cli()
        self.assertEqual(0, exit_code)
        self.assertEqual("pass", record["status"])
        self.assertTrue(record["coverage"]["satisfied"])
        self.assertTrue(record["coverage"]["all_declared_executed"])
        self.assertEqual(1, record["coverage"]["required_passed"])
        self.assertEqual("passed", record["steps"][0]["status"])
        self.assertEqual(0, record["steps"][0]["exit_code"])
        self.assertIn("self-report-current", record["steps"][0]["stdout"])
        self.assertIn("instruction-only", record["steps"][0]["stdout"])
        # No platform or protected completion authority is inferred here.
        self.assertEqual("host", record["coverage"]["mode"])

    def test_git_completion_marker_without_profile_is_not_intercepted(self) -> None:
        # Negative boundary: plain Git can commit a file CLAIMING completed
        # status without invoking WI079. This is deliberately NOT an assertion
        # that RepoPact's canonical validator accepted a completed work item.
        completed = self.root / "work" / "completed" / "999-fixture-unprotected"
        completed.mkdir(parents=True)
        (completed / "work-item.json").write_text(
            json.dumps({"id": "999", "status": "completed"}) + "\n",
            encoding="utf-8",
        )
        self.assertFalse(self.report_path.exists())
        self.git("add", "work/completed/999-fixture-unprotected/work-item.json")
        self.git("commit", "-qm", "Git can commit a completion marker without WI079")
        self.assertNotEqual(self.base, self.git("rev-parse", "HEAD").strip())
        self.assert_denied_by_required_step(message="Review report missing")


if __name__ == "__main__":
    unittest.main()
