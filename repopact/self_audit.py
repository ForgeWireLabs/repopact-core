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
import threading
from pathlib import Path, PurePosixPath
from typing import Any

_MAX_COMMAND_OUTPUT = 16 * 1024 * 1024
_MAX_CONTENT_BYTES = 256 * 1024 * 1024
_MAX_TOTAL_CONTENT_BYTES = 512 * 1024 * 1024
_MAX_REPORT_BYTES = 1024 * 1024
_MAX_CHANGED_PATHS = 10000
_TIMEOUT_SECONDS = 30


class SelfAuditError(RuntimeError):
    pass


def _git(root: Path, *args: str) -> bytes:
    """Run Git with a hard stdout budget and a portable process timeout."""
    try:
        process = subprocess.Popen(
            ["git", "--no-pager", *args],
            cwd=root,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            env={**os.environ, "GIT_TERMINAL_PROMPT": "0", "GIT_OPTIONAL_LOCKS": "0"},
        )
    except OSError as exc:
        raise SelfAuditError(f"Git operation unavailable: {args[0]}") from exc
    chunks: list[bytes] = []
    used = 0
    overflow = False
    reader_error: OSError | None = None

    def read_stdout() -> None:
        nonlocal used, overflow, reader_error
        try:
            assert process.stdout is not None
            while chunk := process.stdout.read(65536):
                used += len(chunk)
                if used > _MAX_COMMAND_OUTPUT:
                    overflow = True
                    process.kill()
                    break
                chunks.append(chunk)
        except (OSError, ProcessLookupError) as exc:
            reader_error = exc

    reader = threading.Thread(target=read_stdout, daemon=True)
    reader.start()
    timed_out = False
    try:
        process.wait(timeout=_TIMEOUT_SECONDS)
    except subprocess.TimeoutExpired:
        timed_out = True
        process.kill()
        process.wait()
    reader.join(timeout=5)
    if process.stdout is not None:
        process.stdout.close()
    if reader.is_alive() or timed_out:
        raise SelfAuditError(f"Git operation timed out: {args[0]}")
    if overflow:
        raise SelfAuditError(f"Git output exceeded bounded capture: {args[0]}")
    if reader_error is not None:
        raise SelfAuditError(f"Git output unreadable: {args[0]}") from reader_error
    if process.returncode != 0:
        raise SelfAuditError(f"Git operation failed: {args[0]} (exit {process.returncode})")
    return b"".join(chunks)

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
        parent = path.parent.resolve()
    except (OSError, RuntimeError) as exc:
        raise SelfAuditError(f"Changed path parent inaccessible: {rel}") from exc
    if not parent.is_relative_to(root):
        raise SelfAuditError(f"Changed path escapes worktree through parent symlink: {rel}")
    try:
        info = path.lstat()
    except FileNotFoundError:
        return {"path": rel, "kind": "deleted"}
    except OSError as exc:
        raise SelfAuditError(f"Changed path inaccessible: {rel}") from exc
    if stat.S_ISLNK(info.st_mode):
        try:
            target = os.readlink(path)
        except OSError as exc:
            raise SelfAuditError(f"Symlink changed during capture: {rel}") from exc
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
    try:
        after = path.stat(follow_symlinks=False)
    except OSError as exc:
        raise SelfAuditError(f"Changed file disappeared during capture: {rel}") from exc
    if (after.st_size, after.st_mtime_ns, after.st_ino) != (info.st_size, info.st_mtime_ns, info.st_ino):
        raise SelfAuditError(f"Changed file moved or mutated during capture: {rel}")
    return {"path": rel, "kind": "file", "bytes": total, "sha256": digest.hexdigest()}


def snapshot(root: Path, base_ref: str) -> dict[str, Any]:
    """Capture a conservative, bounded Git working-tree candidate identity.

    HEAD and the entire index are represented separately. Working-tree paths
    changed relative to base, plus nonignored untracked files, are content-hashed.
    Ignored files are outside this contract and must not be claimed reviewed.
    """
    try:
        root = root.resolve()
    except (OSError, RuntimeError) as exc:
        raise SelfAuditError("Repository root is inaccessible") from exc
    if not root.is_dir() or not base_ref or base_ref.startswith("-"):
        raise SelfAuditError("A repository root and explicit Git base are required")
    try:
        actual_root = Path(os.fsdecode(_git(root, "rev-parse", "--show-toplevel").strip())).resolve()
    except (OSError, RuntimeError) as exc:
        raise SelfAuditError("Git worktree root is inaccessible") from exc
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
    if not isinstance(record, dict):
        raise SelfAuditError("Review report must be a JSON object")
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
        if not isinstance(item, dict):
            raise SelfAuditError("FAIL: unresolved or invalid finding")
        if set(item) != {"path", "status", "scenario"}:
            raise SelfAuditError("INCOMPLETE: finding fields cannot carry approval assertions")
        if item.get("status") not in {"fixed", "nonblocking"}:
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


def _read_report(path: Path) -> Any:
    """Read a bounded UTF-8 JSON report and normalize malformed inputs."""
    try:
        if not path.is_file():
            raise SelfAuditError("Review report missing or not a regular file")
        with path.open("rb") as handle:
            raw = handle.read(_MAX_REPORT_BYTES + 1)
    except SelfAuditError:
        raise
    except OSError as exc:
        raise SelfAuditError("Review report missing or unreadable") from exc
    if len(raw) > _MAX_REPORT_BYTES:
        raise SelfAuditError("Review report exceeds bounded size")
    try:
        return json.loads(raw.decode("utf-8-sig"))
    except (ValueError, UnicodeError, RecursionError) as exc:
        raise SelfAuditError("Review report missing or invalid") from exc


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
            try:
                root = args.root.resolve()
                path = (args.report if args.report.is_absolute() else root / args.report).resolve()
            except (OSError, RuntimeError) as exc:
                raise SelfAuditError("Review report path or repository root is inaccessible") from exc
            if path.is_relative_to(root):
                raise SelfAuditError("Report must be stored outside the candidate working tree to avoid self-reference")
            value = _read_report(path)
            result = inspect_report(root, value)
    except (SelfAuditError, UnicodeError) as exc:
        failed = str(exc).startswith("FAIL:")
        print(json.dumps({"result": "fail" if failed else "incomplete", "message": str(exc)}, sort_keys=True))
        return 1 if failed else 2
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
