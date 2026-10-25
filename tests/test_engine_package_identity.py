from __future__ import annotations

import base64
import csv
import hashlib
import io
import json
import os
import shutil
import stat
import sys
import tarfile
import tempfile
import unittest
import uuid
import zipfile
from dataclasses import replace
from io import BytesIO
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
SCRIPTS = ROOT / "scripts"
if str(SCRIPTS) not in sys.path:
    sys.path.insert(0, str(SCRIPTS))

from generate_engine_semantic_identity import update as update_semantic_identity  # noqa: E402
from verify_engine_package_identity import (  # noqa: E402
    ArchiveIdentityError,
    PackageSource,
    _EXPECTED_MATURIN_INCLUDE,
    _source_package,
    verify_archives,
    verify_sdist,
    verify_wheel,
)


SOURCE_REVISION = "a" * 40


def _metadata(source: PackageSource, *, version: str | None = None) -> bytes:
    project = source.project["project"]
    product_version = version or source.version
    lines = [
        "Metadata-Version: 2.4",
        f"Name: {project['name']}",
        f"Version: {product_version}",
        f"Summary: {project['description']}",
        f"License-Expression: {project['license']}",
        f"Requires-Python: {project['requires-python']}",
        "Description-Content-Type: text/markdown; charset=UTF-8; variant=GFM",
    ]
    for requirement in project.get("dependencies", []):
        lines.append(f"Requires-Dist: {requirement}")
    for extra, requirements in project.get("optional-dependencies", {}).items():
        for requirement in requirements:
            lines.append(f"Requires-Dist: {requirement} ; extra == '{extra}'")
    for extra in project.get("optional-dependencies", {}):
        lines.append(f"Provides-Extra: {extra}")
    lines.append("License-File: LICENSE")
    for name, value in project.get("urls", {}).items():
        lines.append(f"Project-URL: {name}, {value}")
    return ("\n".join(lines) + "\n\n" + source.source_files["README.md"].decode("utf-8")).encode()


