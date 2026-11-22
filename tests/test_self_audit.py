"""Exploratory WI079 candidate-snapshot and self-report tests.

No canonical RepoPact engine needed for these local unit tests.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from repopact import self_audit


@unittest.skipUnless(shutil.which("git"), "Git required")
class CandidateAuditTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / "repo"
        self.root.mkdir()
        self.git("init", "-q")
        self.git("config", "user.email", "unit-test@example.invalid")
        self.git("config", "user.name", "Unit Test")
        (self.root / "alpha.txt").write_text("alpha\n", encoding="utf-8")
        (self.root / "retire.txt").write_text("old\n", encoding="utf-8")
        self.git("add", ".")
        self.git("commit", "-qm", "baseline")
        self.base = self.git("rev-parse", "HEAD").strip()

    def git(self, *args: str) -> str:
        env = {**os.environ, "GIT_TERMINAL_PROMPT": "0"}
        return subprocess.check_output(
            ["git", *args], cwd=self.root, env=env, text=True
        )

    def candidate(self) -> dict:
        return self_audit.snapshot(self.root, self.base)

    def test_empty_working_tree_is_bound_to_head_and_base(self):
        candidate = self.candidate()
        self.assertEqual(self.base, candidate["base_commit"])
        self.assertEqual(self.base, candidate["head_commit"])
        self.assertEqual([], candidate["changed_paths"])
        self.assertEqual(64, len(candidate["fingerprint"]))
        self.assertFalse(candidate["ignored_files_covered"])

    def test_covers_staged_unstaged_untracked_and_deleted(self):
        (self.root / "alpha.txt").write_text("modified\n", encoding="utf-8")
        (self.root / "staged.txt").write_text("staged\n", encoding="utf-8")
        (self.root / "untracked.txt").write_text("untracked\n", encoding="utf-8")
        (self.root / "retire.txt").unlink()
        self.git("add", "staged.txt")
        candidate = self.candidate()
        self.assertEqual(
            {"alpha.txt", "staged.txt", "untracked.txt", "retire.txt"},
            set(candidate["changed_paths"]),
        )
        removed = next(x for x in candidate["content_states"] if x["path"] == "retire.txt")
        self.assertEqual("deleted", removed["kind"])

    def test_committed_change_updates_head_and_diff(self):
        (self.root / "alpha.txt").write_text("commit me\n", encoding="utf-8")
        self.git("add", "alpha.txt")
        self.git("commit", "-qm", "update")
        candidate = self.candidate()
        self.assertNotEqual(self.base, candidate["head_commit"])
        self.assertIn("alpha.txt", candidate["changed_paths"])

    def test_dirty_content_and_index_drift_invalidate_report(self):
        (self.root / "alpha.txt").write_text("one\n", encoding="utf-8")
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(
            outcome="pass", actor="coding-agent",
            reviewed_paths=report["candidate"]["changed_paths"],
        )
        result = self_audit.inspect_report(self.root, report)
        self.assertEqual("self-report-current", result["result"])
        (self.root / "alpha.txt").write_text("two\n", encoding="utf-8")
        with self.assertRaisesRegex(self_audit.SelfAuditError, "STALE"):
            self_audit.inspect_report(self.root, report)

    def test_untracked_addition_invalidates_prior_report(self):
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(outcome="pass", actor="agent", reviewed_paths=[])
        (self.root / "new.txt").write_text("new\n", encoding="utf-8")
        with self.assertRaisesRegex(self_audit.SelfAuditError, "STALE"):
            self_audit.inspect_report(self.root, report)

    def test_staged_index_drift_invalidates_prior_report(self):
        (self.root / "alpha.txt").write_text("alpha staged\n", encoding="utf-8")
        before = self.candidate()
        self.git("add", "alpha.txt")
        after = self.candidate()
        self.assertNotEqual(before["index_sha256"], after["index_sha256"])
        self.assertNotEqual(before["fingerprint"], after["fingerprint"])

    def test_report_requires_exhaustive_path_declaration(self):
        (self.root / "new.txt").write_text("new\n", encoding="utf-8")
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(outcome="pass", actor="agent", reviewed_paths=[])
        with self.assertRaisesRegex(self_audit.SelfAuditError, "reviewed paths"):
            self_audit.inspect_report(self.root, report)

    def test_unresolved_findings_fail_and_fixed_findings_need_scenario(self):
        (self.root / "alpha.txt").write_text("modified\n", encoding="utf-8")
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(
            outcome="pass", actor="agent",
            reviewed_paths=["alpha.txt"],
            findings=[{"path": "alpha.txt", "status": "open_blocker", "scenario": "broken"}],
        )
        with self.assertRaisesRegex(self_audit.SelfAuditError, "unresolved"):
            self_audit.inspect_report(self.root, report)
        report["review"]["findings"][0]["status"] = "fixed"
        report["review"]["findings"][0]["scenario"] = ""
        with self.assertRaisesRegex(self_audit.SelfAuditError, "scenario"):
            self_audit.inspect_report(self.root, report)

    def test_malformed_review_coverage_fails_cleanly(self):
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(
            outcome="pass", actor="agent", reviewed_paths=[{"not": "a path"}],
        )
        with self.assertRaisesRegex(self_audit.SelfAuditError, "invalid reviewed-path"):
            self_audit.inspect_report(self.root, report)

    def test_report_without_attestation_does_not_pass(self):
        report = self_audit.report_template(self.root, self.base)
        with self.assertRaisesRegex(self_audit.SelfAuditError, "no passing"):
            self_audit.inspect_report(self.root, report)

    def test_report_rejects_duplicate_path_claims(self):
        (self.root / "alpha.txt").write_text("modified\\n", encoding="utf-8")
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(
            outcome="pass", actor="agent", reviewed_paths=["alpha.txt", "alpha.txt"],
        )
        with self.assertRaisesRegex(self_audit.SelfAuditError, "invalid reviewed-path"):
            self_audit.inspect_report(self.root, report)

    def test_self_report_cannot_assert_independent_approval(self):
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(outcome="pass", actor="agent", reviewed_paths=[])
        report["review"]["independent_approval"] = True
        with self.assertRaisesRegex(self_audit.SelfAuditError, "approval"):
            self_audit.inspect_report(self.root, report)
        del report["review"]["independent_approval"]
        report["approved"] = True
        with self.assertRaisesRegex(self_audit.SelfAuditError, "approval"):
            self_audit.inspect_report(self.root, report)

    def test_invalid_limitations_declared_incomplete(self):
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(
            outcome="pass", actor="agent", reviewed_paths=[],
            limitations="none",
        )
        with self.assertRaisesRegex(self_audit.SelfAuditError, "limitation"):
            self_audit.inspect_report(self.root, report)

    def test_git_rename_exposes_both_source_and_destination(self):
        self.git("mv", "alpha.txt", "renamed.txt")
        candidate = self.candidate()
        self.assertIn("alpha.txt", candidate["changed_paths"])
        self.assertIn("renamed.txt", candidate["changed_paths"])

    def test_git_ignored_files_are_explicitly_not_covered(self):
        (self.root / ".gitignore").write_text("ignored.bin\\n", encoding="utf-8")
        (self.root / "ignored.bin").write_text("not part of Git candidate", encoding="utf-8")
        candidate = self.candidate()
        self.assertIn(".gitignore", candidate["changed_paths"])
        self.assertNotIn("ignored.bin", candidate["changed_paths"])
        self.assertFalse(candidate["ignored_files_covered"])

    def test_git_stdout_budget_fails_instead_of_truncating(self):
        from unittest.mock import patch
        (self.root / "untracked-file.txt").write_text("new\\n", encoding="utf-8")
        with patch.object(self_audit, "_MAX_COMMAND_OUTPUT", 4):
            with self.assertRaisesRegex(self_audit.SelfAuditError, "exceeded bounded capture"):
                self_audit._git(self.root, "status", "--porcelain=v1", "--untracked-files=all")

    def test_non_git_root_and_nested_root_fail(self):
        nested = self.root / "nested"
        nested.mkdir()
        with self.assertRaisesRegex(self_audit.SelfAuditError, "top-level"):
            self_audit.snapshot(nested, self.base)
        with self.assertRaises(self_audit.SelfAuditError):
            self_audit.snapshot(Path(self.temp.name), self.base)

    def test_oversized_untracked_file_does_not_silently_skip(self):
        path = self.root / "large.dat"
        with path.open("wb") as handle:
            handle.truncate(self_audit._MAX_CONTENT_BYTES + 1)
        with self.assertRaisesRegex(self_audit.SelfAuditError, "exceeds bounded capture"):
            self.candidate()

    def test_symlink_target_is_hashed_not_followed(self):
        link = self.root / "link"
        try:
            link.symlink_to("nonexistent-target")
        except (OSError, NotImplementedError):
            self.skipTest("symlink creation unavailable")
        entry = next(x for x in self.candidate()["content_states"] if x["path"] == "link")
        self.assertEqual("symlink", entry["kind"])

    def test_top_level_cli_exposes_experimental_capture(self):
        from contextlib import redirect_stdout
        from io import StringIO
        from repopact import cli

        output = StringIO()
        with redirect_stdout(output):
            result = cli.main([
                "self-audit", "capture", "--root", str(self.root),
                "--base", self.base,
            ])
        self.assertEqual(0, result)
        record = json.loads(output.getvalue())
        self.assertEqual("incomplete", record["review"]["outcome"])
        self.assertEqual(self.base, record["candidate"]["head_commit"])

    def test_cli_rejects_report_inside_worktree(self):
        report = self_audit.report_template(self.root, self.base)
        path = self.root / "review.json"
        path.write_text(json.dumps(report), encoding="utf-8")
        self.assertEqual(
            2, self_audit.main([
                "check", "--root", str(self.root), "--report", str(path)
            ]),
        )

    def test_cli_reports_blocking_finding_as_failure(self):
        (self.root / "alpha.txt").write_text("modified\\n", encoding="utf-8")
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(
            outcome="pass", actor="agent", reviewed_paths=["alpha.txt"],
            findings=[{"path": "alpha.txt", "status": "open_blocker", "scenario": "unsafe"}],
        )
        path = Path(self.temp.name) / "review.json"
        path.write_text(json.dumps(report), encoding="utf-8")
        self.assertEqual(1, self_audit.main([
            "check", "--root", str(self.root), "--report", str(path),
        ]))

    def test_cli_reports_current_self_claim_without_pretending_independence(self):
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(outcome="pass", actor="agent", reviewed_paths=[])
        path = Path(self.temp.name) / "review.json"
        path.write_text(json.dumps(report), encoding="utf-8")
        self.assertEqual(0, self_audit.main([
            "check", "--root", str(self.root), "--report", str(path),
        ]))


if __name__ == "__main__":
    unittest.main()
