"""WI079 reference self-audit report corpus, separate from Rust governance.

The vectors use fresh disposable Git worktrees and the real Python helper CLI.
They test structural claims, not reviewer independence or protected admission.
"""
from __future__ import annotations

import io
import json
import os
import subprocess
import tempfile
from contextlib import redirect_stdout
from pathlib import Path

from . import self_audit

# Fixed expected outputs are independent of the helper implementation.
# Each vector gets a new repository and a fresh synthetic report.
VECTORS = (
    ("WI079-current-report", 0, "self-report-current", ""),
    ("WI079-missing-report", 2, "incomplete", "Review report missing"),
    ("WI079-incomplete-report", 2, "incomplete", "no passing self-review claim"),
    ("WI079-stale-report", 2, "incomplete", "STALE"),
    ("WI079-omitted-untracked", 2, "incomplete", "reviewed paths do not match"),
    ("WI079-root-approval", 2, "incomplete", "approval authority"),
    ("WI079-review-approval", 2, "incomplete", "approval assertion"),
    ("WI079-finding-approval", 2, "incomplete", "approval assertions"),
    ("WI079-blocking-finding", 1, "fail", "unresolved or invalid finding"),
    ("WI079-reported-fixed", 0, "self-report-current", ""),
)


def _git(root: Path, *args: str) -> str:
    env = {
        **{key: value for key, value in os.environ.items() if not key.startswith("GIT_")},
        "GIT_TERMINAL_PROMPT": "0",
        "GIT_OPTIONAL_LOCKS": "0",
    }
    return subprocess.run(
        ["git", *args],
        cwd=root,
        env=env,
        capture_output=True,
        text=True,
        check=True,
        timeout=30,
    ).stdout.strip()


def _check_vector(case_id: str, expected_exit: int, expected_result: str,
                  expected_message: str) -> tuple[str, bool, str]:
    try:
        with tempfile.TemporaryDirectory(prefix="repopact-wi079-vector-") as tmp:
            parent = Path(tmp).resolve()
            root = parent / "repo"
            root.mkdir()
            report_path = parent / "report.json"
            _git(root, "init", "-q")
            _git(root, "config", "user.name", "WI079 Corpus")
            _git(root, "config", "user.email", "wi079@example.invalid")
            (root / "alpha.txt").write_text("baseline\n", encoding="utf-8")
            _git(root, "add", "alpha.txt")
            _git(root, "commit", "-qm", "fixture baseline")
            base = _git(root, "rev-parse", "HEAD")
            (root / "alpha.txt").write_text("changed\n", encoding="utf-8")
            if case_id == "WI079-omitted-untracked":
                (root / "untracked.txt").write_text("untracked\n", encoding="utf-8")

            capture = io.StringIO()
            with redirect_stdout(capture):
                capture_exit = self_audit.main([
                    "capture", "--root", str(root), "--base", base,
                ])
            if capture_exit != 0:
                return case_id, False, "fixture capture failed"
            report = json.loads(capture.getvalue())
            review = report["review"]
            if case_id != "WI079-incomplete-report":
                review.update(
                    outcome="pass",
                    actor="synthetic-corpus",
                    reviewed_paths=list(report["candidate"]["changed_paths"]),
                    limitations=["synthetic claim; no independent inspection"],
                )

            if case_id == "WI079-stale-report":
                (root / "alpha.txt").write_text("changed again\n", encoding="utf-8")
            elif case_id == "WI079-omitted-untracked":
                review["reviewed_paths"].remove("untracked.txt")
            elif case_id == "WI079-root-approval":
                report["independent_approval"] = True
            elif case_id == "WI079-review-approval":
                review["independent_approval"] = True
            elif case_id in {"WI079-finding-approval", "WI079-blocking-finding", "WI079-reported-fixed"}:
                finding = {
                    "path": "alpha.txt",
                    "status": "open_blocker" if case_id == "WI079-blocking-finding" else "fixed",
                    "scenario": "synthetic fixture finding",
                }
                if case_id == "WI079-finding-approval":
                    finding["independent_approval"] = True
                review["findings"] = [finding]

            if case_id != "WI079-missing-report":
                report_path.write_text(json.dumps(report), encoding="utf-8")
            output = io.StringIO()
            with redirect_stdout(output):
                actual_exit = self_audit.main([
                    "check", "--root", str(root), "--report", str(report_path),
                ])
            result = json.loads(output.getvalue())
            if not isinstance(result, dict):
                return case_id, False, "helper returned a non-object result"
            actual_result = result.get("result")
            correct = actual_exit == expected_exit and actual_result == expected_result
            if expected_exit == 0:
                correct = correct and (
                    result.get("assurance") == "instruction-only"
                    and result.get("candidate_fingerprint") == report["candidate"]["fingerprint"]
                    and "independent_approval" not in result
                )
            else:
                message = result.get("message", "")
                correct = correct and isinstance(message, str) and expected_message in message
                correct = correct and "assurance" not in result
            return (
                case_id, bool(correct),
                f"expected exit={expected_exit}, result={expected_result}; "
                f"actual exit={actual_exit}, result={actual_result}; "
                f"assurance={result.get('assurance', 'none')}",
            )
    except (OSError, ValueError, KeyError, TypeError, AttributeError,
            subprocess.SubprocessError, self_audit.SelfAuditError) as exc:
        # Unavailable Git, an invalid fixture, or malformed output is not a pass.
        return case_id, False, f"fixture/check error: {type(exc).__name__}"


def run_self_audit_corpus() -> list[tuple[str, bool, str]]:
    return [_check_vector(*vector) for vector in VECTORS]
