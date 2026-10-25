"""Verify locally built Maturin archives against a trusted source/build context.

This verifier is for artifacts produced by the local release build. It is not a
general-purpose authenticity verifier for downloaded packages: the build output,
source tree, Cargo metadata, and source revision are trust inputs supplied by the
controlled build process.
"""

from __future__ import annotations

import argparse
import base64
import csv
import email
import hashlib
import importlib.util
import io
import json
import os
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
import tomllib
import unicodedata
import uuid
import zipfile
from dataclasses import dataclass
from pathlib import Path, PurePosixPath
from typing import Any

from generate_engine_semantic_identity import (
    ManifestError,
    _pyproject_projection,
    check,
    semantic_identity,
    workspace_manifest_projection,
)


class ArchiveIdentityError(RuntimeError):
    """Raised when a local distribution differs from its trusted build context."""


_EXPECTED_MATURIN_INCLUDE = (
    ("repopact/schemas/*.json", ("sdist", "wheel")),
    ("repopact/templates/*", ("sdist", "wheel")),
    (".gitattributes", ("sdist",)),
    ("SPEC.md", ("sdist",)),
    ("conformance/**/*", ("sdist",)),
    ("rust/**", ("sdist",)),
    ("VERSION", ("sdist",)),
    ("RELEASE_LABEL", ("sdist",)),
    ("LICENSE", ("sdist",)),
    ("repopact/requirements-runtime.lock", ("sdist", "wheel")),
    ("repopact/runtime-dependency-groups.json", ("sdist", "wheel")),
    ("build/semantic-input-policy.json", ("sdist",)),
    ("build/semantic-input-manifest.json", ("sdist",)),
    ("scripts/generate_engine_semantic_identity.py", ("sdist",)),
    ("scripts/verify_engine_package_identity.py", ("sdist",)),
    ("repopact/_semantic-input-manifest.json", ("sdist", "wheel")),
    ("repopact/_semantic_identity.json", ("sdist", "wheel")),
)
_SOURCE_TOP_LEVEL_FILES = (
    ".gitattributes",
    "LICENSE",
    "README.md",
    "RELEASE_LABEL",
    "SPEC.md",
    "VERSION",
    "pyproject.toml",
)
_SDIST_FIXED_FILES = (
    "build/semantic-input-manifest.json",
    "build/semantic-input-policy.json",
    "scripts/generate_engine_semantic_identity.py",
    "scripts/verify_engine_package_identity.py",
)
_PACKAGE_SUFFIXES = {".json", ".lock", ".md", ".py"}
_EXECUTABLE_SUFFIXES = {".dll", ".dylib", ".exe", ".pyd", ".so"}
_ALLOWED_PAX_KEYS = {"atime", "ctime", "gid", "gname", "mtime", "path", "size", "uid", "uname"}
_METADATA_FIELDS = {
    "description-content-type",
    "license-expression",
    "license-file",
    "metadata-version",
    "name",
    "project-url",
    "provides-extra",
    "requires-dist",
    "requires-python",
    "summary",
    "version",
}


@dataclass(frozen=True)
class PackageSource:
    root: Path
    version: str
    protocol_version: int
    semantic_identity: str
    source_revision: str
    project: dict[str, Any]
    source_files: dict[str, bytes]
    package_files: dict[str, bytes]
    workspace_members: tuple[str, ...]
    cargo_packages: dict[str, dict[str, Any]]
    cargo_nodes: dict[str, dict[str, Any]]
    cargo_root_id: str
    cargo_closure: frozenset[str]
    cargo_edges: frozenset[tuple[tuple[str, str], tuple[str, str]]]
    cargo_target_directory: Path


