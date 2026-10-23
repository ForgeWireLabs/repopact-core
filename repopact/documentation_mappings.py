"""Python compatibility helpers for WI047 structural documentation freshness."""

from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path

DEFAULT_MAPPING_PATH = "governance/documentation-mappings.json"
_MAPPING_ID = re.compile(r"^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?$")


def _safe_path(root: Path, relative: object) -> Path | None:
    if not isinstance(relative, str) or not relative or relative.startswith("/"):
        return None
    if "\\" in relative or ":" in relative or any(ord(char) < 32 for char in relative):
        return None
    if any(part in {"", ".", ".."} for part in relative.split("/")):
        return None
    candidate = root / Path(*relative.split("/"))
    try:
        resolved = candidate.resolve(strict=False)
        resolved.relative_to(root.resolve())
    except (OSError, RuntimeError, ValueError):
        return None
    return resolved


def _paths_digest(root: Path, paths: object) -> str:
    if not isinstance(paths, list) or not paths:
        raise ValueError("mapped path list must be a non-empty array")
    if any(not isinstance(relative, str) for relative in paths):
        raise ValueError("mapped paths must be strings")
    digest = hashlib.sha256()
    for relative in sorted(paths):
        path = _safe_path(root, relative)
        if path is None:
            raise ValueError(f"'{relative}' is not a safe repository-relative path")
        if not path.is_file():
            raise ValueError(f"mapped file '{relative}' is unavailable or is not a regular file")
        content = path.read_bytes()
        encoded = relative.encode("utf-8")
        digest.update(len(encoded).to_bytes(8, "big"))
        digest.update(encoded)
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return digest.hexdigest()


def _configuration(root: Path) -> tuple[Path | None, list[tuple[str, str, Path]]]:
    owners_path = root / "governance" / "owners.json"
    try:
        owners = json.loads(owners_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return None, [("documentation.mapping-config-invalid", str(exc), owners_path)]
    if not isinstance(owners, dict) or "documentation_mappings" not in owners:
        return None, []
    config = owners.get("documentation_mappings")
    if not isinstance(config, dict) or not isinstance(config.get("enabled"), bool):
        return None, [(
            "documentation.mapping-config-invalid",
            "documentation_mappings must be an object with a boolean enabled field",
            owners_path,
        )]
    if config.get("enabled") is not True:
        return None, []
    relative = config.get("path", DEFAULT_MAPPING_PATH)
    path = _safe_path(root, relative)
    if path is None:
        return None, [(
            "documentation.mapping-config-invalid",
            "documentation_mappings.path must be a repository-relative POSIX path without dot-segments",
            owners_path,
        )]
    return path, []


def inspect(root: Path) -> list[tuple[str, str, Path]]:
    """Return structural drift findings without changing repository files."""
    mapping_path, findings = _configuration(root)
    if mapping_path is None:
        return findings
    try:
        data = json.loads(mapping_path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        return [("documentation.mappings-invalid-json", str(exc), mapping_path)]
    mappings = data.get("mappings") if isinstance(data, dict) else None
    if not isinstance(mappings, list):
        return [("documentation.mappings-invalid", "documentation mapping file must contain a mappings array", mapping_path)]
    seen: set[str] = set()
    for mapping in mappings:
        if not isinstance(mapping, dict):
            continue
        mapping_id = mapping.get("id")
        kind = mapping.get("kind")
        if not isinstance(mapping_id, str) or not _MAPPING_ID.fullmatch(mapping_id):
            continue
        if mapping_id in seen:
            findings.append(("documentation.mappings-invalid", f"duplicate mapping id '{mapping_id}'", mapping_path))
            continue
        seen.add(mapping_id)
        if kind not in {"manual", "generated"}:
            continue
        snapshot = mapping.get("snapshot")
        try:
            source_digest = _paths_digest(root, mapping.get("source_paths"))
            documentation_digest = _paths_digest(root, mapping.get("documentation_paths"))
        except (OSError, ValueError) as exc:
            findings.append(("documentation.mapping-path-invalid", f"mapping '{mapping_id}': {exc}", mapping_path))
            continue
        expected_source = snapshot.get("source_digest") if isinstance(snapshot, dict) else None
        expected_docs = snapshot.get("documentation_digest") if isinstance(snapshot, dict) else None
        if expected_source == source_digest and expected_docs == documentation_digest:
            continue
        source_unchanged = expected_source == source_digest
        docs_unchanged = expected_docs == documentation_digest
        if not source_unchanged and docs_unchanged:
            if kind == "generated":
                code = "documentation.generated-stale"
                reason = "mapped source/generator changed while generated documentation stayed at its reviewed bytes"
            else:
                code = "documentation.mapping-stale"
                reason = "mapped source changed while related documentation stayed at its reviewed bytes"
        elif source_unchanged:
            code = "documentation.mapping-review-required"
            reason = "mapped documentation changed and requires an explicit freshness review"
        else:
            code = "documentation.mapping-review-required"
            reason = "mapped source and documentation bytes changed; reconcile the pair and refresh the reviewed snapshot"
        findings.append((
            code,
            f"mapping '{mapping_id}': {reason}; use `repopact documentation snapshot {mapping_id}` after review/regeneration and merge the returned hashes into the mapping record",
            mapping_path,
        ))
    return findings


def compute_snapshot(root: Path, mapping_id: str) -> dict[str, str]:
    """Compute current digests for a mapping without updating its record."""
    mapping_path, errors = _configuration(root)
    if errors:
        raise ValueError(errors[0][1])
    if mapping_path is None:
        raise ValueError("documentation mappings are not enabled in governance/owners.json")
    data = json.loads(mapping_path.read_text(encoding="utf-8"))
    mappings = data.get("mappings", []) if isinstance(data, dict) else []
    mapping = next((item for item in mappings if isinstance(item, dict) and item.get("id") == mapping_id), None)
    if mapping is None:
        raise ValueError(f"no documentation mapping with id '{mapping_id}'")
    return {
        "source_digest": _paths_digest(root, mapping.get("source_paths")),
        "documentation_digest": _paths_digest(root, mapping.get("documentation_paths")),
    }
