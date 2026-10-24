from __future__ import annotations

import json
import tempfile
import unittest
from importlib.metadata import PackageNotFoundError
from pathlib import Path
from unittest import mock

from repopact import runtime_dependencies
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

    def test_python_full_version_lock_marker_is_enforced_only_below_313(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            self.write_fixture(root)
            groups_path = root / "repopact" / "runtime-dependency-groups.json"
            groups = json.loads(groups_path.read_text(encoding="utf-8"))
            groups["groups"]["core"].append("typing-extensions")
            groups_path.write_text(json.dumps(groups), encoding="utf-8")
            lock = root / "repopact" / "requirements-runtime.lock"
            lock.write_text(
                lock.read_text(encoding="utf-8")
                + "typing-extensions==4.16.0 ; python_full_version < '3.13' \\\n"
                + "    --hash=sha256:abc\n",
                encoding="utf-8",
            )

            with mock.patch.object(runtime_dependencies.sys, "version_info", (3, 12, 4)):
                with mock.patch(
                    "repopact.runtime_dependencies.importlib.metadata.version",
                    side_effect=lambda name: {
                        "attrs": "26.1.0",
                        "jsonschema": "4.26.0",
                        "typing-extensions": "4.16.0",
                    }[name],
                ):
                    installed = verify_runtime_dependencies(root=root)
            self.assertEqual(installed["typing-extensions"], "4.16.0")

            with mock.patch.object(runtime_dependencies.sys, "version_info", (3, 13, 0)):
                with mock.patch(
                    "repopact.runtime_dependencies.importlib.metadata.version",
                    side_effect=lambda name: {
                        "attrs": "26.1.0",
                        "jsonschema": "4.26.0",
                    }[name],
                ):
                    installed = verify_runtime_dependencies(root=root)
            self.assertNotIn("typing-extensions", installed)


if __name__ == "__main__":
    unittest.main()
