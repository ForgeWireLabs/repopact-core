from __future__ import annotations

import tempfile
import unittest
from datetime import date
from pathlib import Path
from unittest import mock

from repopact import cli, engine_client, takeover, validate_repo


class EngineClientContractTests(unittest.TestCase):
    def test_inconsistent_canonical_validation_fails_closed(self) -> None:
        diagnostics = [{"severity": "error", "code": "invalid", "message": "bad"}]
        response = {
            "ok": True,
            "result": {
                "valid": True,
                "error_count": 0,
                "warning_count": 0,
                "diagnostics": diagnostics,
            },
            "diagnostics": diagnostics,
        }
        with self.assertRaisesRegex(engine_client.EngineProtocolError, "counts are inconsistent"):
            engine_client._validated_result(response)

    def test_mutation_result_conflicts_fail_closed(self) -> None:
        response = {
            "ok": True,
            "result": {"success": True, "changed_paths": [], "diagnostics": [
                {"severity": "error", "code": "failure", "message": "denied"}
            ]},
            "diagnostics": [{"severity": "error", "code": "failure", "message": "denied"}],
        }
        with self.assertRaisesRegex(engine_client.EngineProtocolError, "success conflicts"):
            engine_client.validated_mutation_result(response)

    def test_preflight_requires_advertised_canonical_operations(self) -> None:
        client = object.__new__(engine_client.EngineClient)
        client._handshaken = True
        client._operations = frozenset({"validate"})
        with self.assertRaisesRegex(engine_client.EngineProtocolError, "dashboard.write"):
            client.check_compatibility("validate", "dashboard.write")

    def test_bootstrap_does_not_write_without_required_engine(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "new-repo"
            with mock.patch.object(
                engine_client,
                "EngineClient",
            ) as client_type:
                client_type.return_value.check_compatibility.side_effect = engine_client.EngineUnavailableError(
                    "missing required engine"
                )
                from repopact.init_repo import bootstrap
                with self.assertRaises(engine_client.EngineUnavailableError):
                    bootstrap(root)
            self.assertFalse(root.exists())

    def test_direct_new_work_item_uses_typed_engine_mutation(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            response = {
                "ok": True,
                "result": {
                    "success": True,
                    "changed_paths": ["work/proposed/001-probe/work-item.json"],
                    "diagnostics": [],
                },
                "diagnostics": [],
            }
            with mock.patch.object(engine_client, "EngineClient") as client_type:
                client_type.return_value.call.return_value = response
                from repopact.new import new_work_item
                created = new_work_item("Probe", date(2026, 9, 25), root=root, status="proposed")
            self.assertEqual(root / "work/proposed/001-probe/work-item.json", created)
            client_type.return_value.call.assert_called_once_with(
                "work.create", root=root,
                params={"title": "Probe", "date": "2026-09-25", "status": "proposed"},
            )

    def test_takeover_preserves_source_when_canonical_engine_is_unavailable(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            source = root / "todos" / "legacy"
            source.mkdir(parents=True)
            marker = source / "README.md"
            marker.write_text("preserve me", encoding="utf-8")
            with mock.patch.object(
                takeover,
                "validate_repository",
                side_effect=engine_client.EngineUnavailableError("missing engine"),
            ):
                report = takeover.takeover(root, delete=True)
            self.assertFalse(report["validated"])
            self.assertTrue(marker.is_file())
            self.assertFalse((root / "archive").exists())

    def test_direct_validator_entrypoint_has_no_python_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            response = {
                "ok": True,
                "result": {"valid": True, "error_count": 0, "warning_count": 0, "diagnostics": []},
                "diagnostics": [],
            }
            with (
                mock.patch("sys.argv", ["repopact-validate", "--root", temp]),
                mock.patch.object(engine_client, "EngineClient") as client_type,
                mock.patch.object(validate_repo, "validate", side_effect=AssertionError("Python authority")),
            ):
                client_type.return_value.call.return_value = response
                self.assertEqual(0, validate_repo.main())
                client_type.return_value.call.assert_called_once_with(
                    "validate", root=Path(temp).resolve()
                )

    def test_malformed_and_multiple_responses_fail_closed(self) -> None:
        client = object.__new__(engine_client.EngineClient)
        with self.assertRaises(engine_client.EngineProtocolError):
            client._decode_response("not json", "", "request-1")
        with self.assertRaisesRegex(engine_client.EngineProtocolError, "more than one"):
            client._decode_response('{"ok": true} {}', "", "request-1")

    def test_protocol_and_product_mismatch_fail_closed(self) -> None:
        client = object.__new__(engine_client.EngineClient)
        base = {
            "protocol": engine_client.PROTOCOL,
            "protocol_version": engine_client.PROTOCOL_VERSION,
            "request_id": "request-1",
            "engine_version": engine_client.expected_product_version(),
        }
        with self.assertRaisesRegex(engine_client.EngineProtocolError, "protocol major mismatch"):
            client._verify_response({**base, "protocol_version": 99}, "validate", "request-1")
        with self.assertRaisesRegex(engine_client.EngineProtocolError, "product version mismatch"):
            client._verify_response({**base, "engine_version": "0.0.0"}, "validate", "request-1")
        with self.assertRaisesRegex(engine_client.EngineProtocolError, "identity mismatch"):
            client._verify_response(base, "validate", "request-2")

    def test_public_validate_uses_engine_without_legacy_fallback(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            response = {
                "ok": True,
                "result": {"valid": True, "error_count": 0, "warning_count": 0, "diagnostics": []},
                "diagnostics": [],
            }
            with (
                mock.patch.object(engine_client, "EngineClient") as client_type,
                mock.patch.object(validate_repo, "validate", side_effect=AssertionError("legacy fallback")),
            ):
                client_type.return_value.call.return_value = response
                self.assertEqual(0, cli.main(["validate", "--root", temp]))
                client_type.return_value.call.assert_called_once_with(
                    "validate", root=Path(temp).resolve()
                )

    def test_missing_engine_does_not_fallback_to_python(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            with (
                mock.patch.object(
                    engine_client,
                    "locate_engine",
                    side_effect=engine_client.EngineUnavailableError("missing test engine"),
                ),
                mock.patch.object(validate_repo, "validate", side_effect=AssertionError("legacy fallback")),
            ):
                self.assertEqual(1, cli.main(["validate", "--root", temp]))

    def test_work_commands_send_typed_intents_to_engine(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            response = {
                "ok": True,
                "result": {
                    "success": True,
                    "changed_paths": ["work/proposed/001-probe/work-item.json"],
                    "diagnostics": [],
                },
                "diagnostics": [],
            }
            with mock.patch.object(engine_client, "EngineClient") as client_type:
                client_type.return_value.call.return_value = response
                self.assertEqual(0, cli.main(["work", "propose", "Probe", "--root", str(root)]))
                call = client_type.return_value.call.call_args
                operation = call.args[0]
                kwargs = call.kwargs
                self.assertEqual("work.propose", operation)
                self.assertEqual(root.resolve(), kwargs["root"])
                self.assertEqual("Probe", kwargs["params"]["title"])

    def test_engine_client_does_not_parse_human_output(self) -> None:
        client = object.__new__(engine_client.EngineClient)
        response = client._decode_response(
            '{"protocol":"repopact-engine","protocol_version":1,"request_id":"r",'
            + f'"engine_version":"{engine_client.expected_product_version()}","ok":true}}',
            "human stderr is not semantic output",
            "r",
        )
        self.assertTrue(response["ok"])


if __name__ == "__main__":
    unittest.main()
