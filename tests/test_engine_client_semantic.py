from __future__ import annotations

import json
import os
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

from repopact import engine_client


SEMANTIC_ID = "repopact-semantic-v1:sha256:" + "a" * 64
STALE_ID = "repopact-semantic-v1:sha256:" + "b" * 64


def fake_engine_source(
    *,
    events: Path,
    mutation: Path,
    identity: str | None = SEMANTIC_ID,
    protocol_version: int = engine_client.PROTOCOL_VERSION,
    read_until_eof: bool = False,
    exit_after_operation: bool = False,
    operation_delay_seconds: float = 0,
) -> str:
    """Return a line-oriented stand-in used to test process/session boundaries."""
    line_source = "sys.stdin.read().splitlines()" if read_until_eof else "sys.stdin"
    return f"""
import json
import os
import sys
import time
from pathlib import Path

events = Path({str(events)!r})
mutation = Path({str(mutation)!r})
identity = {identity!r}
for line in {line_source}:
    request = json.loads(line)
    operation = request["operation"]
    with events.open("a", encoding="utf-8") as stream:
        stream.write(json.dumps({{"pid": os.getpid(), "operation": operation}}) + "\\n")
    response = {{
        "protocol": "repopact-engine",
        "protocol_version": {protocol_version!r},
        "request_id": request["request_id"],
        "engine_version": {engine_client.expected_product_version()!r},
        "ok": True,
        "build_info": {{
            "source_revision": "0123456789abcdef0123456789abcdef01234567",
            "rustc_version": "rustc 1.90.0; host: test-host",
            "target": "test-target",
            "profile": "release",
        }},
    }}
    if identity is not None:
        response["semantic_identity"] = identity
    if operation == "handshake":
        response["capabilities"] = {{
            "operations": ["validate", "dashboard.write"],
            "semantic_surfaces": ["repository", "documentation-mappings"],
        }}
        response["result"] = {{"status": "ready"}}
    elif operation == "admit":
        response["result"] = {{
            "status": "admitted",
            "semantic_identity": request["params"].get("semantic_identity"),
        }}
    else:
        mutation.write_text(operation, encoding="utf-8")
        if {operation_delay_seconds!r}:
            time.sleep({operation_delay_seconds!r})
        response["result"] = {{"accepted": True}}
        if {exit_after_operation!r}:
            sys.exit(0)
    print(json.dumps(response), flush=True)
"""


