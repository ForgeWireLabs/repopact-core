"""Bounded progress capture and crash sentinel for evidence-backed local CI."""

from __future__ import annotations

import json
import os
import re
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


MAX_CAPTURE_LOG_BYTES = 128 * 1024
_STATE_BYTES = 4096
_EVIDENCE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,159}$")
ACTIVE_CAPTURE_ID_ENV = "REPOPACT_ACTIVE_VERIFICATION_CAPTURE_ID"


def active_capture_artifact_paths(root: Path) -> set[str]:
    """Return the current invocation's validated journal paths, if any.

    The marker is intentionally ignored only by source/fixture identity checks
    in the process that owns this exact evidence-backed invocation. Stale,
    malformed, partial, and other run markers remain visible as local files.
    """
    capture_id = os.environ.get(ACTIVE_CAPTURE_ID_ENV, "")
    if not _EVIDENCE_ID.fullmatch(capture_id):
        return set()

    repository_root = root.resolve()
    progress_dir = repository_root / "evidence" / "runs" / ".in-progress"
    state_path = progress_dir / f"{capture_id}.state.json"
    log_path = progress_dir / f"{capture_id}.steps.jsonl.partial"
    try:
        is_junction = getattr(progress_dir, "is_junction", lambda: False)
        if progress_dir.is_symlink() or is_junction() or state_path.is_symlink() or log_path.is_symlink():
            return set()
        progress_dir.resolve(strict=True).relative_to(repository_root)
        state = json.loads(state_path.read_text(encoding="utf-8"))
        if (
            not isinstance(state, dict)
            or state.get("format") != "repopact-local-verification-capture-v1"
            or state.get("id") != capture_id
            or state.get("status") != "partial"
            or state.get("phase") != "running"
            or state.get("capture_log")
            != f"evidence/runs/.in-progress/{capture_id}.steps.jsonl.partial"
            or not log_path.is_file()
            or log_path.stat().st_size > MAX_CAPTURE_LOG_BYTES
        ):
            return set()
    except (OSError, ValueError, json.JSONDecodeError):
        return set()

    return {
        f"evidence/runs/.in-progress/{capture_id}.state.json",
        f"evidence/runs/.in-progress/{capture_id}.steps.jsonl.partial",
    }


class CaptureStateError(RuntimeError):
    """Raised when local progress evidence cannot be safely persisted."""


