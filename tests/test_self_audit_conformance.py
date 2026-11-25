"""WI079 reference vectors and their conformance-command integration."""
from __future__ import annotations

import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest.mock import patch

from repopact import run_conformance, self_audit_conformance


class SelfAuditConformanceTests(unittest.TestCase):
    def test_reference_corpus_has_all_ten_expected_results(self) -> None:
        expected = {
            "WI079-current-report",
            "WI079-missing-report",
            "WI079-incomplete-report",
            "WI079-stale-report",
            "WI079-omitted-untracked",
            "WI079-root-approval",
            "WI079-review-approval",
            "WI079-finding-approval",
            "WI079-blocking-finding",
            "WI079-reported-fixed",
        }
        results = self_audit_conformance.run_self_audit_corpus()
        self.assertEqual(len(expected), len(results))
        self.assertEqual(expected, {case_id for case_id, _, _ in results})
        self.assertEqual([], [row for row in results if not row[1]])

    def test_reference_corpus_does_not_pass_when_git_is_unavailable(self) -> None:
        with patch.object(
            self_audit_conformance, "_git", side_effect=FileNotFoundError("git")
        ):
            results = self_audit_conformance.run_self_audit_corpus()
        self.assertEqual(10, len(results))
        self.assertTrue(all(not passed for _, passed, _ in results))

    def test_reference_vector_failure_blocks_combined_conformance(self) -> None:
        # Inject a failing WI079 outcome to test the command's aggregation,
        # not to claim that this mock ran a canonical Rust validator.
        provenance = {
            "engine": {"status": "complete"},
            "conformance": {"passed": 1, "failed": 0},
        }
        canonical = [run_conformance.CaseResult("canonical-probe", True, "fixture")]
        admission = [("admission-probe", True, "fixture")]
        vectors = [
            ("WI079-current-report", True, "fixture"),
            ("WI079-review-approval", False, "unexpected approval"),
        ]
        output = io.StringIO()
        with (
            patch("sys.argv", ["run_conformance"]),
            patch.object(run_conformance, "run_canonical_suite",
                         return_value=(canonical, provenance)),
            patch("repopact.admission_conformance.run_admission_corpus",
                  return_value=admission),
            patch.object(self_audit_conformance, "run_self_audit_corpus",
                         return_value=vectors),
            patch.object(run_conformance, "_write_provenance",
                         return_value=Path("fixture.json")) as write,
            redirect_stdout(output),
        ):
            exit_code = run_conformance.main()
        self.assertEqual(1, exit_code)
        self.assertEqual("failed", provenance["result"])
        self.assertEqual(1, provenance["exit_code"])
        summary = provenance["conformance"]["wi079_self_audit_corpus"]
        self.assertEqual("python-reference-self-audit", summary["implementation"])
        self.assertEqual("instruction-only", summary["assurance"])
        self.assertEqual(1, summary["passed"])
        self.assertEqual(1, summary["failed"])
        self.assertEqual("FAIL", summary["cases"][1]["status"])
        self.assertIn("WI079 Python self-audit reference corpus: 1/2", output.getvalue())
        write.assert_called_once_with(provenance, None)


if __name__ == "__main__":
    unittest.main()
