"""Discover and hash RepoPact semantic inputs without invoking a build tool."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import struct
import sys
import tomllib
import unicodedata
from pathlib import Path
from typing import Any

POLICY_PATH = Path("build/semantic-input-policy.json")
IDENTITY_PREFIX = b"repopact-engine-semantic-inputs\0v1\0"
IDENTITY_FORMAT = "repopact-semantic-v1:sha256:"
_INCLUDE_CALL = re.compile(r"\binclude(?:_str|_bytes)?\s*!\s*\(")
_INCLUDE_LITERAL = re.compile(
    r'\binclude(?:_str|_bytes)?\s*!\s*\(\s*"([^"]*)"\s*\)', re.DOTALL
)


class ManifestError(RuntimeError):
    """The governed source closure cannot be identified safely."""


def _json_bytes(value: Any) -> bytes:
    return json.dumps(
        value, ensure_ascii=False, sort_keys=True, separators=(",", ":")
    ).encode("utf-8")


def _pretty_json(value: Any) -> bytes:
    return (
        json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n"
    ).encode("utf-8")


def _normalized_relative(path: str) -> str:
    value = unicodedata.normalize("NFC", path.replace("\\", "/"))
    if not value or "\x00" in value or "\n" in value or "\r" in value:
        raise ManifestError(f"invalid semantic input path: {path!r}")
    return value


def _safe_path(root: Path, candidate: Path) -> tuple[Path, str]:
    root = root.resolve()
    try:
        relative = candidate.relative_to(root) if candidate.is_absolute() else candidate
        current = root
        for part in relative.parts:
            current = current / part
            if current.is_symlink():
                raise ManifestError(f"symlink is not allowed in semantic inputs: {current}")
        resolved = (root / relative).resolve(strict=True)
        relative_to_root = resolved.relative_to(root)
    except (OSError, ValueError) as exc:
        raise ManifestError(f"semantic input escapes the repository or is missing: {candidate}") from exc
    if not resolved.is_file():
        raise ManifestError(f"semantic input is not a regular file: {candidate}")
    return resolved, _normalized_relative(relative_to_root.as_posix())


def _add_file(root: Path, path: Path, entries: dict[str, tuple[Path | None, bytes]]) -> None:
    resolved, relative = _safe_path(root, path)
    previous = entries.get(relative)
    content = resolved.read_bytes()
    if previous is not None and previous[0] != resolved:
        raise ManifestError(f"normalized path collision for {relative!r}")
    entries[relative] = (resolved, content)


def _tree_files(
    root: Path, relative_root: str, *, extensions: set[str] | None,
    ignored_directory_names: set[str] | None = None,
) -> list[Path]:
    directory = root / relative_root
    if not directory.is_dir():
        raise ManifestError(f"required semantic input directory is missing: {relative_root}")
    ignored = ignored_directory_names or set()
    result: list[Path] = []
    for item in directory.rglob("*"):
        if item.is_symlink():
            raise ManifestError(f"symlink found under semantic input root: {item}")
        if item.is_dir():
            continue
        relative_parts = item.relative_to(directory).parts
        if any(part in ignored for part in relative_parts[:-1]):
            continue
        if extensions is None or item.suffix.lower() in extensions:
            result.append(item)
    return sorted(result, key=lambda item: _normalized_relative(item.relative_to(root).as_posix()).encode("utf-8"))


def _workspace_packages(root: Path, workspace_manifest: str) -> tuple[Path, dict[str, Path], dict[str, Any]]:
    manifest_path, _ = _safe_path(root, root / workspace_manifest)
    workspace_root = manifest_path.parent
    workspace_data = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    workspace = workspace_data.get("workspace")
    if not isinstance(workspace, dict):
        raise ManifestError(f"{workspace_manifest} has no [workspace] table")
    members: dict[str, Path] = {}
    for member in workspace.get("members", []):
        matches = sorted(workspace_root.glob(member), key=lambda item: item.as_posix())
        if not matches:
            raise ManifestError(f"Cargo workspace member has no match: {member}")
        for match in matches:
            candidate = match / "Cargo.toml"
            if not candidate.is_file():
                raise ManifestError(f"Cargo workspace member has no Cargo.toml: {match}")
            data = tomllib.loads(candidate.read_text(encoding="utf-8"))
            package = data.get("package")
            if isinstance(package, dict) and isinstance(package.get("name"), str):
                members[package["name"]] = candidate.resolve()
    if not members:
        raise ManifestError("Cargo workspace contains no packages")
    return workspace_root, members, workspace_data


def workspace_manifest_projection(manifest_text: str, excluded_fields: list[str]) -> bytes:
    """Canonicalize build-relevant workspace settings, excluding package membership lists.

    Maturin/Cargo prunes unrelated workspace members and reformats this file in an
    sdist. The engine's actual dependency closure is discovered from the selected
    seed package; membership lists only determine which packages are available to
    that traversal, so they are represented by the discovered closure instead.
    """
    workspace_data = tomllib.loads(manifest_text)
    workspace = workspace_data.get("workspace")
    if not isinstance(workspace, dict):
        raise ManifestError("Cargo workspace manifest has no [workspace] table")
    allowed_exclusions = {"members", "exclude", "default-members"}
    if not isinstance(excluded_fields, list) or not all(
        isinstance(field, str) for field in excluded_fields
    ):
        raise ManifestError("unsupported Cargo workspace manifest projection exclusions")
    excluded = set(excluded_fields)
    if not excluded <= allowed_exclusions:
        raise ManifestError("unsupported Cargo workspace manifest projection exclusions")
    projection = {
        "workspace": {key: value for key, value in workspace.items() if key not in excluded}
    }
    return _json_bytes(projection)


def _dependency_tables(manifest: dict[str, Any]) -> list[dict[str, Any]]:
    tables: list[dict[str, Any]] = []
    for section in ("dependencies", "build-dependencies"):
        value = manifest.get(section)
        if isinstance(value, dict):
            tables.append(value)
    targets = manifest.get("target", {})
    if isinstance(targets, dict):
        for target_config in targets.values():
            if not isinstance(target_config, dict):
                continue
            for section in ("dependencies", "build-dependencies"):
                value = target_config.get(section)
                if isinstance(value, dict):
                    tables.append(value)
    return tables


def _local_dependency_manifest(
    root: Path,
    package_dir: Path,
    workspace_root: Path,
    dependency_name: str,
    specification: Any,
    workspace_dependencies: dict[str, Any],
) -> Path | None:
    spec = specification
    base_dir = package_dir
    if isinstance(specification, dict) and specification.get("workspace") is True:
        if dependency_name not in workspace_dependencies:
            raise ManifestError(f"workspace dependency {dependency_name!r} is unresolved")
        spec = workspace_dependencies[dependency_name]
        base_dir = workspace_root
    if not isinstance(spec, dict) or "path" not in spec:
        return None
    path = Path(spec["path"])
    dependency_path = path if path.is_absolute() else base_dir / path
    manifest = dependency_path if dependency_path.name == "Cargo.toml" else dependency_path / "Cargo.toml"
    resolved, _ = _safe_path(root, manifest)
    return resolved


def _rust_closure(
    root: Path, config: dict[str, Any], entries: dict[str, tuple[Path | None, bytes]]
) -> list[Path]:
    workspace_root, packages, workspace_data = _workspace_packages(
        root, config["workspace_manifest"]
    )
    workspace_dependencies = workspace_data.get("workspace", {}).get("dependencies", {})
    seed = config["seed_package"]
    if seed not in packages:
        raise ManifestError(f"Cargo workspace seed package is missing: {seed}")
    pending = [seed]
    visited: set[str] = set()
    source_files: set[Path] = set()
    while pending:
        name = pending.pop()
        if name in visited:
            continue
        visited.add(name)
        manifest_path = packages.get(name)
        if manifest_path is None:
            raise ManifestError(f"unresolved local Cargo dependency: {name}")
        _add_file(root, manifest_path, entries)
        package_dir = manifest_path.parent
        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        for source in _tree_files(
            root,
            package_dir.relative_to(root).as_posix(),
            extensions={config["source_extension"]},
            ignored_directory_names=set(config.get("ignored_directory_names", [])),
        ):
            source_files.add(source)
        package = manifest.get("package", {})
        custom_build = package.get("build") if isinstance(package, dict) else None
        build_script = custom_build if isinstance(custom_build, str) else "build.rs"
        build_path = package_dir / build_script
        if build_path.is_file():
            source_files.add(build_path.resolve())
        for table in _dependency_tables(manifest):
            for dependency_name, specification in table.items():
                local_manifest = _local_dependency_manifest(
                    root, package_dir, workspace_root, dependency_name, specification,
                    workspace_dependencies,
                )
                if local_manifest is not None:
                    data = tomllib.loads(local_manifest.read_text(encoding="utf-8"))
                    local_package = data.get("package", {})
                    local_name = local_package.get("name") if isinstance(local_package, dict) else None
                    if not isinstance(local_name, str):
                        raise ManifestError(f"path dependency has no package name: {local_manifest}")
                    packages[local_name] = local_manifest
                    pending.append(local_name)

    scanned: set[Path] = set()
    while source_files - scanned:
        source_path = sorted(source_files - scanned, key=lambda item: item.as_posix())[0]
        scanned.add(source_path)
        _add_file(root, source_path, entries)
        source = source_path.read_text(encoding="utf-8")
        calls = list(_INCLUDE_CALL.finditer(source))
        literal_calls = list(_INCLUDE_LITERAL.finditer(source))
        if len(calls) != len(literal_calls):
            raise ManifestError(
                f"unsupported non-literal Rust compile-time include in {source_path}"
            )
        for match in literal_calls:
            include_name = match.group(1)
            if "\\" in include_name:
                raise ManifestError(f"escaped include path is unsupported in {source_path}")
            include_path = (source_path.parent / include_name)
            resolved, _ = _safe_path(root, include_path)
            source_files.add(resolved)
    return sorted(source_files, key=lambda item: item.as_posix())


def _pyproject_projection(root: Path, config: dict[str, Any]) -> bytes:
    data = tomllib.loads((root / "pyproject.toml").read_text(encoding="utf-8"))
    project = data.get("project", {})
    optional = project.get("optional-dependencies", {})
    extras = config.get("runtime_optional_extras", [])
    missing = [name for name in extras if name not in optional]
    if missing:
        raise ManifestError("pyproject runtime optional extras are missing: " + ", ".join(missing))
    projection = {
        "build-system": data.get("build-system", {}),
        "project": {
            "dependencies": project.get("dependencies", []),
            "optional-dependencies": {name: optional[name] for name in sorted(extras)},
            "requires-python": project.get("requires-python"),
        },
        "tool.maturin": data.get("tool", {}).get("maturin", {}),
    }
    return _json_bytes(projection)


def discover_inputs(root: Path) -> list[dict[str, str]]:
    """Return the complete discovered input membership and raw-byte digests."""
    root = root.resolve()
    try:
        policy_path, policy_relative = _safe_path(root, root / POLICY_PATH)
        policy = json.loads(policy_path.read_text(encoding="utf-8"))
    except (json.JSONDecodeError, OSError) as exc:
        raise ManifestError(f"unable to read semantic input policy: {exc}") from exc
    if policy.get("format_version") != 1:
        raise ManifestError("unsupported semantic input policy format")
    entries: dict[str, tuple[Path | None, bytes]] = {}

    python = policy["python"]
    for source in _tree_files(
        root, python["root"], extensions=set(python["extensions"]),
        ignored_directory_names=set(python.get("ignored_directory_names", [])),
    ):
        _add_file(root, source, entries)
    for resource_root in python["resource_roots"]:
        for resource in _tree_files(root, resource_root, extensions=None):
            _add_file(root, resource, entries)

    rust_config = policy["rust"]
    workspace_manifest, _ = _safe_path(root, root / rust_config["workspace_manifest"])
    rust_closure = _rust_closure(root, rust_config, entries)
    workspace_projection_path = "@projection/rust-workspace-manifest.json"
    entries[workspace_projection_path] = (
        None,
        workspace_manifest_projection(
            workspace_manifest.read_text(encoding="utf-8"),
            rust_config.get("workspace_manifest_projection_exclude_fields", []),
        ),
    )
    lockfile = workspace_manifest.parent / "Cargo.lock"
    _add_file(root, lockfile, entries)

    conformance_root = policy["conformance_root"]
    for fixture in _tree_files(root, conformance_root, extensions=None):
        _add_file(root, fixture, entries)
    for fixed in policy["fixed_files"]:
        _add_file(root, root / fixed, entries)

    _add_file(root, policy_path, entries)
    projection_path = "@projection/pyproject-semantic-build-config.json"
    entries[projection_path] = (None, _pyproject_projection(root, policy["pyproject_projection"]))

    paths = sorted(entries, key=lambda value: value.encode("utf-8"))
    if len(paths) != len(set(paths)):
        raise ManifestError("duplicate normalized paths in semantic input set")
    output_paths = policy["outputs"]
    excluded_outputs = {
        output_paths["manifest"],
        output_paths["package_manifest"],
        output_paths["identity"],
    }
    if excluded_outputs.intersection(paths):
        raise ManifestError("generated identity outputs cannot be semantic input files")
    return [
        {"path": path, "sha256": hashlib.sha256(entries[path][1]).hexdigest()}
        for path in paths
    ]


def semantic_identity(inputs: list[dict[str, str]]) -> str:
    """Hash the sorted canonical path/digest encoding for a discovered manifest."""
    normalized: list[tuple[bytes, bytes, str]] = []
    for entry in inputs:
        relative = _normalized_relative(entry["path"])
        if relative.startswith("/") or ":" in relative.split("/", 1)[0]:
            raise ManifestError(f"semantic input path must be repository-relative: {relative!r}")
        if any(part in {"", ".", ".."} for part in relative.split("/")):
            raise ManifestError(f"semantic input path is not canonical: {relative!r}")
        path_bytes = relative.encode("utf-8")
        try:
            digest_bytes = bytes.fromhex(entry["sha256"])
        except ValueError as exc:
            raise ManifestError(f"invalid SHA-256 digest for {relative!r}") from exc
        if len(digest_bytes) != 32:
            raise ManifestError(f"invalid SHA-256 digest for {relative!r}")
        normalized.append((path_bytes, digest_bytes, relative))
    normalized.sort(key=lambda item: item[0])
    if any(left[0] == right[0] for left, right in zip(normalized, normalized[1:])):
        raise ManifestError("duplicate normalized paths in semantic identity inputs")
    digest = hashlib.sha256()
    digest.update(IDENTITY_PREFIX)
    digest.update(struct.pack(">Q", len(normalized)))
    for path_bytes, digest_bytes, _relative in normalized:
        digest.update(struct.pack(">I", len(path_bytes)))
        digest.update(path_bytes)
        digest.update(digest_bytes)
    return IDENTITY_FORMAT + digest.hexdigest()


def render_outputs(root: Path) -> tuple[bytes, bytes, bytes]:
    inputs = discover_inputs(root)
    identity = semantic_identity(inputs)
    manifest = {
        "format_version": 1,
        "hash_algorithm": "sha256",
        "semantic_identity": identity,
        "inputs": inputs,
    }
    identity_document = {
        "format_version": 1,
        "semantic_identity": identity,
        "manifest": "repopact/_semantic-input-manifest.json",
    }
    manifest_bytes = _pretty_json(manifest)
    return manifest_bytes, manifest_bytes, _pretty_json(identity_document)


def check(root: Path) -> None:
    policy = json.loads((root / POLICY_PATH).read_text(encoding="utf-8"))
    expected_manifest, expected_package_manifest, expected_identity = render_outputs(root)
    outputs = policy["outputs"]
    for relative, expected in (
        (outputs["manifest"], expected_manifest),
        (outputs["package_manifest"], expected_package_manifest),
        (outputs["identity"], expected_identity),
    ):
        path = root / relative
        if not path.is_file():
            raise ManifestError(f"generated semantic identity output is missing: {relative}")
        if path.read_bytes() != expected:
            raise ManifestError(
                f"semantic identity is stale: {relative}; inspect inputs and run "
                "the explicit --update command to propose a baseline change"
            )


def update(root: Path) -> str:
    policy = json.loads((root / POLICY_PATH).read_text(encoding="utf-8"))
    manifest, package_manifest, identity = render_outputs(root)
    for relative, content in (
        (policy["outputs"]["manifest"], manifest),
        (policy["outputs"]["package_manifest"], package_manifest),
        (policy["outputs"]["identity"], identity),
    ):
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(content)
    document = json.loads(identity.decode("utf-8"))
    return document["semantic_identity"]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="recompute and compare without writing")
    mode.add_argument("--update", action="store_true", help="explicitly write the semantic baseline")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args(argv)
    try:
        if args.check:
            check(args.root.resolve())
            print("Semantic input manifest is current.")
        else:
            print("Updated semantic identity:", update(args.root.resolve()))
    except (ManifestError, OSError, KeyError, TypeError, ValueError) as exc:
        print(f"semantic identity error: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
