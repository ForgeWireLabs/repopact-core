"""Fail-closed inventory and reclamation for explicitly owned local CI artifacts."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import time
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


OWNER_FORMAT = "repopact-local-validation-ownership-v1"
MIN_CI_FREE_BYTES = 12 * 1024**3
MAX_INVENTORY_FILES = 100_000
MAX_INVENTORY_BYTES = 32 * 1024**3
_STATE_DIR = "repopact-local-validation"


class LocalStorageError(RuntimeError):
    """Raised when local-storage ownership or safety cannot be established."""


def _git_env() -> dict[str, str]:
    env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    env["GIT_OPTIONAL_LOCKS"] = "0"
    env["GIT_TERMINAL_PROMPT"] = "0"
    return env


def _git(root: Path, *args: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    try:
        result = subprocess.run(
            ["git", *args],
            cwd=root,
            env=_git_env(),
            shell=False,
            check=False,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=20,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        raise LocalStorageError(f"git {' '.join(args)} could not be inspected: {exc}") from exc
    if check and result.returncode:
        detail = result.stderr.strip()[-1000:]
        raise LocalStorageError(f"git {' '.join(args)} failed ({result.returncode}): {detail}")
    return result


def _canonical(path: Path, *, strict: bool = False) -> Path:
    try:
        return path.expanduser().resolve(strict=strict)
    except OSError as exc:
        raise LocalStorageError(f"cannot resolve path {path}: {exc}") from exc


def _common_dir(root: Path) -> Path:
    root = _canonical(root, strict=True)
    result = _git(root, "rev-parse", "--git-common-dir")
    path = Path(result.stdout.strip())
    if not path.is_absolute():
        path = root / path
    return _canonical(path, strict=True)


def _registry_path(root: Path) -> Path:
    return _common_dir(root) / _STATE_DIR / "owned-artifacts.json"


def _origin(root: Path) -> str | None:
    result = _git(root, "config", "--get", "remote.origin.url", check=False)
    return result.stdout.strip() if result.returncode == 0 else None


def _read_registry(root: Path) -> tuple[Path, dict[str, Any]]:
    root = _canonical(root, strict=True)
    common = _common_dir(root)
    path = common / _STATE_DIR / "owned-artifacts.json"
    _reject_symlink_components(path.parent)
    if path.is_symlink():
        raise LocalStorageError(f"ownership registry must not be a symlink: {path}")
    if not path.exists():
        return path, {
            "format": OWNER_FORMAT,
            "repository_common_dir": str(common),
            "origin": _origin(root),
            "artifacts": [],
        }
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise LocalStorageError(f"ownership registry is unreadable: {path}: {exc}") from exc
    if (
        not isinstance(document, dict)
        or document.get("format") != OWNER_FORMAT
        or document.get("repository_common_dir") != str(common)
        or document.get("origin") != _origin(root)
        or not isinstance(document.get("artifacts"), list)
    ):
        raise LocalStorageError(f"ownership registry does not match this repository: {path}")
    return path, document


def _write_registry(path: Path, document: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    _reject_symlink_components(path.parent)
    if path.is_symlink():
        raise LocalStorageError(f"ownership registry must not be a symlink: {path}")
    temporary = path.with_name(f"{path.name}.{uuid.uuid4().hex}.tmp")
    try:
        with temporary.open("x", encoding="utf-8", newline="\n") as handle:
            json.dump(document, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
    except OSError as exc:
        temporary.unlink(missing_ok=True)
        raise LocalStorageError(f"cannot update ownership registry {path}: {exc}") from exc


def _entry(document: dict[str, Any], artifact_id: str) -> dict[str, Any]:
    matches = [item for item in document["artifacts"] if item.get("id") == artifact_id]
    if len(matches) != 1:
        raise LocalStorageError(f"owned artifact id is missing or ambiguous: {artifact_id}")
    return matches[0]


def _path_in(path: Path, parent: Path) -> bool:
    try:
        path.relative_to(parent)
        return True
    except ValueError:
        return False


def _reject_protected_path(root: Path, path: Path) -> None:
    root = _canonical(root, strict=True)
    path = _canonical(path)
    common = _common_dir(root)
    evidence = _canonical(root / "evidence")
    if path == root or path == common or _path_in(path, evidence):
        raise LocalStorageError(f"protected repository or evidence path cannot be reclaimed: {path}")
    if _path_in(root, path) or _path_in(common, path):
        raise LocalStorageError(f"an artifact path cannot contain the repository: {path}")


def _reject_symlink_components(path: Path) -> None:
    current = path
    while True:
        if current.exists() or current.is_symlink():
            is_junction = getattr(current, "is_junction", lambda: False)
            if current.is_symlink() or is_junction():
                raise LocalStorageError(f"symlink or junction path is not eligible: {current}")
        if current.parent == current:
            break
        current = current.parent


def _new_entry(root: Path, kind: str, path: Path, **details: Any) -> dict[str, Any]:
    return {
        "id": uuid.uuid4().hex,
        "kind": kind,
        "path": str(path),
        "repository_common_dir": str(_common_dir(root)),
        "origin": _origin(root),
        "created_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "state": "active",
        **details,
    }


def create_owned_target(root: Path, target: Path) -> dict[str, Any]:
    """Create a previously absent target directory and explicitly register it."""
    root = _canonical(root, strict=True)
    target = _canonical(target if target.is_absolute() else root / target)
    _reject_protected_path(root, target)
    _reject_symlink_components(target.parent)
    if target.exists() or target.is_symlink():
        raise LocalStorageError(f"refusing to claim an existing Cargo target: {target}")
    if not target.parent.is_dir():
        raise LocalStorageError(f"Cargo target parent must already exist: {target.parent}")
    registry_path, document = _read_registry(root)
    if any(item.get("path") == str(target) for item in document["artifacts"]):
        raise LocalStorageError(f"Cargo target is already registered: {target}")
    target.mkdir()
    item = _new_entry(root, "cargo-target", target, snapshot=None)
    document["artifacts"].append(item)
    try:
        _write_registry(registry_path, document)
    except LocalStorageError:
        target.rmdir()
        raise
    return item


def _snapshot(path: Path) -> dict[str, Any]:
    files: list[dict[str, Any]] = []
    directories: list[str] = []
    total_bytes = 0
    for current, dirnames, filenames in os.walk(path, topdown=True, followlinks=False):
        current_path = Path(current)
        for name in list(dirnames):
            child = current_path / name
            if child.is_symlink() or getattr(child, "is_junction", lambda: False)():
                raise LocalStorageError(f"target contains a symlink or junction: {child}")
            directories.append(child.relative_to(path).as_posix())
            if len(directories) > MAX_INVENTORY_FILES:
                raise LocalStorageError(
                    f"target exceeds the bounded ownership inventory ({MAX_INVENTORY_FILES} files/directories)"
                )
        for name in filenames:
            child = current_path / name
            if child.is_symlink():
                raise LocalStorageError(f"target contains a symlink: {child}")
            try:
                stat = child.stat(follow_symlinks=False)
                if not child.is_file():
                    raise LocalStorageError(f"target contains a non-regular file: {child}")
                digest = _file_digest(child)
                after = child.stat(follow_symlinks=False)
            except OSError as exc:
                raise LocalStorageError(f"cannot inspect target file {child}: {exc}") from exc
            if (stat.st_size, stat.st_mtime_ns) != (after.st_size, after.st_mtime_ns):
                raise LocalStorageError(f"target file changed while being inventoried: {child}")
            total_bytes += stat.st_size
            files.append(
                {
                    "path": child.relative_to(path).as_posix(),
                    "size": stat.st_size,
                    "sha256": digest,
                }
            )
            if len(files) > MAX_INVENTORY_FILES or total_bytes > MAX_INVENTORY_BYTES:
                raise LocalStorageError(
                    f"target exceeds the bounded ownership inventory ({MAX_INVENTORY_FILES} files, {MAX_INVENTORY_BYTES} bytes)"
                )
    files.sort(key=lambda item: item["path"])
    directories.sort()
    return {"files": files, "directories": directories, "total_bytes": total_bytes}


def _file_digest(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def seal_owned_target(root: Path, target: Path) -> dict[str, Any]:
    """Seal a tool-created target with a bounded exact-content inventory."""
    root = _canonical(root, strict=True)
    target = _canonical(target, strict=True)
    _reject_protected_path(root, target)
    registry_path, document = _read_registry(root)
    item = next(
        (entry for entry in document["artifacts"] if entry.get("path") == str(target)),
        None,
    )
    if item is None or item.get("kind") != "cargo-target":
        raise LocalStorageError(f"target has no explicit tool-ownership entry: {target}")
    if item.get("state") not in {"active", "sealed"}:
        raise LocalStorageError(f"target cannot be sealed from state {item.get('state')!r}")
    snapshot = _snapshot(target)
    item.update(state="sealed", sealed_at=datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"), snapshot=snapshot)
    _write_registry(registry_path, document)
    return item


def _snapshot_matches(current: dict[str, Any], expected: dict[str, Any], *, allow_missing: bool) -> bool:
    expected_files = {item["path"]: item for item in expected.get("files", [])}
    current_files = {item["path"]: item for item in current.get("files", [])}
    expected_dirs = set(expected.get("directories", []))
    current_dirs = set(current.get("directories", []))
    if not set(current_files).issubset(expected_files) or not current_dirs.issubset(expected_dirs):
        return False
    if not allow_missing and (set(current_files) != set(expected_files) or current_dirs != expected_dirs):
        return False
    return all(current_files[key] == expected_files[key] for key in current_files)


def cleanup_owned_target(root: Path, target: Path, *, apply: bool = False) -> dict[str, Any]:
    """Dry-run by default; delete only an exact sealed target inventory."""
    root = _canonical(root, strict=True)
    target = _canonical(target)
    _reject_protected_path(root, target)
    registry_path, document = _read_registry(root)
    item = next(
        (entry for entry in document["artifacts"] if entry.get("path") == str(target)),
        None,
    )
    if item is None or item.get("kind") != "cargo-target":
        raise LocalStorageError(f"refusing unregistered target path: {target}")
    if item.get("state") not in {"sealed", "cleanup-in-progress"}:
        raise LocalStorageError(f"target is not sealed for cleanup (state={item.get('state')!r})")
    expected = item.get("snapshot")
    if not isinstance(expected, dict):
        raise LocalStorageError(f"target ownership snapshot is missing: {target}")
    if not target.exists():
        if apply:
            document["artifacts"].remove(item)
            _write_registry(registry_path, document)
        return {"action": "already-absent", "path": str(target), "removed_bytes": 0}
    _reject_symlink_components(target)
    current = _snapshot(target)
    resuming = item.get("state") == "cleanup-in-progress"
    if not _snapshot_matches(current, expected, allow_missing=resuming):
        raise LocalStorageError(f"target contents differ from the sealed ownership inventory: {target}")
    result = {
        "action": "would-remove" if not apply else "removed",
        "path": str(target),
        "files": len(current["files"]),
        "bytes": current["total_bytes"],
        "dry_run": not apply,
    }
    if not apply:
        return result

    item["state"] = "cleanup-in-progress"
    _write_registry(registry_path, document)
    for record in expected["files"]:
        candidate = target / Path(*Path(record["path"]).parts)
        if not candidate.exists():
            continue
        if candidate.is_symlink() or not candidate.is_file():
            raise LocalStorageError(f"refusing changed target file during cleanup: {candidate}")
        current_stat = candidate.stat()
        digest = _file_digest(candidate)
        after_stat = candidate.stat()
        if (
            digest != record["sha256"]
            or current_stat.st_size != record["size"]
            or (current_stat.st_size, current_stat.st_mtime_ns)
            != (after_stat.st_size, after_stat.st_mtime_ns)
        ):
            raise LocalStorageError(f"refusing changed target file during cleanup: {candidate}")
        candidate.unlink()
    for relative in sorted(expected["directories"], key=lambda value: value.count("/"), reverse=True):
        (target / Path(*Path(relative).parts)).rmdir()
    target.rmdir()
    document["artifacts"].remove(item)
    _write_registry(registry_path, document)
    result["removed_bytes"] = result.pop("bytes")
    return result


def _worktrees(root: Path) -> list[dict[str, Any]]:
    output = _git(root, "worktree", "list", "--porcelain").stdout
    items: list[dict[str, Any]] = []
    current: dict[str, Any] = {}
    for line in [*output.splitlines(), ""]:
        if not line:
            if current:
                items.append(current)
                current = {}
            continue
        key, _, value = line.partition(" ")
        if key == "worktree":
            candidate = Path(value).expanduser().absolute()
            is_junction = getattr(candidate, "is_junction", lambda: False)
            current["path"] = str(candidate if candidate.is_symlink() or is_junction() else _canonical(candidate))
        elif key == "HEAD":
            current["head"] = value
        elif key == "branch":
            current["branch"] = value.removeprefix("refs/heads/")
        elif key == "locked":
            current["locked"] = True
        elif key == "detached":
            current["detached"] = True
        elif key == "prunable":
            current["prunable"] = True
    return items


def create_owned_worktree(root: Path, path: Path, revision: str) -> dict[str, Any]:
    """Create and register a detached worktree; existing paths are never claimed."""
    root = _canonical(root, strict=True)
    path = _canonical(path if path.is_absolute() else root / path)
    _reject_protected_path(root, path)
    _reject_symlink_components(path.parent)
    if path.exists() or path.is_symlink():
        raise LocalStorageError(f"refusing to claim an existing worktree path: {path}")
    if not path.parent.is_dir():
        raise LocalStorageError(f"worktree parent must already exist: {path.parent}")
    resolved = _git(root, "rev-parse", "--verify", f"{revision}^{{commit}}")
    commit = resolved.stdout.strip()
    if len(commit) != 40:
        raise LocalStorageError("worktree revision did not resolve to a full commit id")
    registry_path, document = _read_registry(root)
    _git(root, "worktree", "add", "--detach", str(path), commit)
    item = _new_entry(root, "worktree", path, commit=commit, detached=True)
    document["artifacts"].append(item)
    try:
        _write_registry(registry_path, document)
    except LocalStorageError:
        # No force removal: if Git sees any content change, it will refuse.
        _git(root, "worktree", "remove", str(path), check=False)
        raise
    return item


def cleanup_owned_worktree(root: Path, path: Path, *, apply: bool = False) -> dict[str, Any]:
    """Dry-run by default; remove only a registered, unchanged, clean detached worktree."""
    root = _canonical(root, strict=True)
    path = _canonical(path)
    _reject_protected_path(root, path)
    if path == root:
        raise LocalStorageError("the active repository checkout cannot be reclaimed")
    registry_path, document = _read_registry(root)
    item = next(
        (entry for entry in document["artifacts"] if entry.get("path") == str(path)),
        None,
    )
    if item is None or item.get("kind") != "worktree":
        raise LocalStorageError(f"refusing unregistered worktree path: {path}")
    registered = next((entry for entry in _worktrees(root) if entry.get("path") == str(path)), None)
    if registered is None:
        if apply:
            document["artifacts"].remove(item)
            _write_registry(registry_path, document)
        return {"action": "already-absent", "path": str(path), "removed_bytes": 0}
    if registered.get("locked") or registered.get("prunable"):
        raise LocalStorageError(f"locked or prunable worktree is not eligible: {path}")
    if not registered.get("detached") or registered.get("head") != item.get("commit"):
        raise LocalStorageError(f"worktree moved from its registered detached commit: {path}")
    status = _git(path, "status", "--porcelain", "--untracked-files=all").stdout
    if status:
        raise LocalStorageError(f"worktree has user changes; refusing cleanup: {path}")
    ignored = _git(
        path,
        "status",
        "--porcelain",
        "--untracked-files=all",
        "--ignored=traditional",
    ).stdout
    if any(line.startswith("!! ") for line in ignored.splitlines()):
        raise LocalStorageError(f"worktree contains ignored files; refusing cleanup: {path}")
    size = tree_usage(path)
    if size["truncated"] or size["bytes"] > MAX_INVENTORY_BYTES:
        raise LocalStorageError(
            f"worktree exceeds the bounded cleanup inventory ({MAX_INVENTORY_FILES} files, {MAX_INVENTORY_BYTES} bytes): {path}"
        )
    result = {
        "action": "would-remove" if not apply else "removed",
        "path": str(path),
        "bytes": size["bytes"],
        "dry_run": not apply,
    }
    if not apply:
        return result
    item["state"] = "cleanup-in-progress"
    _write_registry(registry_path, document)
    _git(root, "worktree", "remove", str(path))
    document["artifacts"].remove(item)
    _write_registry(registry_path, document)
    result["removed_bytes"] = result.pop("bytes")
    return result


def _existing_parent(path: Path) -> Path:
    current = path
    while not current.exists():
        if current.parent == current:
            raise LocalStorageError(f"no existing parent for disk-space check: {path}")
        current = current.parent
    return _canonical(current, strict=True)


def disk_preflight(
    root: Path,
    *,
    minimum_free_bytes: int = MIN_CI_FREE_BYTES,
    cargo_target_dir: str | None = None,
    usage_fn=shutil.disk_usage,
) -> dict[str, Any]:
    """Check every volume needed for CI outputs and Cargo artifacts."""
    root = _canonical(root, strict=True)
    if minimum_free_bytes < 0:
        raise ValueError("minimum_free_bytes must be nonnegative")
    target_text = cargo_target_dir if cargo_target_dir is not None else os.environ.get("CARGO_TARGET_DIR")
    target = Path(target_text) if target_text else root / "rust" / "target"
    if not target.is_absolute():
        target = root / target
    locations = [("repository-and-evidence", root), ("cargo-target", target)]
    volumes: dict[str, dict[str, Any]] = {}
    for purpose, location in locations:
        existing = _existing_parent(location)
        usage = usage_fn(existing)
        try:
            device = os.stat(existing).st_dev
        except OSError:
            device = os.path.splitdrive(str(existing))[0].casefold() or str(existing.anchor).casefold()
        volume = str(device)
        item = volumes.setdefault(
            volume,
            {
                "volume": os.path.splitdrive(str(existing))[0].casefold() or volume,
                "checked_path": str(existing),
                "purposes": [],
                "total_bytes": usage.total,
                "used_bytes": usage.used,
                "free_bytes": usage.free,
            },
        )
        item["purposes"].append(purpose)
    checks = list(volumes.values())
    for item in checks:
        item["ready"] = item["free_bytes"] >= minimum_free_bytes
        item["required_free_bytes"] = minimum_free_bytes
    return {
        "ready": all(item["ready"] for item in checks),
        "minimum_free_bytes_per_volume": minimum_free_bytes,
        "cargo_target_dir": str(target),
        "volumes": checks,
    }


def tree_usage(path: Path, *, max_files: int = MAX_INVENTORY_FILES) -> dict[str, Any]:
    """Bounded size inventory; symlinks and junctions are reported without traversal."""
    path = path.expanduser().absolute()
    try:
        _reject_symlink_components(path)
    except LocalStorageError:
        return {
            "path": str(path.absolute()),
            "exists": True,
            "files": 0,
            "bytes": 0,
            "truncated": False,
            "excluded": "symlink-or-junction",
        }
    path = _canonical(path)
    if not path.exists():
        return {"path": str(path), "exists": False, "files": 0, "bytes": 0, "truncated": False}
    files = 0
    total = 0
    truncated = False
    for current, dirnames, filenames in os.walk(path, topdown=True, followlinks=False):
        current_path = Path(current)
        dirnames[:] = [name for name in dirnames if not (current_path / name).is_symlink()]
        for name in filenames:
            candidate = current_path / name
            try:
                stat = candidate.lstat()
            except OSError:
                truncated = True
                continue
            files += 1
            total += stat.st_size
            if files >= max_files:
                truncated = True
                break
        if truncated:
            break
    return {"path": str(path), "exists": True, "files": files, "bytes": total, "truncated": truncated}


def inventory(root: Path) -> dict[str, Any]:
    """Read-only bounded inventory of known builds, evidence, environments and worktrees."""
    root = _canonical(root, strict=True)
    preflight = disk_preflight(root)
    known = [
        ("default-cargo-target", root / "rust" / "target"),
        ("root-cargo-target", root / "target"),
        ("python-build", root / "build"),
        ("distribution-build", root / "dist"),
        ("python-venv", root / ".venv"),
        ("evidence-runs-immutable", root / "evidence" / "runs"),
    ]
    env_target = os.environ.get("CARGO_TARGET_DIR")
    if env_target:
        ext = Path(env_target)
        if not ext.is_absolute():
            ext = root / ext
        if _canonical(ext) != _canonical(root / "rust" / "target"):
            known.append(("configured-cargo-target", ext))
    paths = []
    for purpose, path in known:
        item = tree_usage(path)
        item["purpose"] = purpose
        item["cleanup_policy"] = "never-auto-delete-immutable-evidence" if purpose.startswith("evidence-") else "ownership-marker-and-dry-run-required"
        paths.append(item)
    registry_path, document = _read_registry(root)
    registered = {item.get("path"): item for item in document["artifacts"]}
    worktrees = []
    for item in _worktrees(root):
        path = Path(item["path"])
        path_replaced = path.is_symlink() or getattr(path, "is_junction", lambda: False)()
        status = (
            _git(path, "status", "--porcelain", "--untracked-files=all").stdout
            if path.exists() and not path_replaced
            else "unavailable"
        )
        measured = tree_usage(path)
        worktrees.append(
            {
                **item,
                **measured,
                "dirty": bool(status) if status != "unavailable" else None,
                "path_replaced": path_replaced,
                "ownership": registered.get(item["path"], {}).get("id", "unregistered"),
                "cleanup_eligible": bool(
                    item.get("detached")
                    and not item.get("locked")
                    and not item.get("prunable")
                    and not status
                    and not path_replaced
                    and registered.get(item["path"], {}).get("kind") == "worktree"
                    and registered.get(item["path"], {}).get("commit") == item.get("head")
                    and item["path"] != str(root)
                ),
            }
        )
    return {
        "format": "repopact-local-storage-inventory-v1",
        "created_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "repository_root": str(root),
        "git_common_dir": str(_common_dir(root)),
        "free_space": preflight,
        "known_paths": paths,
        "worktrees": worktrees,
        "ownership_registry": str(registry_path),
        "registered_artifacts": [_registered_artifact_summary(item) for item in document["artifacts"]],
        "immutable_evidence_paths_are_excluded_from_cleanup": True,
    }


def _registered_artifact_summary(item: dict[str, Any]) -> dict[str, Any]:
    snapshot = item.get("snapshot")
    if not isinstance(snapshot, dict):
        snapshot = {}
    return {
        key: value for key, value in item.items() if key != "snapshot"
    } | {
        "snapshot_files": len(snapshot.get("files", [])),
        "snapshot_bytes": snapshot.get("total_bytes"),
    }
