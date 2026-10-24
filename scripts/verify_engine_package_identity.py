"""Verify semantic identity agreement in a Maturin sdist and wheel."""

from __future__ import annotations

import argparse
import email
import os
import hashlib
import json
import re
import subprocess
import sys
import tarfile
import tempfile
import zipfile
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
    """A built archive omits or changes governed semantic identity inputs."""


def _load_json(raw: bytes, description: str) -> dict[str, Any]:
    try:
        value = json.loads(raw)
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ArchiveIdentityError(f"{description} is invalid JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise ArchiveIdentityError(f"{description} must be a JSON object")
    return value


def _safe_manifest_path(value: str) -> str:
    path = PurePosixPath(value)
    if (
        path.is_absolute()
        or path.as_posix() != value
        or any(part in {"", ".", ".."} for part in path.parts)
    ):
        raise ArchiveIdentityError(f"semantic input path is not safe in an archive: {value!r}")
    return path.as_posix()


def source_identity(root: Path) -> str:
    root = root.resolve()
    try:
        check(root)
        identity = _load_json((root / "repopact" / "_semantic_identity.json").read_bytes(), "source identity")
    except (OSError, ManifestError) as exc:
        raise ArchiveIdentityError(f"source semantic identity is not current: {exc}") from exc
    value = identity.get("semantic_identity")
    if not isinstance(value, str):
        raise ArchiveIdentityError("source identity omitted semantic_identity")
    return value


def verify_sdist(path: Path, expected_identity: str) -> dict[str, Any]:
    root_name = path.name.removesuffix(".tar.gz")
    prefix = root_name + "/"
    with tarfile.open(path, "r:gz") as archive:
        files = [member for member in archive.getmembers() if member.isfile()]
        if len({member.name for member in files}) != len(files):
            raise ArchiveIdentityError("sdist contains duplicate file paths")
        members = {member.name: member for member in files}

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
        if identity.get("semantic_identity") != expected_identity:
            raise ArchiveIdentityError("sdist identity differs from the source identity")
        if manifest.get("semantic_identity") != expected_identity:
            raise ArchiveIdentityError("sdist manifest differs from the source identity")
        if package_manifest != manifest:
            raise ArchiveIdentityError("sdist package and build manifests differ")
        if identity.get("manifest") != "repopact/_semantic-input-manifest.json":
            raise ArchiveIdentityError("sdist identity points to an unexpected manifest path")

        inputs = manifest.get("inputs")
        if not isinstance(inputs, list):
            raise ArchiveIdentityError("sdist semantic manifest omitted its input list")
        try:
            if semantic_identity(inputs) != expected_identity:
                raise ArchiveIdentityError("sdist manifest inputs do not produce its semantic identity")
        except (KeyError, TypeError, ManifestError) as exc:
            raise ArchiveIdentityError(f"sdist manifest inputs are malformed: {exc}") from exc

        projection_config = policy.get("pyproject_projection")
        if not isinstance(projection_config, dict):
            raise ArchiveIdentityError("sdist policy omitted pyproject projection configuration")
        with tempfile.TemporaryDirectory(prefix="repopact-archive-identity-") as temporary:
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
            digest = hashlib.sha256(content).hexdigest()
            if digest != entry.get("sha256"):
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
    return {"path": path.name, "semantic_identity": expected_identity, "inputs_verified": len(inputs)}


def _probe_packaged_engine(
    member_name: str,
    executable: bytes,
) -> dict[str, Any]:
    """Run only the non-mutating protocol handshake from an isolated temp dir."""
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
            raise ArchiveIdentityError(f"packaged engine handshake could not complete: {exc}") from exc
        if result.returncode != 0:
            detail = result.stderr.strip()[-1000:]
            raise ArchiveIdentityError(
                "packaged engine handshake exited unsuccessfully"
                + (f": {detail}" if detail else "")
            )
        lines = [line for line in result.stdout.splitlines() if line.strip()]
        if len(lines) != 1:
            raise ArchiveIdentityError("packaged engine handshake did not return exactly one JSON line")
        try:
            response = json.loads(lines[0])
        except json.JSONDecodeError as exc:
            raise ArchiveIdentityError(f"packaged engine returned invalid handshake JSON: {exc.msg}") from exc
        if not isinstance(response, dict):
            raise ArchiveIdentityError("packaged engine handshake response must be a JSON object")
        return response


def _validate_packaged_handshake(
    response: dict[str, Any], *, expected_identity: str, expected_product: str
) -> None:
    if response.get("ok") is not True:
        raise ArchiveIdentityError("packaged engine handshake was not successful")
    if response.get("protocol") != "repopact-engine" or response.get("protocol_version") != 2:
        raise ArchiveIdentityError("packaged engine has an incompatible protocol identity")
    if response.get("engine_version") != expected_product:
        raise ArchiveIdentityError("packaged engine product identity differs from wheel metadata")
    if response.get("semantic_identity") != expected_identity:
        raise ArchiveIdentityError("packaged engine semantic identity differs from wheel identity")
    if response.get("request_id") != "wheel-identity-check":
        raise ArchiveIdentityError("packaged engine handshake did not match the verifier request")


def verify_wheel(
    path: Path,
    expected_identity: str,
    *,
    engine_probe: Any = _probe_packaged_engine,
) -> dict[str, Any]:
    with zipfile.ZipFile(path) as archive:
        listed = archive.namelist()
        if len(set(listed)) != len(listed):
            raise ArchiveIdentityError("wheel contains duplicate file paths")
        names = set(listed)
        required = (
            "repopact/_semantic_identity.json",
            "repopact/_semantic-input-manifest.json",
            "repopact/requirements-runtime.lock",
            "repopact/runtime-dependency-groups.json",
        )
        missing = [name for name in required if name not in names]
        if missing:
            raise ArchiveIdentityError("wheel omitted required identity/runtime files: " + ", ".join(missing))
        identity = _load_json(archive.read("repopact/_semantic_identity.json"), "wheel identity")
        manifest = _load_json(
            archive.read("repopact/_semantic-input-manifest.json"), "wheel semantic manifest"
        )
        if identity.get("semantic_identity") != expected_identity:
            raise ArchiveIdentityError("wheel identity differs from the source identity")
        if manifest.get("semantic_identity") != expected_identity:
            raise ArchiveIdentityError("wheel manifest differs from the source identity")
        if identity.get("manifest") != "repopact/_semantic-input-manifest.json":
            raise ArchiveIdentityError("wheel identity points to an unexpected manifest path")
        inputs = manifest.get("inputs")
        if not isinstance(inputs, list):
            raise ArchiveIdentityError("wheel semantic manifest omitted its input list")
        try:
            if semantic_identity(inputs) != expected_identity:
                raise ArchiveIdentityError("wheel manifest inputs do not produce its semantic identity")
        except (KeyError, TypeError, ManifestError) as exc:
            raise ArchiveIdentityError(f"wheel manifest inputs are malformed: {exc}") from exc

        package_inputs: dict[str, str] = {}
        for entry in inputs:
            if not isinstance(entry, dict) or not isinstance(entry.get("path"), str):
                raise ArchiveIdentityError("wheel semantic manifest contains a malformed input entry")
            relative = _safe_manifest_path(entry["path"])
            digest = entry.get("sha256")
            if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-f]{64}", digest):
                raise ArchiveIdentityError(f"wheel semantic input has an invalid digest: {relative}")
            if relative.startswith("repopact/"):
                if relative in package_inputs:
                    raise ArchiveIdentityError(f"wheel semantic manifest repeats input {relative}")
                package_inputs[relative] = digest
        for relative, expected_digest in sorted(package_inputs.items()):
            if relative not in names:
                raise ArchiveIdentityError(f"wheel omitted semantic package input {relative}")
            actual_digest = hashlib.sha256(archive.read(relative)).hexdigest()
            if actual_digest != expected_digest:
                raise ArchiveIdentityError(f"wheel semantic input digest mismatch: {relative}")

        metadata_paths = sorted(
            name for name in names if name.endswith(".dist-info/METADATA")
        )
        if len(metadata_paths) != 1:
            raise ArchiveIdentityError("wheel must contain exactly one dist-info/METADATA file")
        metadata = email.message_from_bytes(archive.read(metadata_paths[0]))
        product_version = metadata.get("Version")
        if not product_version:
            raise ArchiveIdentityError("wheel metadata omitted its product version")
        engine_paths = sorted(
            name
            for name in names
            if len(PurePosixPath(name).parts) == 3
            and PurePosixPath(name).parts[0].endswith(".data")
            and PurePosixPath(name).parts[1] == "scripts"
            and PurePosixPath(name).name in {"repopact-engine", "repopact-engine.exe"}
        )
        if len(engine_paths) != 1:
            raise ArchiveIdentityError(
                "wheel must contain exactly one Maturin scripts/repopact-engine executable"
            )
        packaged_engine = archive.read(engine_paths[0])
        engine_response = engine_probe(engine_paths[0], packaged_engine)
        if not isinstance(engine_response, dict):
            raise ArchiveIdentityError("packaged engine probe returned a malformed response")
        _validate_packaged_handshake(
            engine_response,
            expected_identity=expected_identity,
            expected_product=product_version,
        )
    return {
        "path": path.name,
        "semantic_identity": expected_identity,
        "semantic_package_files_verified": len(package_inputs),
        "engine_member": engine_paths[0],
        "engine_sha256": hashlib.sha256(packaged_engine).hexdigest(),
        "engine_build_info": engine_response.get("build_info"),
        "engine_identity_verified": True,
    }


def verify_archives(root: Path, sdist: Path, wheel: Path) -> dict[str, Any]:
    expected = source_identity(root)
    return {
        "semantic_identity": expected,
        "sdist": verify_sdist(sdist, expected),
        "wheel": verify_wheel(wheel, expected),
        "binary_authenticity": "not established by semantic identity agreement",
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--sdist", type=Path, required=True)
    parser.add_argument("--wheel", type=Path, required=True)
    args = parser.parse_args(argv)
    try:
        report = verify_archives(args.root, args.sdist, args.wheel)
    except (OSError, ValueError, ArchiveIdentityError, tarfile.TarError, zipfile.BadZipFile) as exc:
        print(f"package semantic identity verification failed: {exc}", file=sys.stderr)
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