def _sbom(source: PackageSource) -> bytes:
    root_package = source.cargo_packages[source.cargo_root_id]
    references = {
        package_id: (
            "engine-root"
            if package_id == source.cargo_root_id
            else f"component-{source.cargo_packages[package_id]['name']}"
        )
        for package_id in source.cargo_closure
    }
    components = [
        {
            "type": "library",
            "name": source.cargo_packages[package_id]["name"],
            "version": source.cargo_packages[package_id]["version"],
            "bom-ref": references[package_id],
        }
        for package_id in sorted(source.cargo_closure)
        if package_id != source.cargo_root_id
    ]
    dependency_entries = []
    for package_id in sorted(source.cargo_closure):
        dependency_entries.append(
            {
                "ref": references[package_id],
                "dependsOn": [
                    references[dependency["pkg"]]
                    for dependency in source.cargo_nodes[package_id].get("deps", [])
                    if dependency["pkg"] in references
                ],
            }
        )
    document = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "serialNumber": "urn:uuid:" + str(uuid.UUID(int=1)),
        "version": 1,
        "metadata": {
            "component": {
                "type": "application",
                "name": root_package["name"],
                "version": root_package["version"],
                "bom-ref": "engine-root",
                "components": [
                    {
                        "type": "application",
                        "name": name,
                        "version": root_package["version"],
                        "bom-ref": f"target-{name}",
                    }
                    for name in ("repopact", "repopact-engine")
                ],
            }
        },
        "components": components,
        "dependencies": dependency_entries,
    }
    return (json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n").encode()


def _record_bytes(contents: dict[str, bytes], record_path: str) -> bytes:
    output = io.StringIO(newline="")
    writer = csv.writer(output, lineterminator="\n")
    for name, content in sorted(contents.items()):
        digest = base64.urlsafe_b64encode(hashlib.sha256(content).digest()).rstrip(b"=").decode()
        writer.writerow((name, f"sha256={digest}", str(len(content))))
    writer.writerow((record_path, "", ""))
    return output.getvalue().encode()


def _wheel_contents(source: PackageSource, trusted_dir: Path) -> tuple[dict[str, bytes], str, str, str]:
    dist_info = f"repopact-{source.version}.dist-info"
    data_dir = f"repopact-{source.version}.data"
    record_path = f"{dist_info}/RECORD"
    contents = dict(source.package_files)
    contents[f"{data_dir}/scripts/repopact-engine.exe"] = (
        trusted_dir / "release" / "repopact-engine.exe"
    ).read_bytes()
    contents[f"{data_dir}/scripts/repopact.exe"] = (
        trusted_dir / "release" / "repopact.exe"
    ).read_bytes()
    contents[f"{dist_info}/METADATA"] = _metadata(source)
    contents[f"{dist_info}/WHEEL"] = (
        b"Wheel-Version: 1.0\n"
        b"Generator: maturin (1.15.0)\n"
        b"Root-Is-Purelib: false\n"
        b"Tag: py3-none-win_amd64\n"
    )
    contents[f"{dist_info}/licenses/LICENSE"] = source.source_files["LICENSE"]
    contents[f"{dist_info}/sboms/repopact-engine.cyclonedx.json"] = _sbom(source)
    contents[record_path] = _record_bytes(contents, record_path)
    return contents, dist_info, data_dir, record_path


class SyntheticPackageArchiveTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name) / "source"
        self.root.mkdir()
        self._make_source_tree()
        metadata_path = self.root / "target" / "cargo-metadata.json"
        metadata_path.parent.mkdir(parents=True)
        engine_manifest = self.root / "rust" / "apps" / "repopact-engine" / "Cargo.toml"
        protocol_manifest = self.root / "rust" / "crates" / "repopact-protocol" / "Cargo.toml"
        engine_id = "engine-id"
        protocol_id = "protocol-id"
        cargo_metadata = {
            "workspace_root": str(self.root / "rust"),
            "target_directory": str(self.root / "target"),
            "packages": [
                {
                    "id": engine_id,
                    "name": "repopact-engine",
                    "version": "0.1.0",
                    "source": None,
                    "manifest_path": str(engine_manifest),
                },
                {
                    "id": protocol_id,
                    "name": "repopact-protocol",
                    "version": "0.1.0",
                    "source": None,
                    "manifest_path": str(protocol_manifest),
                },
            ],
            "resolve": {
                "nodes": [
                    {
                        "id": engine_id,
                        "deps": [
                            {
                                "pkg": protocol_id,
                                "dep_kinds": [{"kind": None, "target": None}],
                            }
                        ],
                    },
                    {"id": protocol_id, "deps": []},
                ]
            },
        }
        metadata_path.write_text(json.dumps(cargo_metadata), encoding="utf-8")
        target = self.root / "target" / "release"
        target.mkdir(parents=True)
        (target / "repopact-engine.exe").write_bytes(b"trusted engine bytes")
        (target / "repopact.exe").write_bytes(b"trusted cli bytes")
        self.source = _source_package(self.root, metadata_path, SOURCE_REVISION)
        self.sdist = self.root / "repopact-3.1.4.dev0.tar.gz"
        self.wheel = self.root / "repopact-3.1.4.dev0-py3-none-win_amd64.whl"

    def tearDown(self) -> None:
        self.temp.cleanup()

    def _make_source_tree(self) -> None:
        for directory in (
            "repopact/schemas",
            "repopact/templates",
            "scripts",
            "build",
            "conformance",
            "rust",
        ):
            (self.root / directory).mkdir(parents=True, exist_ok=True)
        top = {
            ".gitattributes": b"* text=auto\n",
            "LICENSE": b"Apache License 2.0 test fixture\n",
            "README.md": b"# Test RepoPact\n\nA source-anchored package test.\n",
            "RELEASE_LABEL": b"3.1.4-dev.0\n",
            "SPEC.md": b"# Specification\n",
            "VERSION": b"3.1.4\n",
        }
        for relative, content in top.items():
            (self.root / relative).write_bytes(content)
        include_lines = []
        for path, formats in _EXPECTED_MATURIN_INCLUDE:
            format_items = ", ".join(json.dumps(value) for value in formats)
            include_lines.append(
                f"  {{ path = {json.dumps(path)}, format = [{format_items}] }},"
            )
        pyproject = """[build-system]
requires = [\"maturin>=1.15,<2\"]
build-backend = \"maturin\"

[project]
name = \"repopact\"
version = \"3.1.4.dev0\"
description = \"RepoPact test package\"
readme = \"README.md\"
license = \"Apache-2.0\"
requires-python = \">=3.11\"
dependencies = [\"jsonschema>=4.20\"]

[project.optional-dependencies]
enforcement = [\"cryptography>=43\"]

[project.urls]
Homepage = \"https://example.invalid/repopact\"
Repository = \"https://example.invalid/repopact/source\"

[tool.maturin]
bindings = \"bin\"
manifest-path = \"rust/apps/repopact-engine/Cargo.toml\"
python-source = \".\"
python-packages = [\"repopact\"]
targets = [
  { name = \"repopact-engine\", kind = \"bin\" },
  { name = \"repopact\", kind = \"bin\" },
]
include = [
""" + "\n".join(include_lines) + "\n]\n"
        (self.root / "pyproject.toml").write_text(pyproject, encoding="utf-8")
        (self.root / "repopact" / "__init__.py").write_text(
            '\"\"\"Synthetic package fixture.\"\"\"\n', encoding="utf-8"
        )
        (self.root / "repopact" / "engine_client.py").write_text(
            "PROTOCOL_VERSION = 2\n", encoding="utf-8"
        )
        shutil.copyfile(ROOT / "repopact" / "package_version.py", self.root / "repopact" / "package_version.py")
        (self.root / "repopact" / "requirements-runtime.lock").write_text(
            "jsonschema==4.26.0\n", encoding="utf-8"
        )
        (self.root / "repopact" / "runtime-dependency-groups.json").write_text(
            '{\"format_version\":1,\"groups\":{\"runtime\":[]}}\n', encoding="utf-8"
        )
        (self.root / "repopact" / "schemas" / "test.json").write_text(
            '{\"type\":\"object\"}\n', encoding="utf-8"
        )
        (self.root / "repopact" / "templates" / "test.json").write_text(
            '{\"format_version\":1}\n', encoding="utf-8"
        )
        shutil.copyfile(
            ROOT / "scripts" / "generate_engine_semantic_identity.py",
            self.root / "scripts" / "generate_engine_semantic_identity.py",
        )
        shutil.copyfile(
            ROOT / "scripts" / "verify_engine_package_identity.py",
            self.root / "scripts" / "verify_engine_package_identity.py",
        )
        (self.root / "conformance" / "manifest.json").write_text(
            '{\"format_version\":1,\"cases\":[]}\n', encoding="utf-8"
        )
        (self.root / "rust" / "Cargo.toml").write_text(
            '[workspace]\nresolver = \"2\"\nmembers = [\"apps/repopact-engine\", \"crates/repopact-protocol\"]\n',
            encoding="utf-8",
        )
        (self.root / "rust" / "Cargo.lock").write_text(
            "# This is a synthetic locked workspace for verifier tests.\n", encoding="utf-8"
        )
        engine_dir = self.root / "rust" / "apps" / "repopact-engine"
        (engine_dir / "src").mkdir(parents=True)
        (engine_dir / "Cargo.toml").write_text(
            '[package]\nname = "repopact-engine"\nversion = "0.1.0"\nedition = "2024"\n'
            '[dependencies]\nrepopact-protocol = { path = \"../../crates/repopact-protocol\" }\n',
            encoding="utf-8",
        )
        (engine_dir / "src" / "main.rs").write_text("fn main() {}\n", encoding="utf-8")
        protocol_dir = self.root / "rust" / "crates" / "repopact-protocol"
        (protocol_dir / "src").mkdir(parents=True)
        (protocol_dir / "Cargo.toml").write_text(
            '[package]\nname = "repopact-protocol"\nversion = "0.1.0"\nedition = "2024"\n',
            encoding="utf-8",
        )
        (protocol_dir / "src" / "lib.rs").write_text(
            "pub const PROTOCOL_VERSION: u32 = 2;\n", encoding="utf-8"
        )
        policy = {
            "format_version": 1,
            "python": {
                "root": "repopact",
                "extensions": [".py"],
                "resource_roots": ["repopact/schemas", "repopact/templates"],
                "ignored_directory_names": ["__pycache__"],
            },
            "rust": {
                "workspace_manifest": "rust/Cargo.toml",
                "workspace_manifest_projection_exclude_fields": [
                    "members",
                    "exclude",
                    "default-members",
                ],
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
        (self.root / "build" / "semantic-input-policy.json").write_text(
            json.dumps(policy, indent=2) + "\n", encoding="utf-8"
        )
        update_semantic_identity(self.root)

    def _sdist_entries(self) -> list[tuple[str, bytes, int, bytes | None]]:
        prefix = f"repopact-{self.source.version}/"
        entries = [
            (prefix + name, data, tarfile.REGTYPE, None)
            for name, data in sorted(self.source.source_files.items())
        ]
        entries.append((prefix + "PKG-INFO", _metadata(self.source), tarfile.REGTYPE, None))
        return entries

    def _write_sdist(
        self,
        entries: list[tuple[str, bytes, int, bytes | None]] | None = None,
        path: Path | None = None,
    ) -> Path:
        target = path or self.sdist
        with tarfile.open(target, "w:gz") as archive:
            for name, data, entry_type, linkname in entries or self._sdist_entries():
                info = tarfile.TarInfo(name)
                info.size = len(data)
                info.mode = 0o644
                info.type = entry_type
                if linkname is not None:
                    info.linkname = linkname.decode("utf-8")
                if entry_type == tarfile.SYMTYPE or entry_type == tarfile.LNKTYPE:
                    archive.addfile(info)
                else:
                    archive.addfile(info, BytesIO(data))
        return target

    def _wheel_entries(self) -> list[tuple[str, bytes, int, int]]:
        contents, _dist_info, _data_dir, _record_path = _wheel_contents(
            self.source, self.root / "target"
        )
        return [
            (name, data, stat.S_IFREG | 0o644, 3)
            for name, data in contents.items()
        ]

    def _write_wheel(
        self,
        entries: list[tuple[str, bytes, int, int]] | None = None,
        path: Path | None = None,
        *,
        refresh_record: bool = True,
    ) -> Path:
        target = path or self.wheel
        values = list(entries or self._wheel_entries())
        _contents, _dist_info, _data_dir, record_path = _wheel_contents(
            self.source, self.root / "target"
        )
        values = [(name, data, mode, system) for name, data, mode, system in values if name != record_path]
        if refresh_record:
            contents = {name: data for name, data, _mode, _system in values}
            values.append(
                (
                    record_path,
                    _record_bytes(contents, record_path),
                    stat.S_IFREG | 0o644,
                    3,
                )
            )
        with zipfile.ZipFile(target, "w") as archive:
            for name, data, mode, system in values:
                info = zipfile.ZipInfo(name)
                info.create_system = system
                info.external_attr = mode << 16
                archive.writestr(info, data)
        return target

    def _probe(self, response: dict[str, Any] | None = None):
        value = response or {
            "ok": True,
            "protocol": "repopact-engine",
            "protocol_version": self.source.protocol_version,
            "request_id": "wheel-identity-check",
            "engine_version": self.source.version,
            "semantic_identity": self.source.semantic_identity,
            "build_info": {"source_revision": self.source.source_revision},
        }
        return lambda _member, _data: value

    def test_source_anchors_current_version_protocol_and_identity(self) -> None:
        self._write_sdist()
        self._write_wheel()
        report = verify_archives(
            self.root,
            self.sdist,
            self.wheel,
            self.root / "target" / "cargo-metadata.json",
            SOURCE_REVISION,
            engine_probe=self._probe(),
        )
        self.assertEqual("3.1.4.dev0", report["product_version"])
        self.assertEqual(2, report["protocol_version"])
        self.assertGreater(report["sdist"]["semantic_inputs_verified"], 0)
        self.assertEqual(
            report["sdist"]["members_verified"],
            len(report["sdist"]["member_inventory"]),
        )
        self.assertTrue(report["source_wheel_sdist_metadata_agreement"])
        self.assertEqual(
            report["trusted_executables"]["repopact-engine.exe"]["sha256"],
            report["wheel"]["engine_sha256"],
        )

    def test_sdist_rejects_unexpected_paths_collisions_and_links(self) -> None:
        baseline = self._sdist_entries()
        prefix = f"repopact-{self.source.version}/"
        existing = next(name for name, _data, _kind, _link in baseline if name.endswith("repopact/engine_client.py"))
        cases = [
            ("unexpected file", baseline + [(prefix + "attacker.py", b"payload", tarfile.REGTYPE, None)]),
            ("parent traversal", baseline + [(prefix + "../escape", b"payload", tarfile.REGTYPE, None)]),
            ("absolute path", baseline + [("/outside", b"payload", tarfile.REGTYPE, None)]),
            ("drive path", baseline + [("C:/outside", b"payload", tarfile.REGTYPE, None)]),
            ("duplicate member", baseline + [next(item for item in baseline if item[0] == existing)]),
            (
                "normalization collision",
                baseline + [(prefix + "repopact/ENGINE_CLIENT.py", b"payload", tarfile.REGTYPE, None)],
            ),
            ("symbolic link", baseline + [(prefix + "repopact/link", b"", tarfile.SYMTYPE, b"../README.md")]),
            ("hard link", baseline + [(prefix + "repopact/hardlink", b"", tarfile.LNKTYPE, b"README.md")]),
        ]
        for label, entries in cases:
            with self.subTest(label=label):
                with self.assertRaises(ArchiveIdentityError):
                    verify_sdist(
                        self._write_sdist(entries, self.root / "case.tar.gz"),
                        self.source,
                    )

    def test_sdist_rejects_missing_and_modified_source_members(self) -> None:
        baseline = self._sdist_entries()
        package_path = f"repopact-{self.source.version}/repopact/engine_client.py"
        with self.assertRaises(ArchiveIdentityError):
            verify_sdist(
                self._write_sdist(
                    [item for item in baseline if item[0] != package_path],
                    self.root / "missing.tar.gz",
                ),
                self.source,
            )
        changed = [
            (name, data + b"# changed\n" if name == package_path else data, kind, link)
            for name, data, kind, link in baseline
        ]
        with self.assertRaisesRegex(ArchiveIdentityError, "sdist source file (size|content) mismatch"):
            verify_sdist(
                self._write_sdist(changed, self.root / "changed.tar.gz"),
                self.source,
            )

    def test_wheel_accepts_complete_closed_inventory_and_record(self) -> None:
        self._write_wheel()
        report = verify_wheel(self.wheel, self.source, engine_probe=self._probe())
        self.assertEqual(2, report["protocol_version"])
        self.assertEqual(2, report["trusted_build_outputs"].__len__())
        self.assertTrue(report["engine_identity_verified"])

    def test_wheel_rejects_extra_installation_files_and_unsafe_names(self) -> None:
        baseline = self._wheel_entries()
        cases = [
            ("extra module", ("repopact/unreviewed.py", b"payload", stat.S_IFREG | 0o644, 3)),
            (
                "unexpected purelib hook",
                (
                    f"repopact-{self.source.version}.data/purelib/attack.pth",
                    b"import os",
                    stat.S_IFREG | 0o644,
                    3,
                ),
            ),
            ("parent traversal", ("../escape", b"payload", stat.S_IFREG | 0o644, 3)),
            ("absolute path", ("/outside", b"payload", stat.S_IFREG | 0o644, 3)),
            ("drive path", ("C:/outside", b"payload", stat.S_IFREG | 0o644, 3)),
            (
                "normalization collision",
                ("repopact/ENGINE_CLIENT.py", b"payload", stat.S_IFREG | 0o644, 3),
            ),
            (
                "unexpected executable",
                (
                    f"repopact-{self.source.version}.data/scripts/unreviewed.exe",
                    b"MZ",
                    stat.S_IFREG | 0o755,
                    3,
                ),
            ),
            (
                "unexpected metadata",
                (
                    f"repopact-{self.source.version}.dist-info/entry_points.txt",
                    b"[console_scripts]\nattack=payload:main\n",
                    stat.S_IFREG | 0o644,
                    3,
                ),
            ),
        ]
        for label, entry in cases:
            with self.subTest(label=label):
                with self.assertRaises(ArchiveIdentityError):
                    self._write_wheel(
                        baseline + [entry],
                        self.wheel,
                    )
                    verify_wheel(self.root / "case.whl", self.source, engine_probe=self._probe())

        symlink = (
            "repopact/unsafe-link",
            b"../README.md",
            stat.S_IFLNK | 0o777,
            3,
        )
        with self.assertRaisesRegex(ArchiveIdentityError, "symlink or unsupported entry"):
            self._write_wheel(baseline + [symlink], self.wheel)
            verify_wheel(self.wheel, self.source, engine_probe=self._probe())

        duplicate = baseline + [next(entry for entry in baseline if entry[0] == "repopact/engine_client.py")]
        with self.assertRaisesRegex(ArchiveIdentityError, "duplicate member"):
            self._write_wheel(duplicate, self.wheel, refresh_record=False)
            verify_wheel(self.wheel, self.source, engine_probe=self._probe())

    def test_wheel_rejects_missing_or_changed_manifest_and_metadata_members(self) -> None:
        baseline = self._wheel_entries()
        member = "repopact/engine_client.py"
        with self.assertRaises(ArchiveIdentityError):
            self._write_wheel(
                [entry for entry in baseline if entry[0] != member],
                self.wheel,
            )
            verify_wheel(
                self.wheel, self.source, engine_probe=self._probe()
            )
        changed = [
            (
                name,
                bytes([data[0] ^ 0x01]) + data[1:] if name == member else data,
                mode,
                system,
            )
            for name, data, mode, system in baseline
        ]
        with self.assertRaisesRegex(ArchiveIdentityError, "wheel package source differs"):
            self._write_wheel(changed, self.wheel)
            verify_wheel(
                self.wheel, self.source, engine_probe=self._probe()
            )
        metadata_path = f"repopact-{self.source.version}.dist-info/METADATA"
        wrong_version = [
            (
                name,
                data.replace(b"Version: 3.1.4.dev0", b"Version: 3.1.3")
                if name == metadata_path
                else data,
                mode,
                system,
            )
            for name, data, mode, system in baseline
        ]
        with self.assertRaisesRegex(ArchiveIdentityError, "Version differs from the trusted source"):
            self._write_wheel(wrong_version, self.wheel)
            verify_wheel(
                self.wheel, self.source, engine_probe=self._probe()
            )

    def test_wheel_checks_both_executable_digests_before_probe(self) -> None:
        baseline = self._wheel_entries()
        engine_path = f"repopact-{self.source.version}.data/scripts/repopact-engine.exe"
        trusted_engine = next(
            data
            for name, data, _mode, _system in baseline
            if name == engine_path
        )
        malicious_payload = bytes([trusted_engine[0] ^ 0x01]) + trusted_engine[1:]
        changed = [
            (name, malicious_payload if name == engine_path else data, mode, system)
            for name, data, mode, system in baseline
        ]
        marker = self.root / "marker.txt"

        def probe(_member: str, _data: bytes) -> dict[str, Any]:
            marker.write_text("probe was called", encoding="utf-8")
            return self._probe()(_member, _data)

        with self.assertRaisesRegex(ArchiveIdentityError, "trusted Maturin build output"):
            self._write_wheel(changed, self.wheel)
            verify_wheel(
                self.wheel,
                self.source,
                engine_probe=probe,
            )
        self.assertFalse(marker.exists(), "modified candidate executable reached the probe")

        missing_engine = [entry for entry in baseline if entry[0] != engine_path]
        with self.assertRaises(ArchiveIdentityError):
            self._write_wheel(missing_engine, self.wheel)
            verify_wheel(
                self.wheel,
                self.source,
                engine_probe=self._probe(),
            )

        copied_target = self.root / "wrong-trust-target"
        shutil.copytree(self.root / "target", copied_target)
        trusted_engine = copied_target / "release" / "repopact-engine.exe"
        trusted_engine.write_bytes(b"different build output")
        wrong_trust = replace(self.source, cargo_target_directory=copied_target)
        self._write_wheel(path=self.wheel)
        with self.assertRaisesRegex(ArchiveIdentityError, "trusted Maturin build output"):
            verify_wheel(self.wheel, wrong_trust, engine_probe=self._probe())

    def test_wheel_rejects_protocol_and_semantic_handshake_mismatch(self) -> None:
        self._write_wheel()
        for label, response in (
            (
                "protocol",
                {
                    "ok": True,
                    "protocol": "repopact-engine",
                    "protocol_version": 1,
                    "request_id": "wheel-identity-check",
                    "engine_version": self.source.version,
                    "semantic_identity": self.source.semantic_identity,
                    "build_info": {"source_revision": SOURCE_REVISION},
                },
            ),
            (
                "semantic",
                {
                    "ok": True,
                    "protocol": "repopact-engine",
                    "protocol_version": 2,
                    "request_id": "wheel-identity-check",
                    "engine_version": self.source.version,
                    "semantic_identity": "repopact-semantic-v1:sha256:" + "b" * 64,
                    "build_info": {"source_revision": SOURCE_REVISION},
                },
            ),
        ):
            with self.subTest(label=label):
                with self.assertRaises(ArchiveIdentityError):
                    verify_wheel(
                        self.wheel,
                        self.source,
                        engine_probe=self._probe(response),
                    )


def _archive_wheel_entries(path: Path) -> list[tuple[str, bytes, int, int]]:
    with zipfile.ZipFile(path) as archive:
        return [
            (
                info.filename,
                archive.read(info),
                info.external_attr >> 16,
                info.create_system,
            )
            for info in archive.infolist()
        ]


def _write_wheel_copy(
    path: Path,
    destination: Path,
    entries: list[tuple[str, bytes, int, int]],
    *,
    refresh_record: bool = True,
) -> None:
    record_path = next(
        (name for name, _data, _mode, _system in entries if name.endswith(".dist-info/RECORD")),
        None,
    )
    if refresh_record and record_path is not None:
        entries = [entry for entry in entries if entry[0] != record_path]
        contents = {name: data for name, data, _mode, _system in entries}
        record = _record_bytes(contents, record_path)
        entries.append((record_path, record, stat.S_IFREG | 0o644, 3))
    with zipfile.ZipFile(destination, "w") as archive:
        for name, data, mode, system in entries:
            info = zipfile.ZipInfo(name)
            info.create_system = system
            info.external_attr = mode << 16
            archive.writestr(info, data)


def _archive_sdist_entries(path: Path) -> list[tuple[str, bytes, int, str, dict[str, str]]]:
    with tarfile.open(path, "r:gz") as archive:
        return [
            (
                member.name,
                archive.extractfile(member).read() if member.isfile() else b"",
                member.mode,
                member.type,
                dict(member.pax_headers),
            )
            for member in archive.getmembers()
        ]


def _write_sdist_copy(
    destination: Path,
    entries: list[tuple[str, bytes, int, str, dict[str, str]]],
) -> None:
    with tarfile.open(destination, "w:gz") as archive:
        for name, data, mode, entry_type, pax_headers in entries:
            info = tarfile.TarInfo(name)
            info.mode = mode
            info.type = entry_type
            info.pax_headers = pax_headers
            info.size = len(data)
            if entry_type in (tarfile.SYMTYPE, tarfile.LNKTYPE):
                info.linkname = data.decode("utf-8", errors="replace")
                archive.addfile(info)
            else:
                archive.addfile(info, BytesIO(data))


@unittest.skipUnless(
    all(
        os.environ.get(name)
        for name in (
            "REPOPACT_TEST_SDIST",
            "REPOPACT_TEST_WHEEL",
            "REPOPACT_TEST_CARGO_METADATA",
            "REPOPACT_TEST_SOURCE_REVISION",
        )
    ),
    "actual Maturin archive paths are supplied by the focused build run",
)
class ActualMaturinArchiveRegressionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.sdist = Path(os.environ["REPOPACT_TEST_SDIST"]).resolve()
        cls.wheel = Path(os.environ["REPOPACT_TEST_WHEEL"]).resolve()
        cls.cargo_metadata = Path(os.environ["REPOPACT_TEST_CARGO_METADATA"]).resolve()
        cls.source_revision = os.environ["REPOPACT_TEST_SOURCE_REVISION"]
        cls.source = _source_package(ROOT, cls.cargo_metadata, cls.source_revision)
        cls.temp = tempfile.TemporaryDirectory(prefix="repopact-actual-archive-attacks-")
        cls.temp_root = Path(cls.temp.name)

    @classmethod
    def tearDownClass(cls) -> None:
        cls.temp.cleanup()

    def test_real_maturin_archives_pass_and_attack_copies_fail_closed(self) -> None:
        report = verify_archives(
            ROOT,
            self.sdist,
            self.wheel,
            self.cargo_metadata,
            self.source_revision,
        )
        self.assertEqual(273, report["sdist"]["members_verified"])
        self.assertEqual(84, report["wheel"]["member_count"])
        self.assertTrue(report["source_wheel_sdist_metadata_agreement"])

        base_wheel = _archive_wheel_entries(self.wheel)
        wheel_name_set = {entry[0] for entry in base_wheel}
        engine_path = next(name for name in wheel_name_set if name.endswith(".data/scripts/repopact-engine.exe"))
        metadata_path = next(name for name in wheel_name_set if name.endswith(".dist-info/METADATA"))
        record_path = next(name for name in wheel_name_set if name.endswith(".dist-info/RECORD"))
        probe_marker = self.temp_root / "probe-called.txt"
        trusted_engine_bytes = next(data for name, data, _mode, _system in base_wheel if name == engine_path)
        marker_payload = bytes([trusted_engine_bytes[0] ^ 0x01]) + trusted_engine_bytes[1:]

        def should_not_probe(_member: str, _data: bytes) -> dict[str, Any]:
            probe_marker.write_text("called", encoding="utf-8")
            raise AssertionError("candidate archive reached executable probe")

        def rejected_wheel(
            label: str,
            entries: list[tuple[str, bytes, int, int]],
            *,
            refresh_record: bool = True,
        ) -> None:
            target = self.temp_root / f"{label}.whl"
            _write_wheel_copy(self.wheel, target, entries, refresh_record=refresh_record)
            with self.subTest(attack=label), self.assertRaises(ArchiveIdentityError):
                verify_wheel(target, self.source, engine_probe=should_not_probe)
            self.assertFalse(probe_marker.exists())

        rejected_wheel(
            "unexpected-module",
            base_wheel + [("repopact/attacker.py", b"payload", stat.S_IFREG | 0o644, 3)],
        )
        rejected_wheel(
            "unexpected-pth",
            base_wheel
            + [
                (
                    f"repopact-{self.source.version}.data/purelib/attack.pth",
                    b"import os",
                    stat.S_IFREG | 0o644,
                    3,
                )
            ],
        )
        rejected_wheel(
            "parent-traversal",
            base_wheel + [("../escape", b"x", stat.S_IFREG | 0o644, 3)],
            refresh_record=False,
        )
        rejected_wheel(
            "absolute-path",
            base_wheel + [("/outside", b"x", stat.S_IFREG | 0o644, 3)],
            refresh_record=False,
        )
        rejected_wheel(
            "drive-path",
            base_wheel + [("C:/outside", b"x", stat.S_IFREG | 0o644, 3)],
            refresh_record=False,
        )
        rejected_wheel(
            "normalization-collision",
            base_wheel + [("repopact/agents.md", b"collision", stat.S_IFREG | 0o644, 3)],
            refresh_record=False,
        )
        rejected_wheel(
            "duplicate-member",
            base_wheel + [next(entry for entry in base_wheel if entry[0] == "repopact/engine_client.py")],
            refresh_record=False,
        )
        rejected_wheel(
            "symlink",
            base_wheel + [("repopact/unsafe", b"../README.md", stat.S_IFLNK | 0o777, 3)],
            refresh_record=False,
        )
        rejected_wheel(
            "unexpected-metadata",
            base_wheel
            + [
                (
                    f"repopact-{self.source.version}.dist-info/entry_points.txt",
                    b"[console_scripts]\nattacker=payload:main\n",
                    stat.S_IFREG | 0o644,
                    3,
                )
            ],
        )
        rejected_wheel(
            "missing-source-member",
            [entry for entry in base_wheel if entry[0] != "repopact/engine_client.py"],
        )
        rejected_wheel(
            "modified-source-member",
            [
                (name, data + b"# altered\n" if name == "repopact/engine_client.py" else data, mode, system)
                for name, data, mode, system in base_wheel
            ],
        )
        rejected_wheel(
            "missing-engine",
            [entry for entry in base_wheel if entry[0] != engine_path],
        )

        rejected_wheel(
            "modified-executable-before-launch",
            [
                (name, marker_payload if name == engine_path else data, mode, system)
                for name, data, mode, system in base_wheel
            ],
        )
        self.assertFalse(probe_marker.exists(), "modified engine reached the handshake probe")
        rejected_wheel(
            "wrong-source-version",
            [
                (
                    name,
                    data.replace(b"Version: " + self.source.version.encode(), b"Version: 3.1.3")
                    if name == metadata_path
                    else data,
                    mode,
                    system,
                )
                for name, data, mode, system in base_wheel
            ],
        )
        rejected_wheel(
            "unexpected-executable",
            base_wheel
            + [
                (
                    f"repopact-{self.source.version}.data/scripts/evil.exe",
                    b"MZ",
                    stat.S_IFREG | 0o755,
                    3,
                )
            ],
        )
        self.assertIn(record_path, wheel_name_set)

        base_sdist = _archive_sdist_entries(self.sdist)
        sdist_prefix = f"repopact-{self.source.version}/"

        def rejected_sdist(
            label: str,
            entries: list[tuple[str, bytes, int, str, dict[str, str]]],
        ) -> None:
            target = self.temp_root / f"{label}.tar.gz"
            _write_sdist_copy(target, entries)
            with self.subTest(attack=label), self.assertRaises(ArchiveIdentityError):
                verify_sdist(target, self.source)

        rejected_sdist(
            "unexpected-source",
            base_sdist + [(sdist_prefix + "attacker.py", b"payload", 0o644, tarfile.REGTYPE, {})],
        )
        rejected_sdist(
            "sdist-traversal",
            base_sdist + [(sdist_prefix + "../escape", b"x", 0o644, tarfile.REGTYPE, {})],
        )
        rejected_sdist(
            "sdist-absolute",
            base_sdist + [("/outside", b"x", 0o644, tarfile.REGTYPE, {})],
        )
        rejected_sdist(
            "sdist-drive",
            base_sdist + [("C:/outside", b"x", 0o644, tarfile.REGTYPE, {})],
        )
        rejected_sdist(
            "sdist-duplicate",
            base_sdist + [next(entry for entry in base_sdist if entry[0].endswith("repopact/engine_client.py"))],
        )
        rejected_sdist(
            "sdist-normalization-collision",
            base_sdist
            + [
                (
                    sdist_prefix + "repopact/ENGINE_CLIENT.py",
                    b"collision",
                    0o644,
                    tarfile.REGTYPE,
                    {},
                )
            ],
        )
        rejected_sdist(
            "sdist-symlink",
            base_sdist
            + [
                (
                    sdist_prefix + "repopact/unsafe-link",
                    b"../README.md",
                    0o777,
                    tarfile.SYMTYPE,
                    {},
                )
            ],
        )
        rejected_sdist(
            "sdist-hardlink",
            base_sdist
            + [
                (
                    sdist_prefix + "repopact/unsafe-hardlink",
                    b"README.md",
                    0o777,
                    tarfile.LNKTYPE,
                    {},
                )
            ],
        )
        rejected_sdist(
            "sdist-missing-source",
            [
                entry
                for entry in base_sdist
                if not entry[0].endswith("repopact/engine_client.py")
            ],
        )
        rejected_sdist(
            "sdist-modified-source",
            [
                (
                    name,
                    data + b"# altered\n" if name.endswith("repopact/engine_client.py") else data,
                    mode,
                    entry_type,
                    pax,
                )
                for name, data, mode, entry_type, pax in base_sdist
            ],
        )


if __name__ == "__main__":
    unittest.main()
