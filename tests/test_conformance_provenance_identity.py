"""WI080 conformance evidence identity and immutability regressions."""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from repopact import run_conformance
from repopact.verification_capture import ACTIVE_CAPTURE_ID_ENV


class ConformanceProvenanceIdentityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        schema = self.root / "repopact" / "schemas" / "evidence-run.schema.json"
        schema.parent.mkdir(parents=True)
        shutil.copyfile(
            run_conformance.ROOT / "repopact" / "schemas" / "evidence-run.schema.json",
            schema,
        )
        self.recorded_at = "2026-11-23T14:48:21.583837Z"

    def git(self, cwd: Path, *args: str) -> str:
        env = {
            key: value
            for key, value in os.environ.items()
            if not key.upper().startswith("GIT_")
        }
        return subprocess.check_output(
            ["git", *args], cwd=cwd, env=env, text=True
        ).strip()

    def payload(self) -> dict:
        return {
            "recorded_at": self.recorded_at,
            "result": "passed",
            "format": "repopact-engine-conformance-provenance-v1",
            "exit_code": 0,
            "source": {"revision": "a" * 40, "dirty": False},
            "engine": {"status": "complete"},
            "build_identity": "repopact-build-v1:sha256:" + "b" * 64,
            "conformance": {
                "passed": 43,
                "failed": 0,
                "wi050_admission_corpus": {"passed": 8, "failed": 0},
            },
        }

    def test_default_filename_is_exact_embedded_id(self):
        with patch.object(run_conformance, "ROOT", self.root):
            path = run_conformance._write_provenance(self.payload(), None)
        record = json.loads(path.read_text(encoding="utf-8"))
        self.assertEqual(path.stem, record["id"])
        self.assertEqual(
            "20261123T144821583837Z-wi076-ac6-engine-conformance",
            record["id"],
        )
        self.assertEqual(self.recorded_at, record["timestamp"])
        self.assertEqual(path.parent, self.root / "evidence" / "runs")

    def test_default_timestamp_in_seconds_still_has_single_id(self):
        payload = self.payload()
        payload["recorded_at"] = "2026-11-23T14:48:21Z"
        with patch.object(run_conformance, "ROOT", self.root):
            path = run_conformance._write_provenance(payload, None)
        self.assertEqual(
            "20261123T144821000000Z-wi076-ac6-engine-conformance",
            json.loads(path.read_text(encoding="utf-8"))["id"],
        )

    def test_explicit_destination_keeps_caller_filename_and_canonical_record_id(self):
        target = self.root / "external" / "customer-chosen.json"
        with patch.object(run_conformance, "ROOT", self.root):
            path = run_conformance._write_provenance(self.payload(), target)
        self.assertEqual(target, path)
        record = json.loads(path.read_text(encoding="utf-8"))
        self.assertNotEqual(target.stem, record["id"])
        self.assertEqual(
            "20261123T144821583837Z-wi076-ac6-engine-conformance",
            record["id"],
        )

    def test_previous_evidence_is_never_overwritten(self):
        with patch.object(run_conformance, "ROOT", self.root):
            path = run_conformance._write_provenance(self.payload(), None)
            previous = path.read_bytes()
            with self.assertRaises(FileExistsError):
                run_conformance._write_provenance(self.payload(), None)
        self.assertEqual(previous, path.read_bytes())

    def test_explicit_destination_is_also_create_only(self):
        target = self.root / "external.json"
        target.write_bytes(b"must remain unchanged")
        with patch.object(run_conformance, "ROOT", self.root):
            with self.assertRaises(FileExistsError):
                run_conformance._write_provenance(self.payload(), target)
        self.assertEqual(b"must remain unchanged", target.read_bytes())

    def test_naive_or_non_utc_timestamps_rejected(self):
        for value in [
            "2026-11-23T14:48:21",
            "2026-11-23T14:48:21+01:00",
            "2026-11-23T14:48:21-06:00",
            "not-a-timestamp",
        ]:
            with self.subTest(value=value):
                with self.assertRaises(ValueError):
                    run_conformance._conformance_evidence_id(value)

    def test_source_identity_ignores_only_current_live_capture_pair(self):
        source = self.root / "source-state"
        source.mkdir()
        self.git(source, "init", "-q", "--initial-branch=main")
        self.git(source, "config", "--local", "user.email", "fixture@example.invalid")
        self.git(source, "config", "--local", "user.name", "Fixture Source")
        (source / "tracked.txt").write_text("committed source\n", encoding="utf-8")
        self.git(source, "add", "tracked.txt")
        self.git(source, "commit", "-q", "-m", "source")
        expected_revision = self.git(source, "rev-parse", "--verify", "HEAD")

        progress = source / "evidence" / "runs" / ".in-progress"
        progress.mkdir(parents=True)
        (progress / "20261124-ci.state.json").write_text(
            json.dumps(
                {
                    "format": "repopact-local-verification-capture-v1",
                    "id": "20261124-ci",
                    "status": "partial",
                    "phase": "running",
                    "capture_log": "evidence/runs/.in-progress/20261124-ci.steps.jsonl.partial",
                }
            ),
            encoding="utf-8",
        )
        (progress / "20261124-ci.steps.jsonl.partial").write_text("", encoding="utf-8")
        with patch.dict(
            os.environ,
            {
                ACTIVE_CAPTURE_ID_ENV: "20261124-ci",
                "GIT_CONFIG_COUNT": "1",
                "GIT_CONFIG_KEY_0": "core.worktree",
                "GIT_CONFIG_VALUE_0": "",
                "GIT_DIR": str(source / "missing-git-dir"),
            },
        ):
            clean = run_conformance._git_source_state(source)
        self.assertEqual(expected_revision, clean["revision"])
        self.assertFalse(clean["dirty"])
        self.assertEqual("", clean["status_porcelain"])
        self.assertEqual(
            [
                "evidence/runs/.in-progress/20261124-ci.state.json",
                "evidence/runs/.in-progress/20261124-ci.steps.jsonl.partial",
            ],
            clean["ignored_runtime_artifacts"],
        )

        with patch.dict(
            os.environ,
            {
                ACTIVE_CAPTURE_ID_ENV: "20261124-ci",
                "GIT_CONFIG_COUNT": "1",
                "GIT_CONFIG_KEY_0": "core.worktree",
                "GIT_CONFIG_VALUE_0": "",
                "GIT_DIR": str(source / "missing-git-dir"),
            },
        ):
            (source / "unrelated.txt").write_text("source change\n", encoding="utf-8")
            dirty = run_conformance._git_source_state(source)
            self.assertTrue(dirty["dirty"])
            self.assertIn("?? unrelated.txt", dirty["status_porcelain"])

            (source / "unrelated.txt").unlink()
            orphan = progress / "20261124-orphan.state.json"
            orphan.write_text("{}\n", encoding="utf-8")
            incomplete = run_conformance._git_source_state(source)
            self.assertTrue(incomplete["dirty"])
            self.assertIn("20261124-orphan.state.json", incomplete["status_porcelain"])

            stale = progress / "20261124-stale.state.json"
            stale.write_text("{}\n", encoding="utf-8")
            (progress / "20261124-stale.steps.jsonl.partial").write_text("", encoding="utf-8")
            stale_status = run_conformance._git_source_state(source)
            self.assertTrue(stale_status["dirty"])
            self.assertIn("20261124-stale.state.json", stale_status["status_porcelain"])


if __name__ == "__main__":
    unittest.main()
