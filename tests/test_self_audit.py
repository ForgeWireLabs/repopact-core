"""Exploratory WI079 candidate-snapshot and self-report tests.

No canonical RepoPact engine needed for these local unit tests.
"""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

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
        env = {
            **{key: value for key, value in os.environ.items() if not key.upper().startswith("GIT_")},
            "GIT_TERMINAL_PROMPT": "0",
            "GIT_OPTIONAL_LOCKS": "0",
        }
        return subprocess.check_output(
            ["git", *args], cwd=self.root, env=env, text=True
        )

    def candidate(self) -> dict:
        return self_audit.snapshot(self.root, self.base)

    def test_inherited_git_overrides_cannot_redirect_candidate(self):
        baseline = self.candidate()
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(outcome="pass", actor="test-agent", reviewed_paths=[])
        conflicting = {
            "GIT_DIR": str(self.root / "missing-git-directory"),
            "GIT_WORK_TREE": str(self.root / "redirected-worktree"),
            "GIT_INDEX_FILE": str(self.root / "redirected.index"),
            "GIT_CONFIG_COUNT": "1",
            "GIT_CONFIG_KEY_0": "core.worktree",
            "GIT_CONFIG_VALUE_0": str(self.root / "redirected-worktree"),
        }
        original_git_env = {
            key: value for key, value in os.environ.items() if key.upper().startswith("GIT_")
        }
        with patch.dict(os.environ, conflicting):
            self.assertEqual(baseline, self.candidate())
            result = self_audit.inspect_report(self.root, report)
        self.assertEqual(
            original_git_env,
            {key: value for key, value in os.environ.items() if key.upper().startswith("GIT_")},
        )

        # Keep a regression check for the exact child-process operation used
        # when later tests materialize fixtures from this checkout.
        source_root = Path(__file__).resolve().parents[1]
        tracked = subprocess.run(
            ["git", "ls-files", "--cached", "-z"],
            cwd=source_root,
            check=True,
            capture_output=True,
            env={
                **{key: value for key, value in os.environ.items() if not key.upper().startswith("GIT_")},
                "GIT_TERMINAL_PROMPT": "0",
                "GIT_OPTIONAL_LOCKS": "0",
            },
        )
        self.assertTrue(tracked.stdout)
        self.assertEqual("self-report-current", result["result"])
        self.assertEqual("instruction-only", result["assurance"])

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

    def test_net_reversal_does_not_hide_review_layers(self):
        path = self.root / "alpha.txt"
        for layer in ("staged", "committed-staged-reversal", "committed-unstaged-reversal"):
            with self.subTest(layer=layer):
                self.git("reset", "--hard", self.base)
                path.write_text("intermediate change\n", encoding="utf-8")
                self.git("add", "alpha.txt")
                if layer.startswith("committed"):
                    self.git("commit", "-qm", "intermediate change")
                path.write_text("alpha\n", encoding="utf-8")
                if layer == "committed-staged-reversal":
                    self.git("add", "alpha.txt")

                # The old net-diff algorithm sees no changes at all.
                self.assertEqual("", self.git("diff", "--name-only", self.base, "--"))
                record = self_audit.report_template(self.root, self.base)
                self.assertEqual(["alpha.txt"], record["candidate"]["changed_paths"])
                self.assertEqual(["alpha.txt"], [
                    state["path"] for state in record["candidate"]["content_states"]
                ])
                record["review"].update(outcome="pass", actor="agent", reviewed_paths=[])
                with self.assertRaisesRegex(self_audit.SelfAuditError, "reviewed paths"):
                    self_audit.inspect_report(self.root, record)
                record["review"]["reviewed_paths"] = ["alpha.txt"]
                result = self_audit.inspect_report(self.root, record)
                self.assertEqual("self-report-current", result["result"])
                self.assertEqual("instruction-only", result["assurance"])

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

    def test_repaired_finding_needs_fresh_snapshot_and_remains_self_reported(self):
        (self.root / "alpha.txt").write_text("known defect\n", encoding="utf-8")
        before = self_audit.report_template(self.root, self.base)
        before["review"].update(
            outcome="pass",
            actor="agent",
            reviewed_paths=["alpha.txt"],
            findings=[{
                "path": "alpha.txt",
                "status": "open_blocker",
                "scenario": "known defect in alpha.txt",
            }],
        )
        with self.assertRaisesRegex(self_audit.SelfAuditError, "unresolved"):
            self_audit.inspect_report(self.root, before)

        # A claimed repair cannot reuse inspection of the previous bytes.
        (self.root / "alpha.txt").write_text("corrected\n", encoding="utf-8")
        with self.assertRaisesRegex(self_audit.SelfAuditError, "STALE"):
            self_audit.inspect_report(self.root, before)

        after = self_audit.report_template(self.root, self.base)
        after["review"].update(
            outcome="pass",
            actor="agent",
            reviewed_paths=["alpha.txt"],
            findings=[{
                "path": "alpha.txt",
                "status": "fixed",
                "scenario": "corrected the previously known defect in alpha.txt",
            }],
        )
        result = self_audit.inspect_report(self.root, after)
        self.assertEqual("self-report-current", result["result"])
        self.assertEqual("instruction-only", result["assurance"])
        self.assertNotEqual(
            before["candidate"]["fingerprint"], after["candidate"]["fingerprint"]
        )

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
        (self.root / "alpha.txt").write_text("modified\n", encoding="utf-8")
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

    def test_finding_cannot_assert_independent_approval(self):
        (self.root / "alpha.txt").write_text("changed\n", encoding="utf-8")
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(
            outcome="pass", actor="agent", reviewed_paths=["alpha.txt"],
            findings=[{
                "path": "alpha.txt", "status": "nonblocking",
                "scenario": "example", "independent_approval": True,
            }],
        )
        with self.assertRaisesRegex(self_audit.SelfAuditError, "finding fields"):
            self_audit.inspect_report(self.root, report)

    def test_cli_rejects_unhashable_finding_status_without_traceback(self):
        from contextlib import redirect_stdout
        from io import StringIO

        (self.root / "alpha.txt").write_text("modified\n", encoding="utf-8")
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(
            outcome="pass", actor="agent", reviewed_paths=["alpha.txt"],
            findings=[{"path": "alpha.txt", "status": [], "scenario": "example"}],
        )
        path = Path(self.temp.name) / "review.json"
        for invalid in ([], {}, ["fixed"]):
            with self.subTest(status=invalid):
                report["review"]["findings"][0]["status"] = invalid
                path.write_text(json.dumps(report), encoding="utf-8")
                output = StringIO()
                with redirect_stdout(output):
                    exit_code = self_audit.main([
                        "check", "--root", str(self.root), "--report", str(path),
                    ])
                self.assertEqual(1, exit_code)
                result = json.loads(output.getvalue())
                self.assertEqual("fail", result["result"])
                self.assertIn("unresolved or invalid finding", result["message"])
                self.assertNotIn("assurance", result)

    def test_cli_rejects_non_object_report_without_traceback(self):
        from contextlib import redirect_stdout
        from io import StringIO

        path = Path(self.temp.name) / "review.json"
        path.write_text("[]", encoding="utf-8")
        output = StringIO()
        with redirect_stdout(output):
            result = self_audit.main([
                "check", "--root", str(self.root), "--report", str(path),
            ])
        self.assertEqual(2, result)
        self.assertEqual("incomplete", json.loads(output.getvalue())["result"])

    def test_cli_rejects_missing_and_malformed_reports_cleanly(self):
        from contextlib import redirect_stdout
        from io import StringIO

        missing = Path(self.temp.name) / "missing.json"
        malformed = Path(self.temp.name) / "malformed.json"
        malformed.write_text("{invalid", encoding="utf-8")
        for path in (missing, malformed):
            with self.subTest(path=path.name):
                output = StringIO()
                with redirect_stdout(output):
                    result = self_audit.main([
                        "check", "--root", str(self.root), "--report", str(path),
                    ])
                self.assertEqual(2, result)
                self.assertEqual("incomplete", json.loads(output.getvalue())["result"])

    def test_cli_accepts_utf8_bom_report(self):
        from contextlib import redirect_stdout
        from io import StringIO

        report = self_audit.report_template(self.root, self.base)
        path = Path(self.temp.name) / "review-bom.json"
        path.write_bytes(b"\xef\xbb\xbf" + json.dumps(report).encode("utf-8"))
        output = StringIO()
        with redirect_stdout(output):
            result = self_audit.main([
                "check", "--root", str(self.root), "--report", str(path),
            ])
        self.assertEqual(2, result)
        self.assertIn("no passing self-review claim", json.loads(output.getvalue())["message"])

    def test_cli_rejects_oversized_report(self):
        from contextlib import redirect_stdout
        from io import StringIO
        from unittest.mock import patch

        path = Path(self.temp.name) / "review.json"
        path.write_text(" " * 17, encoding="utf-8")
        output = StringIO()
        with patch.object(self_audit, "_MAX_REPORT_BYTES", 16):
            with redirect_stdout(output):
                result = self_audit.main([
                    "check", "--root", str(self.root), "--report", str(path),
                ])
        self.assertEqual(2, result)
        self.assertIn("bounded size", json.loads(output.getvalue())["message"])

    def test_symlink_parent_cycle_fails_as_incomplete(self):
        cycle = self.root / "cycle"
        try:
            cycle.symlink_to("cycle", target_is_directory=True)
        except (OSError, NotImplementedError):
            self.skipTest("symlink creation unavailable")
        with self.assertRaisesRegex(self_audit.SelfAuditError, "parent inaccessible"):
            self_audit._file_state(self.root, "cycle/child.txt")

    def test_invalid_limitations_declared_incomplete(self):
        report = self_audit.report_template(self.root, self.base)
        report["review"].update(
            outcome="pass", actor="agent", reviewed_paths=[],
            limitations="none",
        )
        with self.assertRaisesRegex(self_audit.SelfAuditError, "limitation"):
            self_audit.inspect_report(self.root, report)

    def test_linked_worktree_uses_distinct_identity(self):
        linked = Path(self.temp.name) / "other-worktree"
        self.git("worktree", "add", "--detach", str(linked), self.base)
        self.addCleanup(lambda: self.git("worktree", "remove", "--force", str(linked)))
        other = self_audit.snapshot(linked, self.base)
        original = self.candidate()
        self.assertEqual(original["head_commit"], other["head_commit"])
        self.assertNotEqual(original["worktree_key"], other["worktree_key"])
        self.assertNotEqual(original["fingerprint"], other["fingerprint"])

    def test_git_rename_exposes_both_source_and_destination(self):
        self.git("mv", "alpha.txt", "renamed.txt")
        candidate = self.candidate()
        self.assertIn("alpha.txt", candidate["changed_paths"])
        self.assertIn("renamed.txt", candidate["changed_paths"])

    def test_git_ignored_files_are_explicitly_not_covered(self):
        (self.root / ".gitignore").write_text("ignored.bin\n", encoding="utf-8")
        (self.root / "ignored.bin").write_text("not part of Git candidate", encoding="utf-8")
        candidate = self.candidate()
        self.assertIn(".gitignore", candidate["changed_paths"])
        self.assertNotIn("ignored.bin", candidate["changed_paths"])
        self.assertFalse(candidate["ignored_files_covered"])

    def test_git_stdout_budget_fails_instead_of_truncating(self):
        from unittest.mock import patch
        (self.root / "untracked-file.txt").write_text("new\n", encoding="utf-8")
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

    def test_symlink_parent_escape_is_rejected(self):
        outside = Path(self.temp.name) / "outside"
        outside.mkdir()
        (outside / "data.txt").write_text("unrelated", encoding="utf-8")
        link = self.root / "linkdir"
        try:
            link.symlink_to(outside, target_is_directory=True)
        except (OSError, NotImplementedError):
            self.skipTest("directory symlink creation unavailable")
        with self.assertRaisesRegex(self_audit.SelfAuditError, "escapes worktree"):
            self_audit._file_state(self.root, "linkdir/data.txt")

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
        (self.root / "alpha.txt").write_text("modified\n", encoding="utf-8")
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
