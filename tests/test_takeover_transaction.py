from __future__ import annotations

import tempfile
import unittest
from pathlib import Path

from repopact import takeover


class FakeEngine:
    def __init__(self, responses: list[dict]):
        self.responses = iter(responses)
        self.calls: list[tuple[str, dict]] = []

    def call(self, operation: str, **kwargs) -> dict:
        self.calls.append((operation, kwargs))
        return next(self.responses)


class TakeoverTransactionTests(unittest.TestCase):
    def test_retirement_decision_uses_stable_rust_request_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            engine = FakeEngine(
                [
                    {"ok": True, "result": {"found": False}},
                    {
                        "ok": True,
                        "result": {
                            "success": True,
                            "changed_paths": ["decisions/0003-retire-todos-legacy-plan-directory.md"],
                            "diagnostics": [],
                        },
                    },
                ]
            )
            report = {"decisions": []}

            decision_id = takeover.write_retire_decision(
                root,
                "todos",
                item_count=4,
                sha="a" * 40,
                dry_run=False,
                report=report,
                engine=engine,
            )

            self.assertEqual("0003", decision_id)
            self.assertEqual(
                ["mutation.receipt", "record.create-markdown"],
                [operation for operation, _ in engine.calls],
            )
            request_ids = [kwargs["request_id"] for _, kwargs in engine.calls]
            self.assertEqual(request_ids[0], request_ids[1])
            self.assertTrue(request_ids[0].startswith("takeover-retire-"))
            self.assertEqual(
                ["decisions/0003-retire-todos-legacy-plan-directory.md"],
                report["decisions"],
            )
            self.assertFalse((root / "decisions").exists())

    def test_dry_run_previews_through_rust_without_creating_a_decision(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            engine = FakeEngine(
                [
                    {"ok": True, "result": {"found": False}},
                    {
                        "ok": True,
                        "result": {
                            "success": True,
                            "changed_paths": ["decisions/0004-retire-todos-legacy-plan-directory.md"],
                        },
                    },
                ]
            )
            report = {"decisions": []}

            decision_id = takeover.write_retire_decision(
                root,
                "todos",
                item_count=4,
                sha="b" * 40,
                dry_run=True,
                report=report,
                engine=engine,
            )

            self.assertEqual("0004", decision_id)
            self.assertEqual(
                ["mutation.receipt", "record.preview-markdown"],
                [operation for operation, _ in engine.calls],
            )
            self.assertFalse((root / "decisions").exists())

    def test_existing_success_receipt_is_reused_without_another_create(self) -> None:
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            engine = FakeEngine(
                [
                    {
                        "ok": True,
                        "result": {
                            "found": True,
                            "receipt": {
                                "success": True,
                                "changed_paths": ["decisions/0005-retire-todos-legacy-plan-directory.md"],
                            },
                        },
                    }
                ]
            )
            report = {"decisions": []}

            decision_id = takeover.write_retire_decision(
                root,
                "todos",
                item_count=4,
                sha="c" * 40,
                dry_run=False,
                report=report,
                engine=engine,
            )

            self.assertEqual("0005", decision_id)
            self.assertEqual(["mutation.receipt"], [op for op, _ in engine.calls])
            self.assertEqual(
                ["decisions/0005-retire-todos-legacy-plan-directory.md"],
                report["decisions"],
            )


if __name__ == "__main__":
    unittest.main()
