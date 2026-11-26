from __future__ import annotations

import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from repopact import verification, verify_cli, verify_status
from repopact.verification_capture import (
    ACTIVE_CAPTURE_ID_ENV,
    active_capture_artifact_paths,
)


class _BrokenOutput:
    def write(self, _value: str) -> int:
        raise OSError(28, "simulated disk full during output capture")

    def flush(self) -> None:
        return None


class _ShortOutput:
    def write(self, value: str) -> int:
        return max(0, len(value) - 1)

    def flush(self) -> None:
        return None


class VerificationCaptureTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="repopact-verify-capture-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        (self.root / "governance").mkdir()
        (self.root / "work" / "active" / "081-capture-test").mkdir(parents=True)
        (self.root / "evidence" / "runs").mkdir(parents=True)
        (self.root / "work" / "active" / "081-capture-test" / "work-item.json").write_text(
            json.dumps({"id": "081"}), encoding="utf-8"
        )
        config = verification.default_verification_config()
        config["default_profile"] = "ci"
        config["profiles"] = {
            "ci": {
                "description": "bounded capture fixture",
                "coverage": "host",
                "steps": [
                    {
                        "id": "probe",
                        "argv": [sys.executable, "-c", "print('completed-step-output')"],
                        "required": True,
                    }
                ],
            }
        }
        (self.root / "governance" / "verification.json").write_text(
            json.dumps(config), encoding="utf-8"
        )

    def test_interrupted_profile_remains_incomplete_and_not_a_pass(self) -> None:
        previous_capture_id = os.environ.get(ACTIVE_CAPTURE_ID_ENV)

        def interrupt_after_capture_starts(*_args, **_kwargs):
            self.assertEqual(
                "20261124-interrupted-capture",
                os.environ.get(ACTIVE_CAPTURE_ID_ENV),
            )
            self.assertEqual(
                {
                    "evidence/runs/.in-progress/20261124-interrupted-capture.state.json",
                    "evidence/runs/.in-progress/20261124-interrupted-capture.steps.jsonl.partial",
                },
                active_capture_artifact_paths(self.root),
            )
            raise KeyboardInterrupt

        with patch("repopact.local_storage.disk_preflight", return_value={"ready": True, "volumes": []}):
            with patch(
                "repopact.verify_cli.run_profile",
                side_effect=interrupt_after_capture_starts,
            ):
                result = verify_cli.main(
                    [
                        "ci",
                        "--root",
                        str(self.root),
                        "--json",
                        "--evidence-work-item",
                        "081",
                        "--evidence-id",
                        "20261124-interrupted-capture",
                    ]
                )
        self.assertEqual(previous_capture_id, os.environ.get(ACTIVE_CAPTURE_ID_ENV))
        self.assertEqual(2, result)
        state_path = (
            self.root
            / "evidence"
            / "runs"
            / ".in-progress"
            / "20261124-interrupted-capture.state.json"
        )
        state = json.loads(state_path.read_text(encoding="utf-8"))
        self.assertEqual("partial", state["status"])
        self.assertEqual("interrupted", state["phase"])
        self.assertFalse((self.root / "evidence" / "runs" / "20261124-interrupted-capture.json").exists())
        status = verify_status._local_verification_state(self.root)
        self.assertEqual("incomplete", status["effective_status"])

    def test_disk_full_output_capture_returns_incomplete_and_keeps_marker(self) -> None:
        args = [
            "ci",
            "--root",
            str(self.root),
            "--json",
            "--evidence-work-item",
            "081",
            "--evidence-id",
            "20261124-output-capture-failure",
        ]
        with patch("repopact.local_storage.disk_preflight", return_value={"ready": True, "volumes": []}):
            with patch(
                "repopact.verify_cli.record_evidence",
                return_value="evidence/runs/20261124-output-capture-failure.json",
            ):
                with patch("sys.stdout", _BrokenOutput()):
                    result = verify_cli.main(args)
        self.assertEqual(2, result)
        state_path = (
            self.root
            / "evidence"
            / "runs"
            / ".in-progress"
            / "20261124-output-capture-failure.state.json"
        )
        state = json.loads(state_path.read_text(encoding="utf-8"))
        self.assertEqual("partial", state["status"])
        self.assertIn("verification output capture failed", state["diagnostic"])
        self.assertEqual(
            "incomplete",
            verify_status._local_verification_state(self.root)["effective_status"],
        )

    def test_short_output_write_is_incomplete(self) -> None:
        args = ["ci", "--root", str(self.root), "--json", "--evidence-work-item", "081"]
        def expected_evidence_path(_root, _report, _work_item, *, evidence_id=None, **_kwargs):
            return f"evidence/runs/{evidence_id}.json"

        with patch("repopact.local_storage.disk_preflight", return_value={"ready": True, "volumes": []}):
            with patch(
                "repopact.verify_cli.record_evidence",
                side_effect=expected_evidence_path,
            ):
                with patch("sys.stdout", _ShortOutput()):
                    result = verify_cli.main(args)
        self.assertEqual(2, result)
        state_paths = list((self.root / "evidence" / "runs" / ".in-progress").glob("*.state.json"))
        self.assertEqual(1, len(state_paths))
        state = json.loads(state_paths[0].read_text(encoding="utf-8"))
        self.assertEqual("partial", state["status"])

    def test_capture_refuses_symlinked_evidence_directory(self) -> None:
        outside = self.root.parent / f"{self.root.name}-outside"
        outside.mkdir()
        self.addCleanup(outside.rmdir)
        import shutil

        shutil.rmtree(self.root / "evidence")
        try:
            (self.root / "evidence").symlink_to(outside, target_is_directory=True)
        except OSError as exc:
            self.skipTest(f"host cannot create a directory symlink: {exc}")
        with self.assertRaises(verify_cli.CaptureStateError):
            verify_cli.CaptureJournal.begin(
                self.root,
                evidence_id="symlink-capture",
                work_item="081",
                profile="ci",
                candidate_at_start={"commit": None, "tree": None, "dirty": True},
            )

    def test_evidence_storage_failure_never_emits_a_pass_report(self) -> None:
        stdout = __import__("io").StringIO()
        stderr = __import__("io").StringIO()
        args = [
            "ci",
            "--root",
            str(self.root),
            "--json",
            "--evidence-work-item",
            "081",
            "--evidence-id",
            "20261124-evidence-write-failure",
        ]
        with patch("repopact.local_storage.disk_preflight", return_value={"ready": True, "volumes": []}):
            with patch("repopact.verify_cli.record_evidence", side_effect=OSError(28, "disk full")):
                with patch("sys.stdout", stdout), patch("sys.stderr", stderr):
                    result = verify_cli.main(args)
        self.assertEqual(2, result)
        self.assertEqual("", stdout.getvalue())
        self.assertIn("INCOMPLETE", stderr.getvalue())
        state_path = (
            self.root
            / "evidence"
            / "runs"
            / ".in-progress"
            / "20261124-evidence-write-failure.state.json"
        )
        state = json.loads(state_path.read_text(encoding="utf-8"))
        self.assertEqual("partial", state["status"])
        self.assertEqual("storage-failed", state["phase"])


    def test_contract_is_loaded_once_for_recorded_and_unrecorded_runs(self) -> None:
        original_loader = verification._load_contract
        scenarios = [
            (False, None),
            (False, "ci"),
            (True, None),
            (True, "ci"),
        ]
        for index, (record_evidence, explicit_profile) in enumerate(scenarios):
            with self.subTest(record_evidence=record_evidence, explicit_profile=explicit_profile):
                args = ["--root", str(self.root), "--json"]
                if explicit_profile:
                    args.insert(0, explicit_profile)
                evidence_id = f"single-load-{index}"
                if record_evidence:
                    args.extend(["--evidence-work-item", "081", "--evidence-id", evidence_id])

                def expected_evidence_path(_root, _report, _work_item, *, evidence_id=None, **_kwargs):
                    return f"evidence/runs/{evidence_id}.json"

                cli_loader = patch("repopact.verify_cli._load_contract", wraps=original_loader)
                runner_loader = patch("repopact.verification._load_contract", wraps=original_loader)
                with cli_loader as cli_load, runner_loader as runner_load:
                    with patch(
                        "repopact.local_storage.disk_preflight",
                        return_value={"ready": True, "volumes": []},
                    ):
                        with patch(
                            "repopact.verify_cli.record_evidence",
                            side_effect=expected_evidence_path,
                        ):
                            with patch("sys.stdout", io.StringIO()), patch("sys.stderr", io.StringIO()):
                                result = verify_cli.main(args)

                self.assertEqual(0, result)
                self.assertEqual(1, cli_load.call_count + runner_load.call_count)
                self.assertEqual(1 if record_evidence else 0, cli_load.call_count)
                self.assertEqual(0 if record_evidence else 1, runner_load.call_count)

    def test_recorded_run_refuses_verification_contract_drift(self) -> None:
        evidence_id = "verification-contract-drift"
        args = [
            "ci",
            "--root",
            str(self.root),
            "--json",
            "--evidence-work-item",
            "081",
            "--evidence-id",
            evidence_id,
        ]
        original_begin = verify_cli.CaptureJournal.begin

        def begin_then_drift(*begin_args, **begin_kwargs):
            journal = original_begin(*begin_args, **begin_kwargs)
            config_path = self.root / "governance" / "verification.json"
            config = json.loads(config_path.read_text(encoding="utf-8"))
            config["profiles"]["ci"]["steps"][0]["argv"] = [
                sys.executable,
                "-c",
                "print('changed-policy')",
            ]
            config_path.write_text(json.dumps(config), encoding="utf-8")
            return journal

        stdout = io.StringIO()
        stderr = io.StringIO()
        with patch(
            "repopact.local_storage.disk_preflight",
            return_value={"ready": True, "volumes": []},
        ):
            with patch(
                "repopact.verify_cli.CaptureJournal.begin",
                side_effect=begin_then_drift,
            ):
                with patch("sys.stdout", stdout), patch("sys.stderr", stderr):
                    result = verify_cli.main(args)

        self.assertEqual(2, result)
        self.assertEqual("", stdout.getvalue())
        self.assertIn("changed after profile selection", stderr.getvalue())
        progress_dir = self.root / "evidence" / "runs" / ".in-progress"
        state = json.loads((progress_dir / f"{evidence_id}.state.json").read_text(encoding="utf-8"))
        self.assertEqual("recording-failed", state["phase"])
        log_path = progress_dir / f"{evidence_id}.steps.jsonl.partial"
        self.assertEqual(0, log_path.stat().st_size)
        self.assertFalse((self.root / "evidence" / "runs" / f"{evidence_id}.json").exists())

    def test_run_profile_rejects_unchecked_dictionary_handoff(self) -> None:
        raw_config = verification.load_contract(self.root)
        with self.assertRaisesRegex(
            verification.VerificationConfigError,
            "only a validated contract",
        ):
            verification.run_profile(self.root, "ci", _loaded_contract=raw_config)


    def test_validated_contract_snapshot_cannot_be_reused_across_runs(self) -> None:
        loaded = verification._load_contract(self.root)
        with patch(
            "repopact.local_storage.disk_preflight",
            return_value={"ready": True, "volumes": []},
        ):
            first = verification.run_profile(self.root, "ci", _loaded_contract=loaded)
        self.assertEqual("pass", first.status)
        with self.assertRaisesRegex(
            verification.VerificationConfigError,
            "already used",
        ):
            verification.run_profile(self.root, "ci", _loaded_contract=loaded)


if __name__ == "__main__":
    unittest.main()
