"""WI080 conformance evidence identity and immutability regressions."""
from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from repopact import run_conformance


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


if __name__ == "__main__":
    unittest.main()
