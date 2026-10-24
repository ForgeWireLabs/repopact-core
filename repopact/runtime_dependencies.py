"""Runtime checks for the exact Python dependency closure governed by WI076."""

from __future__ import annotations

import importlib.metadata
import json
import platform
import re
import sys
from pathlib import Path
from typing import Any


class RuntimeDependencyError(RuntimeError):
    """The installed dependency set differs from the reviewed exact lock."""


_REQUIREMENT = re.compile(
    r"^([A-Za-z0-9_.-]+)==([^\s;\\]+)(?:\s*;\s*(.*?))?\s*\\?$"
)
_MARKER_ATOM = re.compile(r"^\s*([A-Za-z_]+)\s*(==|!=|<=|>=|<|>)\s*'([^']*)'\s*$")


def _normalize_name(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def _marker_applies(marker: str | None) -> bool:
    if not marker:
        return True
    variables = {
        "platform_python_implementation": platform.python_implementation(),
        "implementation_name": sys.implementation.name,
        "python_version": f"{sys.version_info[0]}.{sys.version_info[1]}",
        "python_full_version": ".".join(str(part) for part in sys.version_info[:3]),
    }
    for atom in marker.split(" and "):
        match = _MARKER_ATOM.fullmatch(atom)
        if not match or match.group(1) not in variables:
            raise RuntimeDependencyError(f"unsupported runtime-lock marker: {marker!r}")
        variable, operator, expected = match.groups()
        actual = variables[variable]
        if variable in {"python_version", "python_full_version"}:
            try:
                actual_version = tuple(int(part) for part in actual.split("."))
                expected_version = tuple(int(part) for part in expected.split("."))
            except ValueError as exc:
                raise RuntimeDependencyError(f"unsupported runtime-lock marker: {marker!r}") from exc
            comparison = {
                "==": actual_version == expected_version,
                "!=": actual_version != expected_version,
                "<": actual_version < expected_version,
                "<=": actual_version <= expected_version,
                ">": actual_version > expected_version,
                ">=": actual_version >= expected_version,
            }[operator]
        elif operator in {"==", "!="}:
            comparison = (actual == expected) if operator == "==" else (actual != expected)
        else:
            raise RuntimeDependencyError(f"unsupported runtime-lock marker: {marker!r}")
        if not comparison:
            return False
    return True


def _read_lock(path: Path) -> dict[str, tuple[str, str | None]]:
    packages: dict[str, tuple[str, str | None]] = {}
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        raise RuntimeDependencyError(f"exact runtime lock is missing: {path}") from exc
    for line in lines:
        if not line or line.startswith("#") or line[0].isspace():
            continue
        match = _REQUIREMENT.fullmatch(line)
        if not match:
            raise RuntimeDependencyError(f"unsupported requirement in runtime lock: {line!r}")
        name, version, marker = match.groups()
        normalized = _normalize_name(name)
        if normalized in packages:
            raise RuntimeDependencyError(f"duplicate package in runtime lock: {name}")
        packages[normalized] = (version, marker)
    if not packages:
        raise RuntimeDependencyError("exact runtime lock contains no package pins")
    return packages


def verify_runtime_dependencies(
    *,
    enforcement: bool = False,
    root: Path | None = None,
) -> dict[str, str]:
    """Verify exact installed versions for the requested governed dependency group."""
    if root is None:
        package_root = Path(__file__).resolve().parent
    else:
        repository_root = root.resolve()
        package_root = repository_root / "repopact" if (repository_root / "repopact").is_dir() else repository_root
    group_path = package_root / "runtime-dependency-groups.json"
    try:
        group_document: Any = json.loads(group_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise RuntimeDependencyError(
            f"runtime dependency group declaration is unavailable: {exc}"
        ) from exc
    if not isinstance(group_document, dict) or group_document.get("format_version") != 1:
        raise RuntimeDependencyError("unsupported runtime dependency group format")
    lock_relative = group_document.get("lock")
    groups = group_document.get("groups")
    if not isinstance(lock_relative, str) or not isinstance(groups, dict):
        raise RuntimeDependencyError("runtime dependency group declaration is malformed")
    lock_path = (package_root / lock_relative).resolve()
    try:
        lock_path.relative_to(package_root)
    except ValueError as exc:
        raise RuntimeDependencyError("runtime dependency lock escapes the package root") from exc
    locked = _read_lock(lock_path)

    declared: dict[str, set[str]] = {}
    for group_name, names in groups.items():
        if not isinstance(group_name, str) or not isinstance(names, list):
            raise RuntimeDependencyError("runtime dependency group entries are malformed")
        normalized_names = set()
        for name in names:
            if not isinstance(name, str) or not name:
                raise RuntimeDependencyError("runtime dependency group contains an invalid name")
            normalized_names.add(_normalize_name(name))
        declared[group_name] = normalized_names
    all_declared = set().union(*declared.values()) if declared else set()
    if all_declared != set(locked):
        missing = sorted(set(locked) - all_declared)
        unknown = sorted(all_declared - set(locked))
        raise RuntimeDependencyError(
            f"runtime lock/group coverage mismatch; unclassified={missing}, absent={unknown}"
        )
    if "core" not in declared or (enforcement and "enforcement" not in declared):
        raise RuntimeDependencyError("required runtime dependency group is not declared")
    selected = set(declared["core"])
    if enforcement:
        selected.update(declared["enforcement"])

    installed: dict[str, str] = {}
    mismatches: list[str] = []
    for name in sorted(selected):
        locked_version, marker = locked[name]
        if not _marker_applies(marker):
            continue
        try:
            actual_version = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            mismatches.append(f"{name}: missing (expected {locked_version})")
            continue
        installed[name] = actual_version
        if actual_version != locked_version:
            mismatches.append(f"{name}: installed {actual_version}, expected {locked_version}")
    if mismatches:
        raise RuntimeDependencyError("; ".join(mismatches))
    return installed