class EngineClientSemanticAdmissionTests(unittest.TestCase):
    def make_client(
        self,
        directory: Path,
        *,
        identity: str | None = SEMANTIC_ID,
        protocol_version: int = engine_client.PROTOCOL_VERSION,
        read_until_eof: bool = False,
        exit_after_operation: bool = False,
        operation_delay_seconds: float = 0,
        timeout: float = 3,
    ) -> tuple[engine_client.EngineClient, Path, Path]:
        events = directory / "events.jsonl"
        mutation = directory / "mutation.txt"
        script = directory / "repopact-engine"
        script.write_text(
            fake_engine_source(
                events=events,
                mutation=mutation,
                identity=identity,
                protocol_version=protocol_version,
                read_until_eof=read_until_eof,
                exit_after_operation=exit_after_operation,
                operation_delay_seconds=operation_delay_seconds,
            ),
            encoding="utf-8",
        )
        client = engine_client.EngineClient(engine=script, timeout=timeout)
        client._strict = True
        client._expected_semantic_identity = SEMANTIC_ID
        client._spawn_command = lambda: [sys.executable, "-u", str(script)]
        return client, events, mutation

    def events(self, path: Path) -> list[dict[str, object]]:
        if not path.exists():
            return []
        return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]

    def test_handshake_admission_and_operations_share_one_live_process(self):
        with tempfile.TemporaryDirectory() as temporary:
            client, events, _ = self.make_client(Path(temporary))
            try:
                client.check_compatibility("validate")
                client.call("validate")
                client.call("validate")
                observed = self.events(events)
                self.assertEqual(
                    ["handshake", "admit", "validate", "validate"],
                    [event["operation"] for event in observed],
                )
                self.assertEqual(1, len({event["pid"] for event in observed}))
            finally:
                client.close()

    def test_same_product_version_stale_engine_is_rejected_before_mutation(self):
        with tempfile.TemporaryDirectory() as temporary:
            client, events, mutation = self.make_client(
                Path(temporary), identity=STALE_ID
            )
            with self.assertRaisesRegex(engine_client.EngineProtocolError, "semantic identity"):
                client.call("validate")
            self.assertEqual(["handshake"], [event["operation"] for event in self.events(events)])
            self.assertFalse(mutation.exists())

    def test_protocol_v1_engine_is_rejected_before_admission_or_mutation(self):
        with tempfile.TemporaryDirectory() as temporary:
            client, events, mutation = self.make_client(
                Path(temporary), protocol_version=1
            )
            with self.assertRaisesRegex(
                engine_client.EngineProtocolError, "protocol major mismatch"
            ):
                client.call("dashboard.write")
            self.assertEqual(["handshake"], [event["operation"] for event in self.events(events)])
            self.assertFalse(mutation.exists())

    def test_eof_framed_legacy_engine_times_out_without_operation_dispatch(self):
        with tempfile.TemporaryDirectory() as temporary:
            client, events, mutation = self.make_client(
                Path(temporary), protocol_version=1, read_until_eof=True, timeout=0.5
            )
            with self.assertRaisesRegex(
                engine_client.EngineProtocolError, "timed out after 0.5s during handshake"
            ):
                client.call("dashboard.write")
            self.assertEqual([], self.events(events))
            self.assertIsNotNone(client._process)
            self.assertIsNotNone(client._process.poll())
            self.assertFalse(mutation.exists())

    def test_missing_identity_legacy_engine_is_rejected_before_mutation(self):
        with tempfile.TemporaryDirectory() as temporary:
            client, events, mutation = self.make_client(Path(temporary), identity=None)
            with self.assertRaisesRegex(engine_client.EngineProtocolError, "semantic identity"):
                client.call("validate")
            self.assertEqual(["handshake"], [event["operation"] for event in self.events(events)])
            self.assertFalse(mutation.exists())

    def test_malformed_and_unsupported_identities_are_rejected_before_mutation(self):
        for identity in ("not-an-identity", "repopact-semantic-v2:sha256:" + "a" * 64):
            with self.subTest(identity=identity), tempfile.TemporaryDirectory() as temporary:
                client, events, mutation = self.make_client(Path(temporary), identity=identity)
                with self.assertRaisesRegex(engine_client.EngineProtocolError, "semantic identity"):
                    client.call("dashboard.write")
                self.assertEqual(
                    ["handshake"],
                    [event["operation"] for event in self.events(events)],
                )
                self.assertFalse(mutation.exists())

    def test_stale_engine_discovered_beside_installed_python_package_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            environment = Path(temporary) / "venv"
            package_dir = environment / "Lib" / "site-packages" / "repopact"
            scripts_dir = environment / "Scripts"
            package_dir.mkdir(parents=True)
            scripts_dir.mkdir()
            events = Path(temporary) / "events.jsonl"
            mutation = Path(temporary) / "mutation.txt"
            script = scripts_dir / "repopact-engine.exe"
            script.write_text(
                fake_engine_source(events=events, mutation=mutation, identity=STALE_ID),
                encoding="utf-8",
            )
            with (
                mock.patch.object(engine_client, "_candidate_scripts_directories", return_value=[scripts_dir]),
                mock.patch.object(engine_client, "semantic_identity_required", return_value=True),
                mock.patch.object(engine_client, "expected_semantic_identity", return_value=SEMANTIC_ID),
            ):
                client = engine_client.EngineClient(timeout=3)
            client._spawn_command = lambda: [sys.executable, "-u", str(script)]
            with self.assertRaisesRegex(engine_client.EngineProtocolError, "semantic identity"):
                client.call("dashboard.write")
            self.assertEqual(["handshake"], [event["operation"] for event in self.events(events)])
            self.assertFalse(mutation.exists())
            self.assertTrue(package_dir.is_dir())

    def test_explicit_engine_override_does_not_bypass_semantic_admission(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            events = directory / "events.jsonl"
            mutation = directory / "mutation.txt"
            script = directory / "repopact-engine"
            script.write_text(
                fake_engine_source(events=events, mutation=mutation, identity=STALE_ID),
                encoding="utf-8",
            )
            with (
                mock.patch.dict(os.environ, {engine_client.ENGINE_OVERRIDE: str(script)}),
                mock.patch.object(engine_client, "semantic_identity_required", return_value=True),
                mock.patch.object(engine_client, "expected_semantic_identity", return_value=SEMANTIC_ID),
            ):
                client = engine_client.EngineClient(timeout=3)
            client._spawn_command = lambda: [sys.executable, "-u", str(script)]
            with self.assertRaisesRegex(engine_client.EngineProtocolError, "semantic identity"):
                client.call("dashboard.write")
            self.assertEqual(["handshake"], [event["operation"] for event in self.events(events)])
            self.assertFalse(mutation.exists())

    def test_replacing_executable_path_after_admission_does_not_replace_live_process(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            client, events, mutation = self.make_client(directory)
            try:
                client.check_compatibility("validate")
                replacement_events = directory / "replacement.jsonl"
                replacement_mutation = directory / "replacement-mutation.txt"
                client.engine.write_text(
                    fake_engine_source(events=replacement_events, mutation=replacement_mutation),
                    encoding="utf-8",
                )
                client.call("validate")
                self.assertEqual(
                    ["handshake", "admit", "validate"],
                    [event["operation"] for event in self.events(events)],
                )
                self.assertFalse(replacement_events.exists())
                self.assertFalse(replacement_mutation.exists())
                self.assertTrue(mutation.exists())
            finally:
                client.close()

    def test_uncertain_operation_result_is_not_retried(self):
        with tempfile.TemporaryDirectory() as temporary:
            client, events, mutation = self.make_client(
                Path(temporary), exit_after_operation=True
            )
            with self.assertRaises(engine_client.EngineOutcomeUnknownError):
                client.call("dashboard.write")
            operations = [event["operation"] for event in self.events(events)]
            self.assertEqual(["handshake", "admit", "dashboard.write"], operations)
            self.assertTrue(mutation.exists())
            with self.assertRaisesRegex(engine_client.EngineProtocolError, "refusing to relaunch"):
                client.call("dashboard.write")
            self.assertEqual(1, operations.count("dashboard.write"))

    def test_timeout_terminates_live_engine_and_prevents_same_client_reuse(self):
        with tempfile.TemporaryDirectory() as temporary:
            client, events, mutation = self.make_client(
                Path(temporary), operation_delay_seconds=3, timeout=0.25
            )
            with self.assertRaises(engine_client.EngineOutcomeUnknownError):
                client.call("dashboard.write")
            self.assertTrue(mutation.exists())
            self.assertIsNotNone(client._process)
            self.assertIsNotNone(client._process.poll())
            with self.assertRaisesRegex(engine_client.EngineProtocolError, "closed"):
                client.call("dashboard.write")
            operations = [event["operation"] for event in self.events(events)]
            self.assertEqual(1, operations.count("dashboard.write"))

    def test_proposal_unknown_outcome_in_admitted_session_keeps_retry_guidance(self):
        client = engine_client.EngineClient.__new__(engine_client.EngineClient)
        client._strict = True
        client.timeout = 1.0
        client._handshake = mock.Mock()
        client._call_admitted = mock.Mock(
            side_effect=engine_client.EngineOutcomeUnknownError("response lost after dispatch")
        )
        client._sample_engine_working_set = mock.Mock(return_value=None)
        key = "scale-verification-retry-key"

        with self.assertRaises(engine_client.OperationOutcomeUnknownError) as caught:
            client.call("work.propose", params={"idempotency_key": key})

        self.assertIn(key, str(caught.exception))
        self.assertIn("proposal-status", str(caught.exception))
        client._call_admitted.assert_called_once()

    def test_keyboard_interrupt_while_waiting_preserves_cancellation_and_poisons_session(self):
        with tempfile.TemporaryDirectory() as temporary:
            client, events, mutation = self.make_client(
                Path(temporary), operation_delay_seconds=3, timeout=5
            )
            original_get = client._stdout_lines.get
            get_calls = 0

            def interrupt_on_operation_response(*, timeout=None):
                nonlocal get_calls
                get_calls += 1
                if get_calls == 3:
                    deadline = time.monotonic() + 2
                    while not any(
                        event["operation"] == "dashboard.write" for event in self.events(events)
                    ):
                        if time.monotonic() >= deadline:
                            self.fail("fake peer did not receive the operation before interruption")
                        time.sleep(0.01)
                    raise KeyboardInterrupt()
                return original_get(timeout=timeout)

            client._stdout_lines.get = interrupt_on_operation_response
            with self.assertRaises(KeyboardInterrupt) as caught:
                client.call("dashboard.write")
            self.assertTrue(caught.exception.outcome_unknown)
            self.assertEqual("dashboard.write", caught.exception.operation)
            self.assertTrue(mutation.exists())
            self.assertIsNotNone(client._process)
            self.assertIsNotNone(client._process.poll())
            with self.assertRaisesRegex(engine_client.EngineProtocolError, "closed"):
                client.call("dashboard.write")
            operations = [event["operation"] for event in self.events(events)]
            self.assertEqual(1, operations.count("dashboard.write"))

    def test_partial_transport_failure_after_full_write_is_unknown_and_never_replayed(self):
        with tempfile.TemporaryDirectory() as temporary:
            client, events, mutation = self.make_client(
                Path(temporary), operation_delay_seconds=3, timeout=5
            )
            client.check_compatibility("dashboard.write")
            process = client._process
            self.assertIsNotNone(process)
            original_stdin = process.stdin
            self.assertIsNotNone(original_stdin)

            class WriteThenFail:
                writes = 0

                def write(self, value):
                    self.writes += 1
                    written = original_stdin.write(value)
                    original_stdin.flush()
                    if self.writes == 1:
                        deadline = time.monotonic() + 2
                        while not mutation.exists():
                            if time.monotonic() >= deadline:
                                raise AssertionError("fake peer did not apply the delivered operation")
                            time.sleep(0.01)
                        raise OSError("simulated transport loss after complete request delivery")
                    return written

                def flush(self):
                    return original_stdin.flush()

                def close(self):
                    return original_stdin.close()

            process.stdin = WriteThenFail()
            with self.assertRaises(engine_client.EngineOutcomeUnknownError):
                client.call("dashboard.write")
            self.assertTrue(mutation.exists())
            self.assertIsNotNone(process.poll())
            with self.assertRaisesRegex(engine_client.EngineProtocolError, "closed"):
                client.call("dashboard.write")
            operations = [event["operation"] for event in self.events(events)]
            self.assertEqual(1, operations.count("dashboard.write"))

    def test_engine_exit_after_admission_before_dispatch_does_not_relaunch(self):
        with tempfile.TemporaryDirectory() as temporary:
            client, events, mutation = self.make_client(Path(temporary))
            client.check_compatibility("dashboard.write")
            assert client._process is not None
            client._process.kill()
            client._process.wait(timeout=3)
            with self.assertRaisesRegex(engine_client.EngineProtocolError, "refusing to relaunch"):
                client.call("dashboard.write")
            self.assertEqual(
                ["handshake", "admit"],
                [event["operation"] for event in self.events(events)],
            )
            self.assertFalse(mutation.exists())

    def test_provenance_marks_wrapper_process_image_incomplete(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            client, _events, _mutation = self.make_client(directory)
            try:
                client.call("validate")
                evidence = client.provenance()
                self.assertEqual("incomplete", evidence["status"])
                self.assertTrue(any("process image path differs" in reason for reason in evidence["incomplete_reasons"]))
                executable = evidence["executable"]
                self.assertNotEqual(executable["selected_path"], executable["process_image_path"])
                self.assertIsNone(executable["sha256"])
                self.assertEqual(64, len(executable["sha256_before_launch"]))
                self.assertEqual(64, len(executable["sha256_at_process_image"]))
                self.assertEqual(executable["sha256_at_process_image"], executable["sha256_after_run"])
                self.assertEqual(SEMANTIC_ID, evidence["engine"]["semantic_identity"])
                self.assertEqual(2, evidence["engine"]["protocol_version"])
                self.assertEqual(
                    "0123456789abcdef0123456789abcdef01234567",
                    evidence["engine"]["build_info"]["source_revision"],
                )
                self.assertIn("not established", evidence["authenticity"])
            finally:
                client.close()

    def test_python_authoritative_admission_writer_checks_runtime_lock_before_mutation(self):
        from repopact import cli, runtime_dependencies

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with (
                mock.patch.object(engine_client, "semantic_identity_required", return_value=True),
                mock.patch.object(
                    runtime_dependencies,
                    "verify_runtime_dependencies",
                    side_effect=runtime_dependencies.RuntimeDependencyError("test mismatch"),
                ),
                self.assertRaises(runtime_dependencies.RuntimeDependencyError),
            ):
                cli._write_canonical_record(
                    root,
                    "evidence/admission/requests",
                    "request.json",
                    b"{}\n",
                )
            self.assertEqual({}, self.snapshot(root))

    @staticmethod
    def snapshot(root: Path) -> dict[str, bytes]:
        if not root.exists():
            return {}
        return {
            path.relative_to(root).as_posix(): path.read_bytes()
            for path in root.rglob("*")
            if path.is_file()
        }

    def test_retained_python_writers_reject_stale_engine_before_first_change(self):
        from datetime import date

        from repopact import adopt_repo, doctor, init_repo, new, plan_import, takeover, verification

        def verification_record(*args, **kwargs):
            return verification.record_evidence(*args, **kwargs)

        cases = [
            ("adopt", lambda root, client: adopt_repo.adopt(root, client=client)),
            ("bootstrap", lambda root, client: init_repo.bootstrap(root, client=client)),
            ("doctor", lambda root, client: doctor.fix(root, client=client)),
            ("new-markdown", lambda root, client: new.new_markdown("decision", "Probe", date(2026, 10, 24), root, client=client)),
            ("plan-import", lambda root, client: plan_import.import_plan(root, client=client)),
            ("takeover", lambda root, client: takeover.takeover(root, client=client)),
            ("evidence", verification_record),
            ("report-write", lambda root, client: adopt_repo.Report(False, client=client).write(root / "decisions" / "probe.md", "probe", root)),
        ]
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            for index, (name, operation) in enumerate(cases):
                with self.subTest(writer=name):
                    case_root = base / f"case-{index}"
                    case_root.mkdir()
                    (case_root / "existing.txt").write_text("preserve\n", encoding="utf-8")
                    before = self.snapshot(case_root)
                    tool_dir = base / f"tool-{index}"
                    tool_dir.mkdir()
                    client, _events, _mutation = self.make_client(tool_dir, identity=STALE_ID)
                    try:
                        if name == "takeover":
                            result = operation(case_root, client)
                            self.assertFalse(result["validated"])
                        elif name == "evidence":
                            with self.assertRaises(verification.VerificationConfigError):
                                operation(
                                    case_root,
                                    None,
                                    "076",
                                    client=client,
                                )
                        else:
                            with self.assertRaises(engine_client.EngineProtocolError):
                                operation(case_root, client)
                    finally:
                        client.close()
                    self.assertEqual(before, self.snapshot(case_root))

    def test_python_authoritative_spec_writer_checks_runtime_lock_before_mutation(self):
        from repopact import generate_spec, runtime_dependencies

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            spec = root / "SPEC.md"
            spec.write_text("# spec\n", encoding="utf-8")
            before = self.snapshot(root)
            with (
                mock.patch.object(engine_client, "semantic_identity_required", return_value=True),
                mock.patch.object(
                    runtime_dependencies,
                    "verify_runtime_dependencies",
                    side_effect=runtime_dependencies.RuntimeDependencyError("test mismatch"),
                ),
                self.assertRaisesRegex(engine_client.EngineProtocolError, "dependency lock mismatch"),
            ):
                generate_spec.write_spec(root)
            self.assertEqual(before, self.snapshot(root))

    def test_conformance_provenance_is_written_as_standard_evidence_and_preserves_incomplete(self):
        from repopact import run_conformance

        revision = "0123456789abcdef0123456789abcdef01234567"
        payload = {
            "format": "repopact-engine-conformance-provenance-v1",
            "recorded_at": "2026-10-24T18:30:00Z",
            "source": {"revision": revision, "dirty": False},
            "engine": {
                "status": "incomplete",
                "incomplete_reasons": ["test image not correlated"],
                "engine": {
                    "protocol": "repopact-engine",
                    "protocol_version": 2,
                    "product_version": "3.1.4.dev0",
                    "semantic_identity": SEMANTIC_ID,
                    "build_info": {"source_revision": revision},
                },
            },
            "build_identity": "repopact-build-v1:sha256:" + "c" * 64,
            "conformance": {
                "passed": 1,
                "failed": 0,
                "wi050_admission_corpus": {"passed": 1, "failed": 0},
            },
            "result": "partial",
            "exit_code": 1,
        }
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "run.json"
            run_conformance._write_provenance(payload, destination)
            record = json.loads(destination.read_text(encoding="utf-8"))
        self.assertEqual("076", record["work_item"])
        self.assertEqual("partial", record["result"])
        self.assertEqual("provisional", record["provenance"])
        self.assertEqual("incomplete", record["environment"]["engine"]["status"])
        self.assertIn("test image not correlated", record["environment"]["engine"]["incomplete_reasons"])

    def test_semantic_identity_enforcement_boundary_preserves_313(self):
        self.assertFalse(engine_client.semantic_identity_required("3.1.3"))
        self.assertFalse(engine_client.semantic_identity_required("3.1.3.dev1"))
        self.assertTrue(engine_client.semantic_identity_required("3.1.4.dev0"))
        self.assertTrue(engine_client.semantic_identity_required("3.1.4"))


if __name__ == "__main__":
    unittest.main()
