"""WI079 exploratory, opt-in self-audit candidate fingerprint helper.

This is a structural freshness check for a SELF-REPORTED review, not RepoPact's
canonical completion validator, an independent reviewer, or a security boundary.
It uses only the Python standard library and Git's argv interface.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import stat
import subprocess
import sys
from pathlib import Path, PurePosixPath
from typing import Any

_MAX_COMMAND_OUTPUT = 16 * 1024 * 1024
_MAX_CONTENT_BYTES = 256 * 1024 * 1024
_MAX_TOTAL_CONTENT_BYTES = 512 * 1024 * 1024
_MAX_CHANGED_PATHS = 10000
_TIMEOUT_SECONDS = 30


class SelfAuditError(RuntimeError):
    pass


def _git(root: Path, *args: str) -> bytes:
    try:
        result = subprocess.run(
            ["git", "--no-pager", *args],
            cwd=root,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=_TIMEOUT_SECONDS,
            check=False,
            env={**os.environ, "GIT_TERMINAL_PROMPT": "0", "GIT_OPTIONAL_LOCKS": "0"},
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise SelfAuditError(f"Git operation unavailable: {args[0]}") from exc
    if result.returncode != 0:
        raise SelfAuditError(
            f"Git operation failed: {args[0]} (exit {result.returncode})"
        )
    if len(result.stdout) > _MAX_COMMAND_OUTPUT:
        raise SelfAuditError(f"Git output exceeded bounded capture: {args[0]}")
    return result.stdout


def _sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _paths(data: bytes) -> set[str]:
    return {os.fsdecode(p) for p in data.split(b"\0") if p}


def _file_state(root: Path, rel: str) -> dict[str, Any]:
    name = PurePosixPath(rel)
    if name.is_absolute() or ".." in name.parts or not rel or "\\" in rel:
        raise SelfAuditError("Unsafe Git path in candidate")
    path = root.joinpath(*name.parts)
    try:
        info = path.lstat()
    except FileNotFoundError:
        return {"path": rel, "kind": "deleted"}
    except OSError as exc:
        raise SelfAuditError(f"Changed path inaccessible: {rel}") from exc
    if stat.S_ISLNK(info.st_mode):
        target = os.readlink(path)
        return {
            "path": rel, "kind": "symlink",
            "sha256": _sha(os.fsencode(target)),
        }
    if not stat.S_ISREG(info.st_mode):
        raise SelfAuditError(f"Unsupported changed path type: {rel}")
    if info.st_size > _MAX_CONTENT_BYTES:
        raise SelfAuditError(f"Changed file exceeds bounded capture: {rel}")
    digest = hashlib.sha256()
    total = 0
    try:
        with path.open("rb") as handle:
            while data := handle.read(1024 * 1024):
                total += len(data)
                if total > _MAX_CONTENT_BYTES:
                    raise SelfAuditError(f"Changed file grew beyond limit: {rel}")
                digest.update(data)
    except OSError as exc:
        raise SelfAuditError(f"Changed file unreadable: {rel}") from exc
    after = path.stat(follow_symlinks=False)
    if (after.st_size, after.st_mtime_ns, after.st_ino) != (info.st_size, info.st_mtime_ns, info.st_ino):
        raise SelfAuditError(f"Changed file moved or mutated during capture: {rel}")
    return {"path": rel, "kind": "file", "bytes": total, "sha256": digest.hexdigest()}


def snapshot(root: Path, base_ref: str) -> dict[str, Any]:
    """Capture a conservative, bounded Git working-tree candidate identity.

    HEAD and the entire index are represented separately. Working-tree paths
    changed relative to base, plus nonignored untracked files, are content-hashed.
    Ignored files are outside this contract and must not be claimed reviewed.
    """
    root = root.resolve()
    if not root.is_dir() or not base_ref or base_ref.startswith("-"):
        raise SelfAuditError("A repository root and explicit Git base are required")
    actual_root = Path(os.fsdecode(_git(root, "rev-parse", "--show-toplevel").strip())).resolve()
    if actual_root != root:
        raise SelfAuditError("Root must be the top-level Git working tree")
    base = _git(root, "rev-parse", "--verify", f"{base_ref}^{{commit}}").strip().decode("ascii")
    head = _git(root, "rev-parse", "--verify", "HEAD^{commit}").strip().decode("ascii")
    gitdir = os.fsdecode(_git(root, "rev-parse", "--absolute-git-dir").strip())
    index = _git(root, "ls-files", "--stage", "-z")
    tracked = _paths(_git(
        root, "diff", "--name-only", "-z", "--no-ext-diff",
        "--no-textconv", "--no-renames", base, "--",
    ))
    untracked = _paths(_git(root, "ls-files", "--others", "--exclude-standard", "-z"))
    status = _git(root, "status", "--porcelain=v1", "-z", "--untracked-files=all", "--ignore-submodules=none")
    changed = sorted(tracked | untracked)
    if len(changed) > _MAX_CHANGED_PATHS:
        raise SelfAuditError("Changed-path count exceeds bounded capture")
    states = []
    total = 0
    for rel in changed:
        state = _file_state(root, rel)
        total += state.get("bytes", 0)
        if total > _MAX_TOTAL_CONTENT_BYTES:
            raise SelfAuditError("Changed-file content exceeds aggregate capture budget")
        states.append(state)
    # Reject a visibly concurrent change rather than binding an inconsistent snapshot.
    if (index != _git(root, "ls-files", "--stage", "-z")
            or status != _git(root, "status", "--porcelain=v1", "-z", "--untracked-files=all", "--ignore-submodules=none")
            or head != _git(root, "rev-parse", "--verify", "HEAD^{commit}").strip().decode("ascii")):
        raise SelfAuditError("Candidate changed during capture")
    payload = {
        "format_version": 1,
        "base_ref": base_ref,
        "base_commit": base,
        "head_commit": head,
        "worktree_key": _sha(os.fsencode(str(root)) + b"\0" + os.fsencode(gitdir)),
        "index_sha256": _sha(index),
        "status_sha256": _sha(status),
        "changed_paths": sorted(tracked | untracked),
        "content_states": states,
        "ignored_files_covered": False,
    }
    payload["fingerprint"] = _sha(json.dumps(
        payload, sort_keys=True, ensure_ascii=True, separators=(",", ":")
    ).encode("utf-8"))
    return payload


def report_template(root: Path, base_ref: str) -> dict[str, Any]:
    return {
        "format_version": 1,
        "kind": "agent-self-review-claim",
        "candidate": snapshot(root, base_ref),
        "review": {
            "outcome": "incomplete",
            "actor": "",
            "reviewed_paths": [],
            "findings": [],
            "limitations": ["No review attested; no test execution evidenced by this report"],
        },
    }


def inspect_report(root: Path, record: dict[str, Any]) -> dict[str, Any]:
    """Reject stale/incomplete self-reports, never certify review quality."""
    if record.get("format_version") != 1 or record.get("kind") != "agent-self-review-claim":
        raise SelfAuditError("Unsupported self-audit report format")
    if set(record) != {"format_version", "kind", "candidate", "review"}:
        raise SelfAuditError("Unsupported report fields; self-review cannot convey approval authority")
    expected = record.get("candidate")
    if not isinstance(expected, dict) or not isinstance(expected.get("base_ref"), str):
        raise SelfAuditError("Report lacks candidate identity")
    current = snapshot(root, expected["base_ref"])
    if current != expected:
        raise SelfAuditError("STALE: repository candidate differs from reviewed snapshot")
    review = record.get("review")
    if not isinstance(review, dict) or review.get("outcome") != "pass":
        raise SelfAuditError("INCOMPLETE: no passing self-review claim")
    if set(review) != {"outcome", "actor", "reviewed_paths", "findings", "limitations"}:
        raise SelfAuditError("INCOMPLETE: invalid self-review fields or approval assertion")
    if not isinstance(review["limitations"], list) or any(
        not isinstance(value, str) for value in review["limitations"]
    ):
        raise SelfAuditError("INCOMPLETE: invalid limitation disclosure")
    if not isinstance(review.get("actor"), str) or not review["actor"].strip():
        raise SelfAuditError("INCOMPLETE: reviewer identity missing")
    covered = review.get("reviewed_paths")
    if (not isinstance(covered, list)
            or any(not isinstance(path, str) for path in covered)
            or len(covered) != len(set(covered))):
        raise SelfAuditError("INCOMPLETE: invalid reviewed-path coverage")
    if set(covered) != set(current["changed_paths"]):
        raise SelfAuditError("INCOMPLETE: reviewed paths do not match candidate")
    findings = review.get("findings")
    if not isinstance(findings, list):
        raise SelfAuditError("INCOMPLETE: findings declaration missing")
    for item in findings:
        if not isinstance(item, dict) or item.get("status") not in {"fixed", "nonblocking"}:
            raise SelfAuditError("FAIL: unresolved or invalid finding")
        if not isinstance(item.get("path"), str) or item["path"] not in covered:
            raise SelfAuditError("INCOMPLETE: finding location not in reviewed paths")
        if not isinstance(item.get("scenario"), str) or not item["scenario"].strip():
            raise SelfAuditError("INCOMPLETE: finding failure scenario missing")
    return {
        "result": "self-report-current",
        "assurance": "instruction-only",
        "candidate_fingerprint": current["fingerprint"],
        "changed_paths": len(current["changed_paths"]),
        "caveat": "Structural match only: not independent approval, tool-execution proof, or a protected completion boundary.",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    capture = sub.add_parser("capture", help="Output an incomplete review template")
    capture.add_argument("--root", type=Path, default=Path.cwd())
    capture.add_argument("--base", required=True)
    check = sub.add_parser("check", help="Check a self-reported review against the live Git candidate")
    check.add_argument("--root", type=Path, default=Path.cwd())
    check.add_argument("--report", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        if args.action == "capture":
            result = report_template(args.root, args.base)
        else:
            path = args.report.resolve()
            root = args.root.resolve()
            if path.is_relative_to(root):
                raise SelfAuditError("Report must be stored outside the candidate working tree to avoid self-reference")
            try:
                value = json.loads(path.read_text(encoding="utf-8"))
            except (OSError, ValueError) as exc:
                raise SelfAuditError("Review report missing or invalid") from exc
            result = inspect_report(root, value)
    except (SelfAuditError, UnicodeError) as exc:
        print(json.dumps({"result": "incomplete", "message": str(exc)}, sort_keys=True))
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
