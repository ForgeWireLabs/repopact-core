from __future__ import annotations

import json
import tempfile
import unittest
from importlib.metadata import PackageNotFoundError
from pathlib import Path
from unittest import mock

from repopact.runtime_dependencies import RuntimeDependencyError, verify_runtime_dependencies


class RuntimeDependencyLockTests(unittest.TestCase):
    def write_fixture(self, root: Path) -> None:
        package = root / "repopact"
        package.mkdir()
        (package / "runtime-dependency-groups.json").write_text(
            json.dumps(
                {
                    "format_version": 1,
                    "lock": "requirements-runtime.lock",
                    "groups": {
                        "core": ["jsonschema", "attrs"],
                        "enforcement": ["cryptography"],
                    },
                }
            ),
            encoding="utf-8",
        )
        (package / "requirements-runtime.lock").write_text(
            "attrs==26.1.0 \\\n"
            "    --hash=sha256:abc\n"
            "jsonschema==4.26.0 \\\n"
            "    --hash=sha256:def\n"
            "cryptography==50.0.2 \\\n"
            "    --hash=sha256:ghi\n",
            encoding="utf-8",
        )

    def test_core_group_checks_exact_installed_versions(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(root)
            with mock.patch(
                "repopact.runtime_dependencies.importlib.metadata.version",
                side_effect=lambda name: {"attrs": "26.1.0", "jsonschema": "4.26.0"}[name],
            ):
                result = verify_runtime_dependencies(root=root)
            self.assertEqual(result, {"attrs": "26.1.0", "jsonschema": "4.26.0"})

    def test_enforcement_group_checks_optional_exact_pin(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(root)
            with mock.patch(
                "repopact.runtime_dependencies.importlib.metadata.version",
                side_effect=lambda name: {
                    "attrs": "26.1.0",
                    "jsonschema": "4.26.0",
                    "cryptography": "50.0.1",
                }[name],
            ):
                with self.assertRaisesRegex(RuntimeDependencyError, "cryptography.*expected 50.0.2"):
                    verify_runtime_dependencies(enforcement=True, root=root)

    def test_unclassified_lock_addition_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(root)
            lock = root / "repopact" / "requirements-runtime.lock"
            lock.write_text(lock.read_text() + "extra-package==1.2.3\n", encoding="utf-8")
            with self.assertRaisesRegex(RuntimeDependencyError, "unclassified"):
                verify_runtime_dependencies(root=root)

    def test_missing_runtime_distribution_fails_closed(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(root)

            def version(name: str) -> str:
                if name == "attrs":
                    raise PackageNotFoundError(name)
                return "4.26.0"

            with mock.patch(
                "repopact.runtime_dependencies.importlib.metadata.version",
                side_effect=version,
            ):
                with self.assertRaisesRegex(RuntimeDependencyError, "attrs: missing"):
                    verify_runtime_dependencies(root=root)


if __name__ == "__main__":
    unittest.main()
