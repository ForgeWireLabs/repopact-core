from __future__ import annotations

import hashlib
import json
import sys
import tarfile
import tempfile
import unittest
import zipfile
from io import BytesIO
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from generate_engine_semantic_identity import (  # noqa: E402
    _pyproject_projection,
    semantic_identity,
    workspace_manifest_projection,
)
from verify_engine_package_identity import ArchiveIdentityError, verify_sdist, verify_wheel  # noqa: E402


class EnginePackageIdentityTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.pyproject = b"""[build-system]
requires = [\"maturin>=1.15,<2\"]
build-backend = \"maturin\"

[project]
dependencies = [\"jsonschema>=4.20\"]
requires-python = \">=3.11\"

[project.optional-dependencies]
enforcement = [\"cryptography>=43\"]

[tool.maturin]
include = [\"build/semantic-input-manifest.json\"]
"""
        self.policy = {
            "format_version": 1,
            "pyproject_projection": {"runtime_optional_extras": ["enforcement"]},
            "rust": {
                "workspace_manifest_projection_exclude_fields": [
                    "members",
                    "exclude",
                    "default-members",
                ]
            },
        }
        projection_root = self.root / "projection"
        projection_root.mkdir()
        (projection_root / "pyproject.toml").write_bytes(self.pyproject)
        projection = _pyproject_projection(
            projection_root, self.policy["pyproject_projection"]
        )
        workspace_manifest = (
            '[workspace]\nresolver = "2"\nmembers = ["seed", "unused"]\n'
            '[workspace.dependencies]\nserde = "1"\n'
        )
        self.files = {
            "pyproject.toml": self.pyproject,
            "sample.txt": b"semantic input\n",
            "repopact/requirements-runtime.lock": b"jsonschema==4.26.0\n",
            "repopact/runtime-dependency-groups.json": b'{"format_version":1}\n',
            "scripts/generate_engine_semantic_identity.py": b"# generator\n",
            "scripts/verify_engine_package_identity.py": b"# verifier\n",
            "rust/Cargo.toml": workspace_manifest.encode(),
            "rust/Cargo.lock": b"# locked\n",
            "conformance/manifest.json": b'{"cases":[]}\n',
        }
        inputs = [
            {"path": name, "sha256": hashlib.sha256(content).hexdigest()}
            for name, content in self.files.items()
            if name not in {"pyproject.toml", "rust/Cargo.toml"}
        ]
        inputs.append(
            {
                "path": "@projection/pyproject-semantic-build-config.json",
                "sha256": hashlib.sha256(projection).hexdigest(),
            }
        )
        workspace_projection = workspace_manifest_projection(
            workspace_manifest,
            self.policy["rust"]["workspace_manifest_projection_exclude_fields"],
        )
        inputs.append(
            {
                "path": "@projection/rust-workspace-manifest.json",
                "sha256": hashlib.sha256(workspace_projection).hexdigest(),
            }
        )
        self.identity = semantic_identity(inputs)
        self.identity_document = {
            "format_version": 1,
            "semantic_identity": self.identity,
            "manifest": "repopact/_semantic-input-manifest.json",
        }
        self.manifest = {
            "format_version": 1,
            "hash_algorithm": "sha256",
            "semantic_identity": self.identity,
            "inputs": inputs,
        }
        self.files["build/semantic-input-policy.json"] = json.dumps(self.policy).encode()
        self.files["build/semantic-input-manifest.json"] = json.dumps(self.manifest).encode()
        self.files["repopact/_semantic-input-manifest.json"] = json.dumps(self.manifest).encode()
        self.files["repopact/_semantic_identity.json"] = json.dumps(self.identity_document).encode()

    def tearDown(self) -> None:
        self.temp.cleanup()

    def _sdist(self, name: str = "repopact-3.1.4.dev0.tar.gz") -> Path:
        path = self.root / name
        prefix = name.removesuffix(".tar.gz") + "/"
        with tarfile.open(path, "w:gz") as archive:
            for relative, data in self.files.items():
                member = tarfile.TarInfo(prefix + relative)
                member.size = len(data)
                archive.addfile(member, BytesIO(data))
        return path

    def _wheel(self, name: str = "repopact-3.1.4.dev0-py3-none-win_amd64.whl") -> Path:
        path = self.root / name
        with zipfile.ZipFile(path, "w") as archive:
            for relative in (
                "repopact/_semantic_identity.json",
                "repopact/_semantic-input-manifest.json",
                "repopact/requirements-runtime.lock",
                "repopact/runtime-dependency-groups.json",
            ):
                archive.writestr(relative, self.files[relative])
        return path

    def test_sdist_checks_every_semantic_input_and_identity(self) -> None:
        report = verify_sdist(self._sdist(), self.identity)
        self.assertEqual(self.identity, report["semantic_identity"])
        self.assertEqual(len(self.manifest["inputs"]), report["inputs_verified"])

    def test_sdist_rejects_missing_semantic_input(self) -> None:
        self.files.pop("sample.txt")
        with self.assertRaisesRegex(ArchiveIdentityError, "omitted semantic input sample.txt"):
            verify_sdist(self._sdist(), self.identity)

    def test_sdist_accepts_maturin_workspace_membership_pruning(self) -> None:
        self.files["rust/Cargo.toml"] = (
            '[workspace]\nresolver = "2"\nmembers = ["seed"]\n'
            '[workspace.dependencies]\nserde = "1"\n'
        ).encode()
        report = verify_sdist(self._sdist(), self.identity)
        self.assertEqual(self.identity, report["semantic_identity"])

    def test_wheel_requires_agreeing_identity_manifest_and_runtime_lock(self) -> None:
        report = verify_wheel(self._wheel(), self.identity)
        self.assertEqual(self.identity, report["semantic_identity"])

    def test_wheel_rejects_identity_drift(self) -> None:
        original = self.files["repopact/_semantic_identity.json"]
        document = dict(self.identity_document, semantic_identity="repopact-semantic-v1:sha256:" + "0" * 64)
        self.files["repopact/_semantic_identity.json"] = json.dumps(document).encode()
        with self.assertRaisesRegex(ArchiveIdentityError, "wheel identity differs"):
            verify_wheel(self._wheel(), self.identity)
        self.files["repopact/_semantic_identity.json"] = original


if __name__ == "__main__":
    unittest.main()
