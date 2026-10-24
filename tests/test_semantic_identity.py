from __future__ import annotations

import hashlib
import json
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts import generate_engine_semantic_identity as identity


POLICY = {
    "format_version": 1,
    "python": {
        "root": "repopact",
        "extensions": [".py"],
        "resource_roots": ["repopact/schemas", "repopact/templates"],
        "ignored_directory_names": ["__pycache__"],
    },
    "rust": {
        "workspace_manifest": "rust/Cargo.toml",
        "seed_package": "repopact-engine",
        "source_extension": ".rs",
        "ignored_directory_names": ["target", ".git", "__pycache__"],
    },
    "conformance_root": "conformance",
    "fixed_files": [
        ".gitattributes",
        "SPEC.md",
        "repopact/requirements-runtime.lock",
        "repopact/runtime-dependency-groups.json",
        "scripts/generate_engine_semantic_identity.py",
        "scripts/verify_engine_package_identity.py",
    ],
    "pyproject_projection": {"runtime_optional_extras": ["enforcement"]},
    "outputs": {
        "manifest": "build/semantic-input-manifest.json",
        "package_manifest": "repopact/_semantic-input-manifest.json",
        "identity": "repopact/_semantic_identity.json",
    },
}


class SemanticIdentityManifestTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self._write("build/semantic-input-policy.json", json.dumps(POLICY, indent=2) + "\n")
        self._write(".gitattributes", "*.py text eol=lf\n")
        self._write("SPEC.md", "protocol and semantic contract\n")
        self._write("repopact/requirements-runtime.lock", "jsonschema==4.26.0 --hash=sha256:abc\n")
        self._write("repopact/runtime-dependency-groups.json", '{"format_version":1}\n')
        self._write("scripts/generate_engine_semantic_identity.py", "# generator source\n")
        self._write("scripts/verify_engine_package_identity.py", "# verifier source\n")
        self._write(
            "pyproject.toml",
            """[build-system]
requires = ["maturin==1.15.0"]
build-backend = "maturin"

[project]
name = "repopact"
version = "9.8.7"
requires-python = ">=3.11"
dependencies = ["jsonschema>=4.20"]

[project.optional-dependencies]
enforcement = ["cryptography>=43"]
dev = ["pytest>=8"]

[tool.maturin]
include = ["repopact/_semantic_identity.json"]
""",
        )
        self._write("repopact/client.py", "def operation(): return True\n")
        self._write("repopact/schemas/contract.json", '{"type":"object"}\n')
        self._write("repopact/templates/work-item.json", "{}\n")
        self._write(
            "rust/Cargo.toml",
            """[workspace]
resolver = "2"
members = ["apps/repopact-engine", "crates/helper", "crates/unused"]

[workspace.dependencies]
helper = { path = "crates/helper" }
""",
        )
        self._write("rust/Cargo.lock", "# version = 4\n")
        self._write(
            "rust/apps/repopact-engine/Cargo.toml",
            """[package]
name = "repopact-engine"
version = "0.1.0"

[dependencies]
helper = { workspace = true }
""",
        )
        self._write("rust/apps/repopact-engine/src/main.rs", "fn main() { helper::load(); }\n")
        self._write(
            "rust/crates/helper/Cargo.toml",
            """[package]
name = "helper"
version = "0.1.0"
""",
        )
        self._write(
            "rust/crates/helper/src/lib.rs",
            'pub fn load() { let _ = include_str!("../fixtures/input.json"); }\n',
        )
        self._write("rust/crates/helper/fixtures/input.json", '{"fixture":true}\n')
        self._write("rust/crates/unused/Cargo.toml", '[package]\nname = "unused"\nversion = "0.1.0"\n')
        self._write("rust/crates/unused/src/lib.rs", "pub fn unused() {}\n")
        self._write("conformance/manifest.json", '{"cases":[]}\n')
        self._write("conformance/fixtures/valid/case.json", '{"valid":true}\n')

    def tearDown(self) -> None:
        self.temp.cleanup()

    def _write(self, relative: str, content: str) -> Path:
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8", newline="\n")
        return path

    def _input_map(self) -> dict[str, str]:
        return {item["path"]: item["sha256"] for item in identity.discover_inputs(self.root)}

    def test_known_answer_uses_canonical_path_and_digest_encoding(self) -> None:
        entries = [
            {"path": "a", "sha256": hashlib.sha256(b"A").hexdigest()},
            {"path": "b", "sha256": hashlib.sha256(b"B").hexdigest()},
        ]
        self.assertEqual(
            identity.semantic_identity(entries),
            "repopact-semantic-v1:sha256:e44c491e36092fc7ae841513ab4bef4ef8909aa812647979f5842aebb235a61d",
        )

    def test_input_order_does_not_change_identity(self) -> None:
        entries = [
            {"path": "a", "sha256": hashlib.sha256(b"A").hexdigest()},
            {"path": "b", "sha256": hashlib.sha256(b"B").hexdigest()},
        ]
        self.assertEqual(identity.semantic_identity(entries), identity.semantic_identity(list(reversed(entries))))

    def test_discovers_transitive_local_crates_and_compile_time_includes(self) -> None:
        paths = set(self._input_map())
        self.assertIn("rust/apps/repopact-engine/src/main.rs", paths)
        self.assertIn("rust/crates/helper/src/lib.rs", paths)
        self.assertIn("rust/crates/helper/fixtures/input.json", paths)
        self.assertNotIn("rust/crates/unused/src/lib.rs", paths)
        self.assertIn("conformance/fixtures/valid/case.json", paths)
        self.assertNotIn("build/semantic-input-manifest.json", paths)
        self.assertNotIn("repopact/_semantic-input-manifest.json", paths)
        self.assertNotIn("repopact/_semantic_identity.json", paths)

    def test_new_and_removed_governed_files_change_membership(self) -> None:
        before = self._input_map()
        self._write("repopact/new_contract.py", "def new_rule(): return False\n")
        with_new = self._input_map()
        self.assertIn("repopact/new_contract.py", with_new)
        self.assertNotEqual(identity.semantic_identity([
            {"path": path, "sha256": digest} for path, digest in before.items()
        ]), identity.semantic_identity([
            {"path": path, "sha256": digest} for path, digest in with_new.items()
        ]))
        (self.root / "repopact/client.py").unlink()
        after_removal = self._input_map()
        self.assertNotIn("repopact/client.py", after_removal)

    def test_pyproject_product_version_is_excluded_but_packaging_config_is_included(self) -> None:
        before = identity.render_outputs(self.root)
        project = self.root / "pyproject.toml"
        original = project.read_text(encoding="utf-8")
        project.write_text(original.replace('version = "9.8.7"', 'version = "9.8.8"'), encoding="utf-8")
        self.assertEqual(before, identity.render_outputs(self.root))
        project.write_text(
            project.read_text(encoding="utf-8").replace(
                'include = ["repopact/_semantic_identity.json"]',
                'include = ["repopact/_semantic_identity.json", "requirements-runtime.lock"]',
            ),
            encoding="utf-8",
        )
        self.assertNotEqual(before, identity.render_outputs(self.root))

    def test_check_detects_drift_without_rewriting_outputs(self) -> None:
        identity.update(self.root)
        manifest_path = self.root / POLICY["outputs"]["manifest"]
        identity_path = self.root / POLICY["outputs"]["identity"]
        original_manifest = manifest_path.read_bytes()
        original_identity = identity_path.read_bytes()
        self._write("repopact/new_contract.py", "def new_rule(): return False\n")
        with self.assertRaisesRegex(identity.ManifestError, "stale"):
            identity.check(self.root)
        self.assertEqual(original_manifest, manifest_path.read_bytes())
        self.assertEqual(original_identity, identity_path.read_bytes())

    def test_explicit_update_writes_a_checkable_baseline(self) -> None:
        identity.update(self.root)
        self._write("conformance/fixtures/valid/added.json", '{"added":true}\n')
        identity.update(self.root)
        identity.check(self.root)

    def test_dynamic_compile_time_include_fails_closed(self) -> None:
        source = self.root / "rust/crates/helper/src/lib.rs"
        source.write_text('include_str!(concat!("file", ".json"));\n', encoding="utf-8")
        with self.assertRaisesRegex(identity.ManifestError, "non-literal"):
            identity.discover_inputs(self.root)


if __name__ == "__main__":
    unittest.main()