def _load_json(raw: bytes, description: str) -> dict[str, Any]:
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ArchiveIdentityError(f"{description} is invalid JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise ArchiveIdentityError(f"{description} must be a JSON object")
    return value


def _sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _archive_digest(path: Path) -> str:
    return _sha256_file(path)


def _safe_source_file(root: Path, relative: str) -> bytes:
    path = root / Path(*PurePosixPath(relative).parts)
    try:
        info = path.lstat()
    except OSError as exc:
        raise ArchiveIdentityError(f"trusted source file is missing: {relative}") from exc
    if stat.S_ISLNK(info.st_mode) or not stat.S_ISREG(info.st_mode):
        raise ArchiveIdentityError(f"trusted source path is not a regular file: {relative}")
    if info.st_mode & 0o111:
        raise ArchiveIdentityError(f"source package contains an executable file: {relative}")
    return path.read_bytes()


def _tree_files(root: Path, relative_root: str, *, allowed_suffixes: set[str] | None = None) -> dict[str, bytes]:
    base = root / Path(*PurePosixPath(relative_root).parts)
    if not base.is_dir() or base.is_symlink():
        raise ArchiveIdentityError(f"trusted source directory is missing or unsafe: {relative_root}")
    result: dict[str, bytes] = {}
    for path in sorted(base.rglob("*")):
        if path.is_symlink():
            raise ArchiveIdentityError(
                f"trusted source tree contains a symlink: {path.relative_to(root).as_posix()}"
            )
        if path.is_dir():
            if path.name in {".git", "__pycache__", "target"}:
                continue
            continue
        if not path.is_file():
            raise ArchiveIdentityError(
                f"trusted source tree contains a non-file entry: {path.relative_to(root).as_posix()}"
            )
        relative = path.relative_to(root).as_posix()
        if any(part in {".git", "__pycache__", "target"} for part in PurePosixPath(relative).parts):
            continue
        suffix = path.suffix.casefold()
        if allowed_suffixes is not None and suffix not in allowed_suffixes:
            raise ArchiveIdentityError(
                f"package source file has an unauthorized suffix: {relative}"
            )
        if suffix in _EXECUTABLE_SUFFIXES or suffix in {".pyc", ".pth", ".whl", ".zip"}:
            raise ArchiveIdentityError(
                f"package source tree contains an executable or install hook: {relative}"
            )
        if path.stat().st_mode & 0o111:
            raise ArchiveIdentityError(f"source tree contains an executable file: {relative}")
        result[relative] = path.read_bytes()
    return result


def _validate_maturin_policy(project: dict[str, Any]) -> tuple[str, ...]:
    tool = project.get("tool", {})
    maturin = tool.get("maturin") if isinstance(tool, dict) else None
    if not isinstance(maturin, dict):
        raise ArchiveIdentityError("source pyproject omitted the Maturin packaging configuration")
    if (
        maturin.get("bindings") != "bin"
        or maturin.get("manifest-path") != "rust/apps/repopact-engine/Cargo.toml"
        or maturin.get("python-source") != "."
        or maturin.get("python-packages") != ["repopact"]
    ):
        raise ArchiveIdentityError("source Maturin build configuration differs from the reviewed packaging policy")
    targets = maturin.get("targets")
    if not isinstance(targets, list):
        raise ArchiveIdentityError("source Maturin configuration omitted binary targets")
    target_map: dict[str, str] = {}
    for target in targets:
        if not isinstance(target, dict) or target.get("kind") != "bin":
            raise ArchiveIdentityError("source Maturin configuration contains an unsupported target")
        name = target.get("name")
        if not isinstance(name, str) or name in target_map:
            raise ArchiveIdentityError("source Maturin configuration has duplicate or invalid binary targets")
        target_map[name] = "bin"
    if target_map != {"repopact": "bin", "repopact-engine": "bin"}:
        raise ArchiveIdentityError("source Maturin binary target set differs from the reviewed policy")
    includes = maturin.get("include")
    if not isinstance(includes, list):
        raise ArchiveIdentityError("source Maturin configuration omitted its explicit include policy")
    normalized: list[tuple[str, tuple[str, ...]]] = []
    for entry in includes:
        if not isinstance(entry, dict) or set(entry) != {"path", "format"}:
            raise ArchiveIdentityError("source Maturin include entry is not explicitly scoped")
        path, formats = entry.get("path"), entry.get("format")
        if not isinstance(path, str) or not isinstance(formats, list) or not all(
            isinstance(value, str) for value in formats
        ):
            raise ArchiveIdentityError("source Maturin include entry is malformed")
        normalized.append((path, tuple(formats)))
    if tuple(normalized) != _EXPECTED_MATURIN_INCLUDE:
        raise ArchiveIdentityError(
            "source Maturin include policy changed; review and update the closed packaging policy explicitly"
        )
    return tuple(target_map)


def _source_protocol_version(root: Path) -> int:
    python_source = (root / "repopact" / "engine_client.py").read_text(encoding="utf-8")
    rust_source = (
        root / "rust" / "crates" / "repopact-protocol" / "src" / "lib.rs"
    ).read_text(encoding="utf-8")
    python_matches = re.findall(r"(?m)^PROTOCOL_VERSION\s*=\s*(\d+)\s*$", python_source)
    rust_matches = re.findall(
        r"(?m)^pub const PROTOCOL_VERSION:\s*u32\s*=\s*(\d+)\s*;", rust_source
    )
    if len(python_matches) != 1 or len(rust_matches) != 1:
        raise ArchiveIdentityError("source protocol major could not be derived unambiguously")
    python_version, rust_version = int(python_matches[0]), int(rust_matches[0])
    if python_version != rust_version:
        raise ArchiveIdentityError("Python and Rust source protocol majors disagree")
    return python_version


def _source_identity(root: Path) -> str:
    root = root.resolve()
    try:
        check(root)
        identity = _load_json(
            (root / "repopact" / "_semantic_identity.json").read_bytes(),
            "source identity",
        )
    except (OSError, ManifestError) as exc:
        raise ArchiveIdentityError(f"source semantic identity is not current: {exc}") from exc
    value = identity.get("semantic_identity")
    if not isinstance(value, str):
        raise ArchiveIdentityError("source identity omitted semantic_identity")
    return value


def _source_version(root: Path, project: dict[str, Any]) -> str:
    source_module = root / "repopact" / "package_version.py"
    spec = importlib.util.spec_from_file_location(
        "_repopact_trusted_package_version", source_module
    )
    if spec is None or spec.loader is None:
        raise ArchiveIdentityError("trusted package version module could not be loaded")
    module = importlib.util.module_from_spec(spec)
    try:
        spec.loader.exec_module(module)
        version = module.package_version(root)
    except (ImportError, OSError, ValueError) as exc:
        raise ArchiveIdentityError(f"source product version could not be derived: {exc}") from exc
    if not isinstance(version, str) or not version:
        raise ArchiveIdentityError("source product version is empty")
    canonical_version = version
    for marker in ("alpha", "a", "beta", "b", "rc", "dev"):
        if marker in version and version.rsplit(marker, 1)[1].isdigit():
            prefix, number = version.rsplit(marker, 1)
            canonical_version = (
                version if prefix.endswith(".") else f"{prefix}.{marker}{number}"
            )
            break
    project_info = project.get("project")
    if not isinstance(project_info, dict) or project_info.get("version") != canonical_version:
        raise ArchiveIdentityError(
            "source VERSION/RELEASE_LABEL and pyproject product version disagree"
        )
    return canonical_version


def _cargo_model(
    root: Path, cargo_metadata: dict[str, Any], maturin_manifest: str
) -> tuple[
    dict[str, dict[str, Any]],
    dict[str, dict[str, Any]],
    str,
    frozenset[str],
    frozenset[tuple[tuple[str, str], tuple[str, str]]],
    tuple[str, ...],
    Path,
]:
    packages_value = cargo_metadata.get("packages")
    resolve = cargo_metadata.get("resolve")
    if not isinstance(packages_value, list) or not isinstance(resolve, dict):
        raise ArchiveIdentityError("trusted Cargo metadata is incomplete")
    packages = {
        package["id"]: package
        for package in packages_value
        if isinstance(package, dict) and isinstance(package.get("id"), str)
    }
    nodes_value = resolve.get("nodes")
    if not isinstance(nodes_value, list):
        raise ArchiveIdentityError("trusted Cargo metadata omitted resolved dependency nodes")
    nodes = {
        node["id"]: node
        for node in nodes_value
        if isinstance(node, dict) and isinstance(node.get("id"), str)
    }
    root_manifest = (root / Path(*PurePosixPath(maturin_manifest).parts)).resolve()
    roots = [
        package_id
        for package_id, package in packages.items()
        if Path(str(package.get("manifest_path", ""))).resolve() == root_manifest
    ]
    if len(roots) != 1:
        raise ArchiveIdentityError("Cargo metadata does not identify the configured Maturin engine package")
    root_id = roots[0]
    if root_id not in nodes:
        raise ArchiveIdentityError("Cargo metadata omitted the engine dependency node")

    closure: set[str] = set()
    stack = [root_id]
    edges: set[tuple[tuple[str, str], tuple[str, str]]] = set()
    coordinates: dict[str, tuple[str, str]] = {}
    while stack:
        package_id = stack.pop()
        if package_id in closure:
            continue
        package = packages.get(package_id)
        node = nodes.get(package_id)
        if not isinstance(package, dict) or not isinstance(node, dict):
            raise ArchiveIdentityError("Cargo metadata dependency graph references an unknown package")
        name, version = package.get("name"), package.get("version")
        if not isinstance(name, str) or not isinstance(version, str):
            raise ArchiveIdentityError("Cargo metadata package identity is malformed")
        coordinate = (name, version)
        if coordinate in coordinates.values():
            raise ArchiveIdentityError(
                f"Cargo graph has ambiguous package coordinates: {name} {version}"
            )
        coordinates[package_id] = coordinate
        closure.add(package_id)
        dependencies = node.get("deps", [])
        if not isinstance(dependencies, list):
            raise ArchiveIdentityError("Cargo metadata dependency list is malformed")
        for dependency in dependencies:
            if not isinstance(dependency, dict) or not isinstance(dependency.get("pkg"), str):
                raise ArchiveIdentityError("Cargo metadata contains a malformed dependency edge")
            kinds = dependency.get("dep_kinds", [])
            if not isinstance(kinds, list):
                raise ArchiveIdentityError("Cargo metadata dependency kinds are malformed")
            include = not kinds or any(
                isinstance(kind, dict) and kind.get("kind") in (None, "build")
                for kind in kinds
            )
            if not include:
                continue
            dependency_id = dependency["pkg"]
            if dependency_id not in packages or dependency_id not in nodes:
                raise ArchiveIdentityError("Cargo metadata dependency edge is unresolved")
            stack.append(dependency_id)
            dependency_package = packages[dependency_id]
            dependency_coordinate = (
                str(dependency_package.get("name")),
                str(dependency_package.get("version")),
            )
            edges.add((coordinate, dependency_coordinate))

    local_ids = {
        package_id
        for package_id in closure
        if packages[package_id].get("source") is None
    }
    rust_root = (root / "rust").resolve()
    workspace_manifest = tomllib.loads((rust_root / "Cargo.toml").read_text(encoding="utf-8"))
    workspace = workspace_manifest.get("workspace")
    if not isinstance(workspace, dict) or not isinstance(workspace.get("members"), list):
        raise ArchiveIdentityError("source Cargo workspace members are not an explicit list")
    member_paths = workspace["members"]
    if not all(isinstance(member, str) and not any(ch in member for ch in "*?[") for member in member_paths):
        raise ArchiveIdentityError("globbed or malformed Cargo workspace members require explicit policy review")
    by_manifest = {
        (rust_root / Path(*PurePosixPath(member).parts) / "Cargo.toml").resolve(): member
        for member in member_paths
    }
    selected_members: list[str] = []
    for package_id in local_ids:
        package_path = Path(str(packages[package_id].get("manifest_path"))).resolve()
        try:
            package_path.relative_to(rust_root)
        except ValueError as exc:
            raise ArchiveIdentityError("Cargo local package escaped the source Rust workspace") from exc
        member = by_manifest.get(package_path)
        if member is None:
            raise ArchiveIdentityError("reachable local Cargo package is not a declared workspace member")
        selected_members.append(member)
    selected_set = set(selected_members)
    ordered_members = tuple(member for member in member_paths if member in selected_set)
    target_directory = cargo_metadata.get("target_directory")
    if not isinstance(target_directory, str) or not target_directory:
        raise ArchiveIdentityError("trusted Cargo metadata omitted its target directory")
    target_path = Path(target_directory).resolve()
    env_target = os.environ.get("CARGO_TARGET_DIR")
    if env_target:
        configured = Path(env_target)
        if not configured.is_absolute():
            configured = (root / configured).resolve()
        else:
            configured = configured.resolve()
        if configured != target_path:
            raise ArchiveIdentityError(
                "Cargo metadata target directory differs from the active CARGO_TARGET_DIR"
            )
    return (
        packages,
        nodes,
        root_id,
        frozenset(closure),
        frozenset(edges),
        ordered_members,
        target_path,
    )


def _source_package(root: Path, cargo_metadata_path: Path, source_revision: str) -> PackageSource:
    root = root.resolve()
    if not re.fullmatch(r"[0-9a-f]{40}", source_revision):
        raise ArchiveIdentityError("source revision must be a full lowercase Git commit SHA")
    try:
        project_bytes = (root / "pyproject.toml").read_bytes()
        project = tomllib.loads(project_bytes.decode("utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        raise ArchiveIdentityError(f"trusted source pyproject is invalid: {exc}") from exc
    targets = _validate_maturin_policy(project)
    version = _source_version(root, project)
    identity = _source_identity(root)
    protocol_version = _source_protocol_version(root)
    try:
        cargo_metadata = _load_json(cargo_metadata_path.read_bytes(), "trusted Cargo metadata")
    except OSError as exc:
        raise ArchiveIdentityError(f"trusted Cargo metadata could not be read: {exc}") from exc
    expected_workspace_root = (root / "rust").resolve()
    if Path(str(cargo_metadata.get("workspace_root", ""))).resolve() != expected_workspace_root:
        raise ArchiveIdentityError("Cargo metadata was not produced for the trusted source workspace")
    (
        cargo_packages,
        cargo_nodes,
        cargo_root_id,
        cargo_closure,
        cargo_edges,
        workspace_members,
        target_directory,
    ) = _cargo_model(root, cargo_metadata, project["tool"]["maturin"]["manifest-path"])

    source_files: dict[str, bytes] = {}
    for relative in _SOURCE_TOP_LEVEL_FILES + _SDIST_FIXED_FILES + (
        "rust/Cargo.lock",
        "rust/Cargo.toml",
    ):
        if relative in source_files:
            raise ArchiveIdentityError(f"source packaging policy repeats {relative}")
        source_files[relative] = _safe_source_file(root, relative)
    source_files.update(_tree_files(root, "repopact", allowed_suffixes=_PACKAGE_SUFFIXES))
    source_files.update(_tree_files(root, "conformance"))
    for member in workspace_members:
        member_root = (root / "rust" / Path(*PurePosixPath(member).parts)).resolve()
        try:
            member_root.relative_to((root / "rust").resolve())
        except ValueError as exc:
            raise ArchiveIdentityError("Cargo member path escaped the source Rust workspace") from exc
        source_files.update(
            _tree_files(root, member_root.relative_to(root).as_posix())
        )
    package_files = {
        relative: content
        for relative, content in source_files.items()
        if relative.startswith("repopact/")
    }
    if not package_files:
        raise ArchiveIdentityError("trusted source contains no Maturin Python package files")
    if set(targets) != {"repopact", "repopact-engine"}:
        raise ArchiveIdentityError("source Maturin targets differ from the verified binary pair")
    return PackageSource(
        root=root,
        version=version,
        protocol_version=protocol_version,
        semantic_identity=identity,
        source_revision=source_revision,
        project=project,
        source_files=source_files,
        package_files=package_files,
        workspace_members=workspace_members,
        cargo_packages=cargo_packages,
        cargo_nodes=cargo_nodes,
        cargo_root_id=cargo_root_id,
        cargo_closure=cargo_closure,
        cargo_edges=cargo_edges,
        cargo_target_directory=target_directory,
    )


def _canonical_archive_path(name: str, description: str) -> str:
    if not isinstance(name, str) or not name:
        raise ArchiveIdentityError(f"{description} contains an empty path")
    if "\x00" in name or "\\" in name:
        raise ArchiveIdentityError(f"{description} contains a NUL or ambiguous path separator")
    if not name.isascii():
        raise ArchiveIdentityError(f"{description} path is not ASCII and needs explicit normalization policy")
    if name.startswith("/") or name.startswith("//") or re.match(r"^[A-Za-z]:", name):
        raise ArchiveIdentityError(f"{description} contains an absolute, UNC, or drive-qualified path")
    if ":" in name:
        raise ArchiveIdentityError(f"{description} contains a Windows alternate-stream path")
    normalized = unicodedata.normalize("NFC", name)
    if normalized != name:
        raise ArchiveIdentityError(f"{description} path is not Unicode-normalized")
    parts = name.split("/")
    if any(part in {"", ".", ".."} for part in parts):
        raise ArchiveIdentityError(f"{description} contains a non-canonical path: {name!r}")
    if any(part.endswith((" ", ".")) for part in parts):
        raise ArchiveIdentityError(f"{description} contains a Windows-ambiguous path component")
    reserved = {"con", "prn", "aux", "nul"} | {
        f"com{index}" for index in range(1, 10)
    } | {f"lpt{index}" for index in range(1, 10)}
    if any(part.split(".", 1)[0].casefold() in reserved for part in parts):
        raise ArchiveIdentityError(f"{description} contains a reserved Windows device path")
    return "/".join(parts)


def _safe_manifest_path(value: str) -> str:
    path = _canonical_archive_path(value, "semantic manifest")
    if path != value:
        raise ArchiveIdentityError(f"semantic input path is not canonical: {value!r}")
    return path


def _validate_unique_paths(names: list[str], description: str) -> list[str]:
    canonical: list[str] = []
    normalized: dict[str, str] = {}
    exact: set[str] = set()
    for raw in names:
        name = _canonical_archive_path(raw, description)
        key = unicodedata.normalize("NFKC", name).casefold()
        if name in exact:
            raise ArchiveIdentityError(f"{description} contains a duplicate member: {name}")
        if key in normalized:
            raise ArchiveIdentityError(
                f"{description} contains normalization-colliding paths: "
                f"{normalized[key]!r} and {name!r}"
            )
        exact.add(name)
        normalized[key] = name
        canonical.append(name)
    return canonical


def _validate_tar_type(member: tarfile.TarInfo) -> None:
    if not member.isreg():
        raise ArchiveIdentityError(f"sdist contains an unsupported link or entry type: {member.name}")
    if member.mode & 0o111:
        raise ArchiveIdentityError(f"sdist contains an unexpected executable file: {member.name}")
    if member.pax_headers and set(member.pax_headers) - _ALLOWED_PAX_KEYS:
        raise ArchiveIdentityError(f"sdist contains unsupported PAX metadata: {member.name}")
    if member.linkname:
        raise ArchiveIdentityError(f"sdist regular member has an unexpected link target: {member.name}")
    if getattr(member, "sparse", None):
        raise ArchiveIdentityError(f"sdist contains an unsupported sparse member: {member.name}")


def _validate_zip_type(info: zipfile.ZipInfo) -> None:
    if info.is_dir() or (info.external_attr & 0x10):
        raise ArchiveIdentityError(f"wheel contains an explicit directory member: {info.filename}")
    mode = info.external_attr >> 16
    kind = stat.S_IFMT(mode)
    if kind not in {0, stat.S_IFREG}:
        raise ArchiveIdentityError(f"wheel contains a symlink or unsupported entry type: {info.filename}")


def _expected_sdist_paths(source: PackageSource) -> set[str]:
    return set(source.source_files) | {"PKG-INFO"}


def _validate_workspace_manifest(source: PackageSource, candidate: bytes) -> None:
    try:
        source_manifest = tomllib.loads(source.source_files["rust/Cargo.toml"].decode("utf-8"))
        candidate_manifest = tomllib.loads(candidate.decode("utf-8"))
    except (UnicodeDecodeError, tomllib.TOMLDecodeError) as exc:
        raise ArchiveIdentityError(f"sdist Rust workspace manifest is invalid: {exc}") from exc
    workspace = source_manifest.get("workspace")
    if not isinstance(workspace, dict):
        raise ArchiveIdentityError("trusted Rust workspace manifest is malformed")
    if "exclude" in workspace or "default-members" in workspace:
        raise ArchiveIdentityError(
            "Cargo workspace exclude/default-members fields need an explicit packaging rule"
        )
    expected = json.loads(json.dumps(source_manifest))
    expected["workspace"]["members"] = list(source.workspace_members)
    if candidate_manifest != expected:
        raise ArchiveIdentityError(
            "sdist Cargo workspace manifest is not the exact Maturin dependency-closure projection"
        )


def _maturin_requirement(value: str, extra: str | None = None) -> str:
    requirement, separator, marker = value.partition(";")
    requirement = requirement.strip()
    if separator:
        marker = marker.strip().replace("python_version", "python_full_version")
        requirement += " ; " + marker
    if extra is not None:
        requirement += " ; " if separator else " ; "
        requirement += f"extra == '{extra}'"
    return requirement


def _validate_metadata(raw: bytes, source: PackageSource, description: str) -> None:
    try:
        message = email.message_from_bytes(raw)
    except (TypeError, ValueError) as exc:
        raise ArchiveIdentityError(f"{description} metadata is malformed: {exc}") from exc
    fields = {key.casefold() for key in message.keys()}
    if fields != _METADATA_FIELDS:
        raise ArchiveIdentityError(
            f"{description} contains missing or unexpected installation metadata fields"
        )
    project = source.project.get("project")
    if not isinstance(project, dict):
        raise ArchiveIdentityError("trusted source project metadata is malformed")
    expected_single = {
        "Metadata-Version": "2.4",
        "Name": project.get("name"),
        "Version": source.version,
        "Summary": project.get("description"),
        "License-Expression": project.get("license"),
        "Requires-Python": project.get("requires-python"),
        "Description-Content-Type": "text/markdown; charset=UTF-8; variant=GFM",
    }
    for field, expected in expected_single.items():
        values = message.get_all(field, [])
        if values != [expected]:
            raise ArchiveIdentityError(
                f"{description} {field} differs from the trusted source project"
            )
    expected_urls = [
        f"{name}, {value}"
        for name, value in (project.get("urls") or {}).items()
    ]
    if message.get_all("Project-URL", []) != expected_urls:
        raise ArchiveIdentityError(f"{description} project URLs differ from source pyproject")
    if message.get_all("License-File", []) != ["LICENSE"]:
        raise ArchiveIdentityError(f"{description} license metadata differs from source packaging")
    dependencies = project.get("dependencies", [])
    optional = project.get("optional-dependencies", {})
    if not isinstance(dependencies, list) or not all(isinstance(item, str) for item in dependencies):
        raise ArchiveIdentityError("trusted source dependencies are malformed")
    if not isinstance(optional, dict) or any(
        not isinstance(name, str)
        or not isinstance(items, list)
        or not all(isinstance(item, str) for item in items)
        for name, items in optional.items()
    ):
        raise ArchiveIdentityError("trusted source optional dependencies are malformed")
    expected_requirements = {
        _maturin_requirement(item) for item in dependencies
    }
    for extra, items in optional.items():
        expected_requirements.update(_maturin_requirement(item, extra) for item in items)
    if set(message.get_all("Requires-Dist", [])) != expected_requirements:
        raise ArchiveIdentityError(
            f"{description} runtime or optional dependency metadata differs from source pyproject"
        )
    actual_requirements = message.get_all("Requires-Dist", [])
    if len(actual_requirements) != len(expected_requirements):
        raise ArchiveIdentityError(f"{description} repeats a dependency metadata entry")
    actual_extras = message.get_all("Provides-Extra", [])
    if len(actual_extras) != len(optional) or set(actual_extras) != set(optional):
        raise ArchiveIdentityError(f"{description} extras differ from source pyproject")
    readme_name = project.get("readme")
    if not isinstance(readme_name, str):
        raise ArchiveIdentityError("trusted source project must name a Markdown readme")
    readme = source.source_files.get(readme_name)
    if readme is None:
        raise ArchiveIdentityError("trusted source readme is not part of the package specification")
    payload = message.get_payload()
    if not isinstance(payload, str):
        raise ArchiveIdentityError(f"{description} readme description is malformed")
    canonical = lambda value: value.replace("\r\n", "\n").replace("\r", "\n").rstrip("\n")
    if canonical(payload) != canonical(readme.decode("utf-8")):
        raise ArchiveIdentityError(f"{description} description differs from trusted README.md")


def _verify_sdist_semantic_inputs(
    archive: tarfile.TarFile,
    members: dict[str, tarfile.TarInfo],
    prefix: str,
    source: PackageSource,
) -> int:
    def read(relative: str) -> bytes:
        member = members.get(prefix + relative)
        if member is None:
            raise ArchiveIdentityError(f"sdist omitted semantic input {relative}")
        stream = archive.extractfile(member)
        if stream is None:
            raise ArchiveIdentityError(f"sdist could not read semantic input {relative}")
        return stream.read()

    identity = _load_json(read("repopact/_semantic_identity.json"), "sdist identity")
    manifest = _load_json(read("build/semantic-input-manifest.json"), "sdist semantic manifest")
    package_manifest = _load_json(
        read("repopact/_semantic-input-manifest.json"), "sdist package semantic manifest"
    )
    policy = _load_json(read("build/semantic-input-policy.json"), "sdist semantic policy")
    if identity.get("semantic_identity") != source.semantic_identity:
        raise ArchiveIdentityError("sdist identity differs from the trusted source identity")
    if manifest.get("semantic_identity") != source.semantic_identity:
        raise ArchiveIdentityError("sdist manifest differs from the trusted source identity")
    if package_manifest != manifest:
        raise ArchiveIdentityError("sdist package and build manifests differ")
    if identity.get("manifest") != "repopact/_semantic-input-manifest.json":
        raise ArchiveIdentityError("sdist identity points to an unexpected manifest path")

    inputs = manifest.get("inputs")
    if not isinstance(inputs, list):
        raise ArchiveIdentityError("sdist semantic manifest omitted its input list")
    try:
        if semantic_identity(inputs) != source.semantic_identity:
            raise ArchiveIdentityError("sdist manifest inputs do not produce its semantic identity")
    except (KeyError, TypeError, ManifestError) as exc:
        raise ArchiveIdentityError(f"sdist manifest inputs are malformed: {exc}") from exc
    projection_config = policy.get("pyproject_projection")
    if not isinstance(projection_config, dict):
        raise ArchiveIdentityError("sdist policy omitted pyproject projection configuration")
    with tempfile.TemporaryDirectory(prefix="repopact-archive-policy-") as temporary:
        projection_root = Path(temporary)
        (projection_root / "pyproject.toml").write_bytes(read("pyproject.toml"))
        projection = _pyproject_projection(projection_root, projection_config)

    checked: set[str] = set()
    for entry in inputs:
        if not isinstance(entry, dict) or not isinstance(entry.get("path"), str):
            raise ArchiveIdentityError("sdist manifest contains a malformed input entry")
        relative = entry["path"]
        if relative == "@projection/pyproject-semantic-build-config.json":
            content = projection
        elif relative == "@projection/rust-workspace-manifest.json":
            rust_config = policy.get("rust")
            if not isinstance(rust_config, dict):
                raise ArchiveIdentityError("sdist policy omitted Rust semantic inputs")
            content = workspace_manifest_projection(
                read("rust/Cargo.toml").decode("utf-8"),
                rust_config.get("workspace_manifest_projection_exclude_fields", []),
            )
        else:
            relative = _safe_manifest_path(relative)
            content = read(relative)
        if hashlib.sha256(content).hexdigest() != entry.get("sha256"):
            raise ArchiveIdentityError(f"sdist semantic input digest mismatch: {relative}")
        checked.add(relative)
    for required in (
        "repopact/requirements-runtime.lock",
        "repopact/runtime-dependency-groups.json",
        "scripts/generate_engine_semantic_identity.py",
        "scripts/verify_engine_package_identity.py",
        "rust/Cargo.lock",
        "conformance/manifest.json",
    ):
        if required not in checked:
            raise ArchiveIdentityError(f"sdist identity closure does not include {required}")
    return len(inputs)


def verify_sdist(path: Path, source: PackageSource) -> dict[str, Any]:
    expected_root = f"repopact-{source.version}"
    prefix = expected_root + "/"
    expected = _expected_sdist_paths(source)
    try:
        with tarfile.open(path, "r:gz") as archive:
            entries = archive.getmembers()
            raw_names = [member.name for member in entries]
            names = _validate_unique_paths(raw_names, "sdist")
            members: dict[str, tarfile.TarInfo] = {}
            for name, member in zip(names, entries):
                _validate_tar_type(member)
                if not name.startswith(prefix):
                    raise ArchiveIdentityError(
                        f"sdist member is outside the source-version root: {name}"
                    )
                relative = name[len(prefix):]
                if not relative:
                    raise ArchiveIdentityError("sdist contains an empty package root member")
                members[relative] = member
            actual = set(members)
            if actual != expected:
                missing = sorted(expected - actual)
                extra = sorted(actual - expected)
                raise ArchiveIdentityError(
                    f"sdist membership differs from the deterministic source package "
                    f"(missing={missing[:8]}, unexpected={extra[:8]})"
                )
            for relative, expected_bytes in source.source_files.items():
                member = members[relative]
                if relative == "rust/Cargo.toml":
                    if member.size > max(65_536, len(expected_bytes) * 2):
                        raise ArchiveIdentityError(
                            "sdist Cargo workspace manifest exceeds its deterministic projection bound"
                        )
                elif member.size != len(expected_bytes):
                    raise ArchiveIdentityError(f"sdist source file size mismatch: {relative}")
                stream = archive.extractfile(member)
                if stream is None:
                    raise ArchiveIdentityError(f"sdist source file could not be read: {relative}")
                actual_bytes = stream.read()
                if relative == "rust/Cargo.toml":
                    _validate_workspace_manifest(source, actual_bytes)
                elif not hashlib.sha256(actual_bytes).digest() == hashlib.sha256(expected_bytes).digest():
                    raise ArchiveIdentityError(f"sdist source content mismatch: {relative}")
            metadata_path = "PKG-INFO"
            metadata_member = members[metadata_path]
            if metadata_member.size > max(1_000_000, len(source.source_files["README.md"]) * 2):
                raise ArchiveIdentityError("sdist PKG-INFO exceeds the source-derived metadata bound")
            metadata_stream = archive.extractfile(metadata_member)
            if metadata_stream is None:
                raise ArchiveIdentityError("sdist PKG-INFO could not be read")
            metadata_bytes = metadata_stream.read()
            _validate_metadata(metadata_bytes, source, "sdist")
            inputs_verified = _verify_sdist_semantic_inputs(
                archive, {prefix + name: item for name, item in members.items()}, prefix, source
            )
    except (tarfile.TarError, OSError) as exc:
        raise ArchiveIdentityError(f"sdist could not be inspected: {exc}") from exc
    return {
        "path": path.name,
        "sha256": _archive_digest(path),
        "semantic_identity": source.semantic_identity,
        "product_version": source.version,
        "semantic_inputs_verified": inputs_verified,
        "members_verified": len(names),
        "member_inventory": sorted(names),
        "pkg_info_sha256": _sha256_bytes(metadata_bytes),
    }


def _validate_wheel_metadata_files(
    wheel_name: str, names: set[str], source: PackageSource
) -> tuple[str, str, str]:
    stem = Path(wheel_name).name
    if not stem.endswith(".whl"):
        raise ArchiveIdentityError("wheel artifact does not have a .whl filename")
    parts = stem[:-4].split("-")
    if len(parts) != 5 or parts[:2] != ["repopact", source.version]:
        raise ArchiveIdentityError("wheel filename distribution or version differs from source")
    _, _, python_tag, abi_tag, platform_tag = parts
    dist_info = f"repopact-{source.version}.dist-info"
    data_dir = f"repopact-{source.version}.data"
    expected_dist = {
        f"{dist_info}/METADATA",
        f"{dist_info}/WHEEL",
        f"{dist_info}/RECORD",
        f"{dist_info}/licenses/LICENSE",
        f"{dist_info}/sboms/repopact-engine.cyclonedx.json",
    }
    if not expected_dist.issubset(names):
        raise ArchiveIdentityError("wheel omitted an explicitly authorized Maturin metadata member")
    expected_tags = {f"{python_tag}-{abi_tag}-{platform_tag}"}
    return dist_info, data_dir, next(iter(expected_tags))


def _cargo_coordinate(source: PackageSource, package_id: str) -> tuple[str, str]:
    package = source.cargo_packages[package_id]
    return str(package["name"]), str(package["version"])


def _validate_sbom(raw: bytes, source: PackageSource) -> None:
    document = _load_json(raw, "wheel CycloneDX SBOM")
    if (
        document.get("bomFormat") != "CycloneDX"
        or document.get("specVersion") != "1.5"
        or document.get("version") != 1
    ):
        raise ArchiveIdentityError("wheel SBOM does not match the supported Maturin CycloneDX format")
    serial = document.get("serialNumber")
    try:
        if not isinstance(serial, str) or not serial.startswith("urn:uuid:"):
            raise ValueError("missing UUID")
        uuid.UUID(serial[len("urn:uuid:"):])
    except (ValueError, AttributeError) as exc:
        raise ArchiveIdentityError("wheel SBOM serialNumber is not a UUID") from exc
    metadata = document.get("metadata")
    root_component = metadata.get("component") if isinstance(metadata, dict) else None
    root_package = source.cargo_packages[source.cargo_root_id]
    root_coordinate = _cargo_coordinate(source, source.cargo_root_id)
    if (
        not isinstance(root_component, dict)
        or root_component.get("type") != "application"
        or (root_component.get("name"), root_component.get("version")) != root_coordinate
        or not isinstance(root_component.get("bom-ref"), str)
    ):
        raise ArchiveIdentityError("wheel SBOM root component differs from Cargo engine source")
    target_components = root_component.get("components")
    if not isinstance(target_components, list):
        raise ArchiveIdentityError("wheel SBOM omitted Maturin binary target records")
    expected_targets = {"repopact", "repopact-engine"}
    actual_targets: set[str] = set()
    for target in target_components:
        if not isinstance(target, dict) or target.get("type") != "application":
            raise ArchiveIdentityError("wheel SBOM contains an unsupported target component")
        if target.get("version") != root_package.get("version") or not isinstance(
            target.get("bom-ref"), str
        ):
            raise ArchiveIdentityError("wheel SBOM binary target identity differs from Cargo source")
        name = target.get("name")
        if not isinstance(name, str) or name in actual_targets:
            raise ArchiveIdentityError("wheel SBOM contains duplicate or malformed binary targets")
        actual_targets.add(name)
    if actual_targets != expected_targets:
        raise ArchiveIdentityError("wheel SBOM binary target set differs from source pyproject")

    expected_components = {
        _cargo_coordinate(source, package_id)
        for package_id in source.cargo_closure
        if package_id != source.cargo_root_id
    }
    component_values = document.get("components")
    if not isinstance(component_values, list):
        raise ArchiveIdentityError("wheel SBOM components are malformed")
    ref_coordinates: dict[str, tuple[str, str]] = {
        root_component["bom-ref"]: root_coordinate
    }
    actual_components: set[tuple[str, str]] = set()
    for component in component_values:
        if not isinstance(component, dict) or component.get("type") != "library":
            raise ArchiveIdentityError("wheel SBOM contains an unsupported dependency component")
        name, version, reference = (
            component.get("name"),
            component.get("version"),
            component.get("bom-ref"),
        )
        if not all(isinstance(value, str) and value for value in (name, version, reference)):
            raise ArchiveIdentityError("wheel SBOM dependency component identity is malformed")
        coordinate = (name, version)
        if coordinate in actual_components or reference in ref_coordinates:
            raise ArchiveIdentityError("wheel SBOM contains duplicate dependency identities")
        actual_components.add(coordinate)
        ref_coordinates[reference] = coordinate
    if actual_components != expected_components:
        raise ArchiveIdentityError("wheel SBOM dependency inventory differs from locked Cargo closure")

    dependency_entries = document.get("dependencies")
    if not isinstance(dependency_entries, list):
        raise ArchiveIdentityError("wheel SBOM dependency graph is malformed")
    actual_edges: set[tuple[tuple[str, str], tuple[str, str]]] = set()
    seen_nodes: set[str] = set()
    for item in dependency_entries:
        if not isinstance(item, dict) or not isinstance(item.get("ref"), str):
            raise ArchiveIdentityError("wheel SBOM dependency graph contains a malformed node")
        reference = item["ref"]
        if reference not in ref_coordinates or reference in seen_nodes:
            raise ArchiveIdentityError("wheel SBOM dependency graph has an unknown or repeated node")
        seen_nodes.add(reference)
        depends_on = item.get("dependsOn", [])
        if not isinstance(depends_on, list) or not all(isinstance(value, str) for value in depends_on):
            raise ArchiveIdentityError("wheel SBOM dependency edge list is malformed")
        if len(set(depends_on)) != len(depends_on):
            raise ArchiveIdentityError("wheel SBOM dependency graph repeats an edge")
        for target in depends_on:
            if target not in ref_coordinates:
                raise ArchiveIdentityError("wheel SBOM dependency graph points outside its component set")
            actual_edges.add((ref_coordinates[reference], ref_coordinates[target]))
    expected_nodes = {
        _cargo_coordinate(source, package_id) for package_id in source.cargo_closure
    }
    if {ref_coordinates[reference] for reference in seen_nodes} != expected_nodes:
        raise ArchiveIdentityError("wheel SBOM dependency graph omits a locked Cargo package")
    if actual_edges != set(source.cargo_edges):
        raise ArchiveIdentityError("wheel SBOM dependency edges differ from Cargo metadata")


def _trusted_binaries(source: PackageSource) -> dict[str, tuple[Path, str, int]]:
    suffix = ".exe" if os.name == "nt" else ""
    binaries = {
        f"repopact-engine{suffix}": source.cargo_target_directory / "release" / f"repopact-engine{suffix}",
        f"repopact{suffix}": source.cargo_target_directory / "release" / f"repopact{suffix}",
    }
    result: dict[str, tuple[Path, str, int]] = {}
    for name, path in binaries.items():
        try:
            info = path.lstat()
        except OSError as exc:
            raise ArchiveIdentityError(
                f"trusted local Maturin build output is missing: {path}"
            ) from exc
        if stat.S_ISLNK(info.st_mode) or not stat.S_ISREG(info.st_mode):
            raise ArchiveIdentityError(f"trusted build output is not a regular file: {path}")
        result[name] = (path, _sha256_file(path), info.st_size)
    return result


def _probe_packaged_engine(member_name: str, executable: bytes) -> dict[str, Any]:
    """Run a bounded handshake after the archive bytes match the trusted build output.

    The subprocess is not sandboxed. Only a locally built binary whose digest was
    matched before this call is eligible for execution.
    """
    basename = PurePosixPath(member_name).name
    if basename not in {"repopact-engine", "repopact-engine.exe"}:
        raise ArchiveIdentityError(f"unexpected packaged engine filename: {member_name}")
    with tempfile.TemporaryDirectory(prefix="repopact-wheel-engine-") as temporary:
        executable_path = Path(temporary) / basename
        executable_path.write_bytes(executable)
        request = {
            "protocol": "repopact-engine",
            "protocol_version": 2,
            "request_id": "wheel-identity-check",
            "operation": "handshake",
            "root": None,
            "params": {},
        }
        environment = os.environ.copy()
        environment.pop("REPOPACT_ENGINE", None)
        try:
            result = subprocess.run(
                [str(executable_path)],
                input=json.dumps(request, sort_keys=True, separators=(",", ":")) + "\n",
                text=True,
                encoding="utf-8",
                errors="strict",
                capture_output=True,
                timeout=5.0,
                cwd=temporary,
                env=environment,
                shell=False,
                check=False,
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            raise ArchiveIdentityError(f"trusted packaged engine handshake could not complete: {exc}") from exc
        if result.returncode != 0:
            detail = result.stderr.strip()[-1000:]
            raise ArchiveIdentityError(
                "trusted packaged engine handshake exited unsuccessfully"
                + (f": {detail}" if detail else "")
            )
        lines = [line for line in result.stdout.splitlines() if line.strip()]
        if len(lines) != 1:
            raise ArchiveIdentityError("trusted packaged engine did not return exactly one JSON line")
        try:
            response = json.loads(lines[0])
        except json.JSONDecodeError as exc:
            raise ArchiveIdentityError(f"packaged engine returned invalid handshake JSON: {exc.msg}") from exc
        if not isinstance(response, dict):
            raise ArchiveIdentityError("packaged engine handshake response must be a JSON object")
        return response


def _validate_packaged_handshake(response: dict[str, Any], source: PackageSource) -> None:
    if response.get("ok") is not True:
        raise ArchiveIdentityError("packaged engine handshake was not successful")
    if (
        response.get("protocol") != "repopact-engine"
        or response.get("protocol_version") != source.protocol_version
    ):
        raise ArchiveIdentityError("packaged engine protocol identity differs from trusted source")
    if response.get("engine_version") != source.version:
        raise ArchiveIdentityError("packaged engine product version differs from trusted source")
    if response.get("semantic_identity") != source.semantic_identity:
        raise ArchiveIdentityError("packaged engine semantic identity differs from trusted source")
    if response.get("request_id") != "wheel-identity-check":
        raise ArchiveIdentityError("packaged engine handshake did not match the verifier request")
    build_info = response.get("build_info")
    if not isinstance(build_info, dict) or build_info.get("source_revision") != source.source_revision:
        raise ArchiveIdentityError("packaged engine source revision differs from the trusted build revision")


def _wheel_record(contents: dict[str, bytes], record_path: str) -> None:
    record_bytes = contents[record_path]
    try:
        rows = list(csv.reader(io.StringIO(record_bytes.decode("utf-8"), newline="")))
    except (UnicodeDecodeError, csv.Error) as exc:
        raise ArchiveIdentityError(f"wheel RECORD is malformed: {exc}") from exc
    rows_by_path: dict[str, tuple[str, str]] = {}
    for row in rows:
        if len(row) != 3:
            raise ArchiveIdentityError("wheel RECORD row must have exactly three fields")
        relative = _canonical_archive_path(row[0], "wheel RECORD")
        if relative in rows_by_path:
            raise ArchiveIdentityError(f"wheel RECORD repeats member {relative}")
        rows_by_path[relative] = (row[1], row[2])
    if set(rows_by_path) != set(contents):
        raise ArchiveIdentityError("wheel RECORD membership differs from the complete wheel")
    for name, data in contents.items():
        digest, size = rows_by_path[name]
        if name == record_path:
            if digest or size:
                raise ArchiveIdentityError("wheel RECORD must not hash itself")
            continue
        expected_digest = "sha256=" + base64.urlsafe_b64encode(
            hashlib.sha256(data).digest()
        ).rstrip(b"=").decode("ascii")
        if digest != expected_digest or size != str(len(data)):
            raise ArchiveIdentityError(f"wheel RECORD digest or size mismatch: {name}")


def verify_wheel(
    path: Path,
    source: PackageSource,
    *,
    engine_probe: Any = _probe_packaged_engine,
) -> dict[str, Any]:
    # Establish both expected executable digests from Cargo's local release
    # target before opening the candidate archive or executing any member.
    trusted = _trusted_binaries(source)
    source_members = set(source.package_files)
    engine_names = set(trusted)
    if len(engine_names) != 2:
        raise ArchiveIdentityError("trusted build output does not contain the exact CLI/engine pair")
    exe_suffixes = {Path(name).suffix for name in engine_names}
    if len(exe_suffixes) != 1:
        raise ArchiveIdentityError("trusted executable pair has inconsistent platform suffixes")
    expected_engine_names = {
        f"repopact-engine{next(iter(exe_suffixes))}",
        f"repopact{next(iter(exe_suffixes))}",
    }
    if engine_names != expected_engine_names:
        raise ArchiveIdentityError("trusted build output names differ from the Maturin target policy")
    try:
        with zipfile.ZipFile(path) as archive:
            infos = archive.infolist()
            names = _validate_unique_paths(
                [getattr(info, "orig_filename", info.filename) for info in infos],
                "wheel",
            )
            for info in infos:
                _validate_zip_type(info)
            actual = set(names)
            dist_info, data_dir, expected_tag = _validate_wheel_metadata_files(
                path.name, actual, source
            )
            record_path = f"{dist_info}/RECORD"
            engine_paths = {
                name: f"{data_dir}/scripts/{name}" for name in engine_names
            }
            expected = source_members | set(engine_paths.values()) | {
                f"{dist_info}/METADATA",
                f"{dist_info}/WHEEL",
                record_path,
                f"{dist_info}/licenses/LICENSE",
                f"{dist_info}/sboms/repopact-engine.cyclonedx.json",
            }
            if actual != expected:
                missing, extra = sorted(expected - actual), sorted(actual - expected)
                raise ArchiveIdentityError(
                    f"wheel membership differs from the closed packaging policy "
                    f"(missing={missing[:8]}, unexpected={extra[:8]})"
                )
            infos_by_name = dict(zip(names, infos))
            authorized_executable_paths = set(engine_paths.values())
            for name, info in infos_by_name.items():
                mode = info.external_attr >> 16
                if mode & 0o111 and name not in authorized_executable_paths:
                    raise ArchiveIdentityError(f"wheel contains an unexpected executable member: {name}")
            expected_sizes = {name: len(data) for name, data in source.package_files.items()}
            expected_sizes[f"{dist_info}/licenses/LICENSE"] = len(source.source_files["LICENSE"])
            expected_sizes.update({
                engine_paths[name]: trusted[name][2] for name in engine_names
            })
            expected_sizes[f"{dist_info}/METADATA"] = max(
                1_000_000, len(source.source_files["README.md"]) * 2
            )
            expected_sizes[f"{dist_info}/WHEEL"] = 65_536
            expected_sizes[f"{dist_info}/RECORD"] = 8_000_000
            expected_sizes[f"{dist_info}/sboms/repopact-engine.cyclonedx.json"] = 8_000_000
            contents: dict[str, bytes] = {}
            for name, info in infos_by_name.items():
                maximum = expected_sizes.get(name)
                if maximum is None or info.file_size > maximum:
                    raise ArchiveIdentityError(f"wheel member exceeds its deterministic size bound: {name}")
                if name in source.package_files or name.endswith("/licenses/LICENSE"):
                    expected_size = expected_sizes[name]
                    if info.file_size != expected_size:
                        raise ArchiveIdentityError(f"wheel source member size mismatch: {name}")
                contents[name] = archive.read(info)
                if len(contents[name]) != info.file_size:
                    raise ArchiveIdentityError(f"wheel member decompressed size mismatch: {name}")

            for name, expected_bytes in source.package_files.items():
                if contents[name] != expected_bytes:
                    raise ArchiveIdentityError(f"wheel package source differs from trusted source: {name}")
            if contents[f"{dist_info}/licenses/LICENSE"] != source.source_files["LICENSE"]:
                raise ArchiveIdentityError("wheel license differs from trusted source LICENSE")

            metadata_path = f"{dist_info}/METADATA"
            wheel_metadata = contents[metadata_path]
            _validate_metadata(wheel_metadata, source, "wheel")
            wheel_description = f"{dist_info}/WHEEL"
            wheel_message = email.message_from_bytes(contents[wheel_description])
            wheel_fields = {key.casefold() for key in wheel_message.keys()}
            if wheel_fields != {"generator", "root-is-purelib", "tag", "wheel-version"}:
                raise ArchiveIdentityError("wheel WHEEL metadata has unexpected fields")
            if (
                wheel_message.get_all("Wheel-Version", []) != ["1.0"]
                or wheel_message.get_all("Root-Is-Purelib", []) != ["false"]
                or wheel_message.get_all("Tag", []) != [expected_tag]
                or not re.fullmatch(
                    r"maturin \([0-9]+\.[0-9]+\.[0-9]+[A-Za-z0-9.+-]*\)",
                    (wheel_message.get("Generator") or ""),
                )
            ):
                raise ArchiveIdentityError("wheel WHEEL metadata differs from the Maturin source/build policy")
            sbom_path = f"{dist_info}/sboms/repopact-engine.cyclonedx.json"
            _validate_sbom(contents[sbom_path], source)

            # The candidate executable is not executed until this digest check
            # and the complete RECORD/source/metadata closure have passed.
            for name, member_path in engine_paths.items():
                path_in_archive = member_path
                trusted_path, trusted_digest, _ = trusted[name]
                if _sha256_bytes(contents[path_in_archive]) != trusted_digest:
                    raise ArchiveIdentityError(
                        f"wheel executable does not match trusted Maturin build output: {name}"
                    )
            _wheel_record(contents, record_path)
            engine_name = next(name for name in engine_names if name.startswith("repopact-engine"))
            engine_member = engine_paths[engine_name]
            engine_response = engine_probe(engine_member, contents[engine_member])
            if not isinstance(engine_response, dict):
                raise ArchiveIdentityError("packaged engine probe returned a malformed response")
            _validate_packaged_handshake(engine_response, source)
    except (zipfile.BadZipFile, OSError) as exc:
        raise ArchiveIdentityError(f"wheel could not be inspected: {exc}") from exc
    return {
        "path": path.name,
        "sha256": _archive_digest(path),
        "semantic_identity": source.semantic_identity,
        "product_version": source.version,
        "protocol_version": source.protocol_version,
        "member_count": len(names),
        "member_inventory": sorted(names),
        "engine_member": engine_member,
        "engine_sha256": trusted[engine_name][1],
        "engine_build_info": engine_response.get("build_info"),
        "metadata_sha256": _sha256_bytes(wheel_metadata),
        "trusted_build_outputs": {
            name: {"path": str(value[0]), "sha256": value[1], "size": value[2]}
            for name, value in sorted(trusted.items())
        },
        "engine_identity_verified": True,
        "executable_trust_boundary": (
            "archive executable bytes matched the local Cargo release output before the "
            "bounded handshake; this is not an external signature or a sandbox"
        ),
    }


def verify_archives(
    root: Path,
    sdist: Path,
    wheel: Path,
    cargo_metadata_path: Path,
    source_revision: str,
    *,
    engine_probe: Any = _probe_packaged_engine,
) -> dict[str, Any]:
    source = _source_package(root, cargo_metadata_path, source_revision)
    sdist_report = verify_sdist(sdist, source)
    wheel_report = verify_wheel(wheel, source, engine_probe=engine_probe)
    if sdist_report["pkg_info_sha256"] != wheel_report["metadata_sha256"]:
        raise ArchiveIdentityError("sdist PKG-INFO and wheel METADATA do not match")
    trusted = _trusted_binaries(source)
    return {
        "source_revision": source_revision,
        "product_version": source.version,
        "protocol_version": source.protocol_version,
        "semantic_identity": source.semantic_identity,
        "cargo_target_directory": str(source.cargo_target_directory),
        "trusted_executables": {
            name: {"path": str(value[0]), "sha256": value[1], "size": value[2]}
            for name, value in sorted(trusted.items())
        },
        "sdist": sdist_report,
        "wheel": wheel_report,
        "source_wheel_sdist_metadata_agreement": True,
        "binary_authenticity": (
            "not established; executable digests are anchored to local build outputs, "
            "not to an independently signed publisher identity"
        ),
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--sdist", type=Path, required=True)
    parser.add_argument("--wheel", type=Path, required=True)
    parser.add_argument("--cargo-metadata", type=Path, required=True)
    parser.add_argument("--source-revision", required=True)
    args = parser.parse_args(argv)
    try:
        report = verify_archives(
            args.root,
            args.sdist,
            args.wheel,
            args.cargo_metadata,
            args.source_revision,
        )
    except (
        OSError,
        ValueError,
        ArchiveIdentityError,
        tarfile.TarError,
        zipfile.BadZipFile,
        csv.Error,
        tomllib.TOMLDecodeError,
    ) as exc:
        print(f"package archive verification failed: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