class CaptureJournal:
    """A durable partial-status marker plus bounded completed-step diagnostics."""

    def __init__(self, root: Path, evidence_id: str, state_path: Path, log_path: Path) -> None:
        self.root = root.resolve()
        self.evidence_id = evidence_id
        self.state_path = state_path
        self.log_path = log_path

    @classmethod
    def begin(
        cls,
        root: Path,
        *,
        evidence_id: str,
        work_item: str,
        profile: str,
        candidate_at_start: dict[str, Any],
        storage_preflight: dict[str, Any] | None = None,
    ) -> "CaptureJournal":
        root = root.resolve()
        if not _EVIDENCE_ID.fullmatch(evidence_id):
            raise CaptureStateError(f"invalid local capture id: {evidence_id!r}")
        progress_dir = root / "evidence" / "runs" / ".in-progress"
        capture_dirs = (root / "evidence", root / "evidence" / "runs", progress_dir)
        for directory in capture_dirs:
            is_junction = getattr(directory, "is_junction", lambda: False)
            if directory.is_symlink() or is_junction():
                raise CaptureStateError(f"capture evidence path must not use a symlink or junction: {directory}")
        progress_dir.mkdir(parents=True, exist_ok=True)
        try:
            progress_dir.resolve(strict=True).relative_to(root)
        except (OSError, ValueError) as exc:
            raise CaptureStateError("capture evidence directory resolves outside the repository") from exc
        state_path = progress_dir / f"{evidence_id}.state.json"
        log_path = progress_dir / f"{evidence_id}.steps.jsonl.partial"
        final_record = root / "evidence" / "runs" / f"{evidence_id}.json"
        if state_path.exists() or log_path.exists() or final_record.exists():
            raise CaptureStateError(f"verification evidence id already exists: {evidence_id}")
        journal = cls(root, evidence_id, state_path, log_path)
        state = {
            "format": "repopact-local-verification-capture-v1",
            "id": evidence_id,
            "work_item": work_item,
            "profile": profile,
            "status": "partial",
            "phase": "running",
            "started_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
            "candidate_at_start": candidate_at_start,
            "storage_preflight": storage_preflight,
            "relevant_environment": {
                "CARGO_TARGET_DIR": os.environ.get("CARGO_TARGET_DIR"),
                "REPOPACT_ENGINE": os.environ.get("REPOPACT_ENGINE"),
            },
            "capture_log": log_path.relative_to(root).as_posix(),
            "diagnostic": "run has not reached durable final evidence; treat as incomplete",
        }
        try:
            journal._write_initial_state(state)
            with log_path.open("xb") as handle:
                handle.flush()
                os.fsync(handle.fileno())
        except OSError as exc:
            # The fixed-size state marker, if created, remains a fail-closed sentinel.
            raise CaptureStateError(f"cannot initialize local verification capture: {exc}") from exc
        return journal

    def _write_initial_state(self, state: dict[str, Any]) -> None:
        payload = json.dumps(state, indent=2, sort_keys=True).encode("utf-8")
        if len(payload) >= _STATE_BYTES:
            raise CaptureStateError("capture state exceeds its fixed storage reservation")
        with self.state_path.open("xb") as handle:
            handle.write(payload.ljust(_STATE_BYTES, b" "))
            handle.flush()
            os.fsync(handle.fileno())

    def _read_state(self) -> dict[str, Any]:
        try:
            document = json.loads(self.state_path.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            raise CaptureStateError(f"capture state is unreadable: {self.state_path}: {exc}") from exc
        if not isinstance(document, dict) or document.get("id") != self.evidence_id:
            raise CaptureStateError(f"capture state identity mismatch: {self.state_path}")
        return document

    def _write_state(self, state: dict[str, Any]) -> None:
        payload = json.dumps(state, indent=2, sort_keys=True).encode("utf-8")
        if len(payload) >= _STATE_BYTES:
            payload = json.dumps(
                {
                    "format": state.get("format"),
                    "id": self.evidence_id,
                    "work_item": state.get("work_item"),
                    "profile": state.get("profile"),
                    "status": "partial",
                    "phase": "incomplete",
                    "started_at": state.get("started_at"),
                    "capture_log": state.get("capture_log"),
                    "diagnostic": str(state.get("diagnostic", "capture interrupted"))[:256],
                },
                indent=2,
                sort_keys=True,
            ).encode("utf-8")
        if len(payload) >= _STATE_BYTES:
            raise CaptureStateError("capture state exceeds its fixed storage reservation")
        try:
            with self.state_path.open("r+b") as handle:
                handle.seek(0)
                handle.write(payload.ljust(_STATE_BYTES, b" "))
                handle.flush()
                os.fsync(handle.fileno())
        except OSError as exc:
            raise CaptureStateError(f"cannot update local verification state: {exc}") from exc

    def record_step(self, step: Any) -> None:
        """Persist one bounded completed-step summary; never record arbitrary env values."""
        record = {
            "step": step.id,
            "status": step.status,
            "exit_code": step.exit_code,
            "summary": str(step.summary)[:1000],
            "stdout_tail": str(step.stdout)[-4000:],
            "stderr_tail": str(step.stderr)[-4000:],
        }
        encoded = (json.dumps(record, ensure_ascii=False, sort_keys=True) + "\n").encode("utf-8")
        try:
            current = self.log_path.stat().st_size
            if current + len(encoded) > MAX_CAPTURE_LOG_BYTES:
                raise OSError(f"bounded capture limit of {MAX_CAPTURE_LOG_BYTES} bytes reached")
            with self.log_path.open("ab") as handle:
                handle.write(encoded)
                handle.flush()
                os.fsync(handle.fileno())
        except OSError as exc:
            raise CaptureStateError(f"partial step log write failed: {exc}") from exc

    def mark_incomplete(self, diagnostic: str, *, phase: str = "interrupted") -> None:
        state = self._read_state()
        state["status"] = "partial"
        state["phase"] = phase
        state["diagnostic"] = str(diagnostic)[:1000]
        state["updated_at"] = datetime.now(timezone.utc).isoformat().replace("+00:00", "Z")
        self._write_state(state)

    def preserve_failed_log(self, status: str) -> list[str]:
        if status == "pass" or not self.log_path.exists() or self.log_path.stat().st_size == 0:
            return []
        final_log = self.root / "evidence" / "runs" / f"{self.evidence_id}.capture.jsonl"
        if final_log.exists():
            raise CaptureStateError(f"refusing to overwrite captured step log: {final_log}")
        os.replace(self.log_path, final_log)
        state = self._read_state()
        state["capture_log"] = final_log.relative_to(self.root).as_posix()
        state["phase"] = "recording-failed-run"
        self._write_state(state)
        return [final_log.relative_to(self.root).as_posix()]

    def complete(self, *, passing: bool) -> None:
        if passing or (self.log_path.exists() and self.log_path.stat().st_size == 0):
            self.log_path.unlink(missing_ok=True)
        self.state_path.unlink(missing_ok=True)
        try:
            self.state_path.parent.rmdir()
        except OSError:
            pass
