from __future__ import annotations

import json
import shutil
import tempfile
import unittest
from contextlib import redirect_stdout
from io import StringIO
from pathlib import Path

from repopact.documentation_mappings import _paths_digest, compute_snapshot, inspect
from repopact.doctor import diagnose
from repopact.cli import main as cli_main
from repopact.validate_repo import validate
from unittest.mock import patch


class DocumentationMappingTests(unittest.TestCase):
    def make_repository(self, name: str = "neutral") -> Path:
        fixture = Path(__file__).resolve().parents[1] / "conformance" / "fixtures" / "valid"
        root = Path(tempfile.mkdtemp(prefix=f"repopact-doc-mapping-{name}-"))
        shutil.copytree(fixture, root, dirs_exist_ok=True)
        (root / "src").mkdir()
        (root / "docs").mkdir(exist_ok=True)
        (root / "src" / "api.py").write_text("VERSION = 1\n", encoding="utf-8")
        (root / "docs" / "api.md").write_text("Version 1.\n", encoding="utf-8")
        owners_path = root / "governance" / "owners.json"
        owners = json.loads(owners_path.read_text(encoding="utf-8"))
        owners["documentation_mappings"] = {
            "enabled": True,
            "path": "governance/documentation-mappings.json",
        }
        owners_path.write_text(json.dumps(owners, indent=2) + "\n", encoding="utf-8")
        record = {
            "$schema": "documentation-mappings.schema.json",
            "version": 1,
            "mappings": [{
                "id": "api-contract",
                "kind": "manual",
                "source_paths": ["src/api.py"],
                "documentation_paths": ["docs/api.md"],
                "description": "Synthetic neutral source-to-guide mapping.",
                "snapshot": {
                    "source_digest": _paths_digest(root, ["src/api.py"]),
                    "documentation_digest": _paths_digest(root, ["docs/api.md"]),
                },
            }],
        }
        (root / "governance" / "documentation-mappings.json").write_text(
            json.dumps(record, indent=2) + "\n", encoding="utf-8"
        )
        return root

    def test_matching_pair_passes_and_snapshot_is_read_only(self) -> None:
        root = self.make_repository()
        before = (root / "governance" / "documentation-mappings.json").read_bytes()
        result = compute_snapshot(root, "api-contract")
        self.assertEqual(result["source_digest"], _paths_digest(root, ["src/api.py"]))
        self.assertFalse(inspect(root))
        self.assertEqual(before, (root / "governance" / "documentation-mappings.json").read_bytes())
        shutil.rmtree(root)

    def test_source_only_change_is_reported_by_python_compatibility_validator(self) -> None:
        root = self.make_repository("stale")
        (root / "src" / "api.py").write_text("VERSION = 2\n", encoding="utf-8")
        self.assertTrue(any(code == "documentation.mapping-stale" for code, _, _ in inspect(root)))
        problems = validate(root)
        self.assertTrue(any("mapped source changed" in problem.message for problem in problems))
        shutil.rmtree(root)

    def test_updated_pair_still_requires_snapshot_review(self) -> None:
        root = self.make_repository("pair")
        (root / "src" / "api.py").write_text("VERSION = 2\n", encoding="utf-8")
        (root / "docs" / "api.md").write_text("Version 2.\n", encoding="utf-8")
        self.assertTrue(any(code == "documentation.mapping-review-required" for code, _, _ in inspect(root)))
        shutil.rmtree(root)

    def test_doctor_surfaces_drift_without_changing_records(self) -> None:
        root = self.make_repository("doctor")
        (root / "src" / "api.py").write_text("VERSION = 2\n", encoding="utf-8")
        before = (root / "governance" / "documentation-mappings.json").read_bytes()
        findings = diagnose(root)
        self.assertTrue(any(item.code == "documentation.mapping-stale" and item.severity == "warn" for item in findings))
        self.assertEqual(before, (root / "governance" / "documentation-mappings.json").read_bytes())
        shutil.rmtree(root)

    def test_snapshot_cli_uses_the_canonical_read_only_engine_operation(self) -> None:
        root = self.make_repository("cli")
        with patch("repopact.engine_client.EngineClient.call", return_value={"result": {"source_digest": "a" * 64, "documentation_digest": "b" * 64}}) as call:
            output = StringIO()
            with redirect_stdout(output):
                status = cli_main(["documentation", "snapshot", "api-contract", "--root", str(root)])
        self.assertEqual(status, 0)
        self.assertIn("source_digest", output.getvalue())
        call.assert_called_once_with("documentation.snapshot", root=root.resolve(), params={"mapping_id": "api-contract"})
        shutil.rmtree(root)


if __name__ == "__main__":
    unittest.main()
