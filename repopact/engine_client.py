"""Fail-closed client for RepoPact's versioned Rust engine protocol."""

from __future__ import annotations

import json
import hashlib
import os
import platform
import queue
import re
import subprocess
import sys
import sysconfig
import threading
import uuid
from pathlib import Path
from typing import Any

from .engine_session import AdmissionError, Phase, SessionAdmissionState
from .package_version import package_version


PROTOCOL = "repopact-engine"
PROTOCOL_VERSION = 2
ENGINE_OVERRIDE = "REPOPACT_ENGINE"
ENGINE_TIMEOUT_SECONDS = 30.0
SEMANTIC_IDENTITY_ENFORCEMENT_VERSION = "3.1.4.dev0"


class EngineError(RuntimeError):
    """Base class for compatibility-engine failures."""


class EngineUnavailableError(EngineError):
    """No trusted, same-environment engine executable was found."""


class EngineProtocolError(EngineError):
    """The engine could not satisfy the transport/version contract."""


class EngineSemanticError(EngineError):
    """The engine returned a structured transport error."""


class EngineOutcomeUnknownError(EngineProtocolError):
    """A request was sent but its execution result could not be trusted."""


class _EngineRequestNotSentError(EngineProtocolError):
    """The selected process was already gone before request bytes were written."""


def expected_product_version() -> str:
    """Return the installed/check-out package identity expected from the engine."""
    value = package_version(Path(__file__).resolve().parents[1])
    # The historical source-side helper emits ``3.0.2dev1`` while installed
    # PEP 440 metadata and Maturin emit the canonical
    # ``3.0.2.dev1`` spelling. Normalize only this representation detail.
    for suffix in ("alpha", "a", "beta", "b", "rc", "dev"):
        marker = suffix
        if marker in value and value.rsplit(marker, 1)[1].isdigit():
            prefix, number = value.rsplit(marker, 1)
            if prefix.endswith("."):
                return value
            return f"{prefix}.{marker}{number}"
    return value


def _version_order(value: str) -> tuple[int, int, int, int, int]:
    """Parse the release forms used here without a runtime packaging dependency."""
    match = re.fullmatch(
        r"(\d+)\.(\d+)\.(\d+)(?:\.(a|b|rc|dev)(\d+))?(?:\+[A-Za-z0-9.]+)?",
        value,
    )
    if not match:
        raise EngineProtocolError(f"unsupported RepoPact product version syntax: {value!r}")
    major, minor, patch, label, serial = match.groups()
    rank = {"dev": 0, "a": 1, "b": 2, "rc": 3, None: 4}[label]
    return int(major), int(minor), int(patch), rank, int(serial or 0)


def semantic_identity_required(product_version: str | None = None) -> bool:
    """Enable strict admission at the first development release after stable 3.1.3."""
    actual = product_version if product_version is not None else expected_product_version()
    return _version_order(actual) >= _version_order(SEMANTIC_IDENTITY_ENFORCEMENT_VERSION)


def expected_semantic_identity() -> str:
    """Read the generated source/package identity; never derive it at runtime."""
    path = Path(__file__).with_name("_semantic_identity.json")
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise EngineProtocolError(f"packaged semantic identity is unavailable: {exc}") from exc
    if not isinstance(document, dict):
        raise EngineProtocolError("packaged semantic identity must be a JSON object")
    identity = document.get("semantic_identity")
    if not isinstance(identity, str) or not re.fullmatch(
        r"repopact-semantic-v1:sha256:[0-9a-f]{64}", identity
    ):
        raise EngineProtocolError("packaged semantic identity is malformed")
    return identity


def _candidate_scripts_directories() -> list[Path]:
    candidates = [Path(sysconfig.get_path("scripts")), Path(sys.executable).resolve().parent]
    result: list[Path] = []
    for candidate in candidates:
        if candidate and candidate not in result:
            result.append(candidate)
    return result


def locate_engine(*, allow_checkout: bool = False) -> Path:
    """Locate the explicit override or engine installed beside this Python.

    Checkout target directories are considered only by callers that explicitly
    opt into development discovery (the conformance runner); normal CLI use
    never searches PATH or unrelated directories.
    """
    override = os.environ.get(ENGINE_OVERRIDE)
    if override:
        path = Path(override).expanduser().resolve()
        if not path.is_file():
            raise EngineUnavailableError(f"{ENGINE_OVERRIDE} does not name an engine executable: {path}")
        return path

    names = ("repopact-engine.exe", "repopact-engine")
    for directory in _candidate_scripts_directories():
        for name in names:
            path = directory / name
            if path.is_file():
                return path

    if allow_checkout:
        root = Path(__file__).resolve().parents[1]
        target_directories: list[Path] = []
        configured = os.environ.get("CARGO_TARGET_DIR")
        if configured:
            target_directories.append(Path(configured).expanduser())
        target_directories.extend([root / "target", root / "rust" / "target"])
        for target in target_directories:
            for profile in ("debug", "release"):
                for name in names:
                    path = target / profile / name
                    if path.is_file():
                        return path.resolve()

    raise EngineUnavailableError(
        "repopact-engine is not installed in this Python environment; "
        "install a RepoPact wheel or set REPOPACT_ENGINE explicitly for development"
    )


def canonical_command_template() -> str:
    """Return the published conformance command for the canonical engine."""
    path = locate_engine(allow_checkout=True)
    return (
        f'"{sys.executable}" -m repopact.engine_client validate '
        f'--engine "{path}" --root "{{repo}}"'
    )


def _file_snapshot(path: Path | None) -> dict[str, Any] | None:
    """Capture a stable file ID and SHA-256, or return None when unreadable."""
    if path is None:
        return None
    try:
        resolved = path.resolve(strict=True)
        before = resolved.stat()
        digest = hashlib.sha256()
        with resolved.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
        after = resolved.stat()
    except OSError:
        return None
    before_id = (before.st_dev, before.st_ino, before.st_size, before.st_mtime_ns)
    after_id = (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns)
    return {
        "path": str(resolved),
        "file_identity": {
            "device": after.st_dev,
            "file_index": after.st_ino,
            "size": after.st_size,
            "mtime_ns": after.st_mtime_ns,
        },
        "sha256": digest.hexdigest(),
        "stable_during_capture": before_id == after_id,
    }


def _process_image_path(pid: int) -> Path | None:
    """Return the child process image path where the host exposes it reliably."""
    try:
        if os.name == "nt":
            import ctypes
            from ctypes import wintypes

            kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
            kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
            kernel32.OpenProcess.restype = wintypes.HANDLE
            kernel32.QueryFullProcessImageNameW.argtypes = [
                wintypes.HANDLE, wintypes.DWORD, wintypes.LPWSTR,
                ctypes.POINTER(wintypes.DWORD),
            ]
            kernel32.QueryFullProcessImageNameW.restype = wintypes.BOOL
            kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
            kernel32.CloseHandle.restype = wintypes.BOOL
            handle = kernel32.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
            if not handle:
                return None
            try:
                buffer = ctypes.create_unicode_buffer(32768)
                length = wintypes.DWORD(len(buffer))
                if not kernel32.QueryFullProcessImageNameW(
                    handle, 0, buffer, ctypes.byref(length)
                ):
                    return None
                return Path(buffer.value).resolve(strict=True)
            finally:
                kernel32.CloseHandle(handle)
        if sys.platform.startswith("linux"):
            return Path(f"/proc/{pid}/exe").resolve(strict=True)
    except (OSError, ValueError):
        return None
    return None


def _same_path(left: str | None, right: str | None) -> bool:
    if not left or not right:
        return False
    return os.path.normcase(os.path.abspath(left)) == os.path.normcase(os.path.abspath(right))


class EngineClient:
    def __init__(self, *, engine: Path | None = None, timeout: float = ENGINE_TIMEOUT_SECONDS):
        self.engine = engine.resolve() if engine is not None else locate_engine()
        self.timeout = timeout
        self._strict = semantic_identity_required()
        self._expected_semantic_identity = expected_semantic_identity() if self._strict else None
        self._handshaken = False
        self._operations: frozenset[str] = frozenset()
        self._session: SessionAdmissionState | None = None
        self._process: subprocess.Popen[str] | None = None
        self._stdout_lines: queue.Queue[str | None] = queue.Queue()
        self._stderr_lines: list[str] = []
        self._io_lock = threading.RLock()
        self._closed = False
        self._launch_provenance: dict[str, Any] | None = None
        self._engine_metadata: dict[str, Any] = {}

    def __enter__(self) -> "EngineClient":
        return self

    def __exit__(self, exc_type: Any, exc: Any, traceback: Any) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:
            pass

    def close(self) -> None:
        """Close this client-owned process without relaunching or retrying work."""
        with self._io_lock:
            if self._closed:
                return
            self._closed = True
            if self._session is not None:
                self._session.close()
            process = self._process
            if process is None:
                return
            if process.stdin is not None:
                try:
                    process.stdin.close()
                except OSError:
                    pass
            try:
                process.wait(timeout=min(self.timeout, 2.0))
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()
            for stream in (process.stdout, process.stderr):
                if stream is not None:
                    try:
                        stream.close()
                    except OSError:
                        pass

    def provenance(self) -> dict[str, Any]:
        """Describe the actual client-owned process and the bytes at its image path.

        A matching pre/post file digest plus process-image path correlation is
        useful run provenance, but does not authenticate a binary against a
        trusted publisher or prevent a malicious in-place replacement race.
        """
        launch = self._launch_provenance
        reasons: list[str] = []
        image_path = launch.get("process_image_path") if launch else None
        after = _file_snapshot(Path(image_path)) if image_path else None
        before = launch.get("before_launch") if launch else None
        image = launch.get("image_at_launch") if launch else None
        process_path = launch.get("process_image_path") if launch else None
        configured_path = launch.get("configured_path") if launch else str(self.engine)
        if not launch:
            reasons.append("engine process was not started by this client")
        if not process_path:
            reasons.append("host did not expose the child process image path")
        if not image:
            reasons.append("process image file identity could not be captured at launch")
        if not before:
            reasons.append("configured executable pre-launch digest could not be captured")
        if not after:
            reasons.append("process image post-run digest could not be captured")
        if process_path and not _same_path(configured_path, process_path):
            reasons.append("process image path differs from the selected executable")
        if before and image and before.get("file_identity") != image.get("file_identity"):
            reasons.append("selected executable file identity changed before process-image capture")
        if image and after and image.get("file_identity") != after.get("file_identity"):
            reasons.append("process image file identity changed during the conformance run")
        digests = [item.get("sha256") for item in (before, image, after) if item]
        if len(digests) != 3 or len(set(digests)) != 1:
            reasons.append("pre-launch, process-image, and post-run executable digests differ or are incomplete")
        if any(not item.get("stable_during_capture") for item in (before, image, after) if item):
            reasons.append("file identity changed while a digest was being captured")
        engine = dict(self._engine_metadata)
        build_info = engine.get("build_info")
        if not isinstance(build_info, dict) or any(
            build_info.get(key) in (None, "", "unavailable")
            for key in ("source_revision", "rustc_version", "target", "profile")
        ):
            reasons.append("engine build metadata is incomplete")
        if not engine.get("semantic_identity"):
            reasons.append("engine semantic identity was not recorded")
        complete = not reasons
        return {
            "format_version": 1,
            "status": "complete" if complete else "incomplete",
            "incomplete_reasons": reasons,
            "executable": {
                "selected_path": configured_path,
                "process_image_path": process_path,
                "pid": launch.get("pid") if launch else None,
                "file_identity": image.get("file_identity") if image else None,
                "sha256": digests[0] if complete else None,
                "sha256_before_launch": before.get("sha256") if before else None,
                "sha256_at_process_image": image.get("sha256") if image else None,
                "sha256_after_run": after.get("sha256") if after else None,
            },
            "engine": engine,
            "python": {
                "implementation": platform.python_implementation(),
                "version": platform.python_version(),
                "executable": str(Path(sys.executable).resolve()),
            },
            "authenticity": "not established by semantic identity or this file digest",
        }

    def _spawn_command(self) -> list[str]:
        return [str(self.engine)]

    def check_compatibility(self, *required_operations: str) -> "EngineClient":
        """Verify the installed engine before a retained Python workflow writes."""
        if getattr(self, "_strict", False):
            with self._io_lock:
                self._ensure_admitted()
        else:
            self._handshake()
        missing = sorted(set(required_operations) - self._operations)
        if missing:
            raise EngineProtocolError(
                "Rust engine does not advertise required operation(s): " + ", ".join(missing)
            )
        return self

    def call(
        self,
        operation: str,
        *,
        root: Path | None = None,
        params: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        if getattr(self, "_strict", False):
            return self._call_admitted(operation, root=root, params=params)
        self._handshake()
        response = self._invoke(
            {
                "protocol": PROTOCOL,
                "protocol_version": PROTOCOL_VERSION,
                "request_id": uuid.uuid4().hex,
                "operation": operation,
                "root": str(root.resolve()) if root is not None else None,
                "params": params or {},
            }
        )
        if not response.get("ok", False):
            error = response.get("error") or {}
            raise EngineSemanticError(
                f"Rust engine rejected {operation}: {error.get('code', 'engine.error')}: "
                f"{error.get('message', 'unknown engine error')}"
            )
        return response

    def _handshake(self) -> None:
        if getattr(self, "_strict", False):
            self._ensure_admitted()
            return
        if self._handshaken:
            return
        request = {
            "protocol": PROTOCOL,
            "protocol_version": PROTOCOL_VERSION,
            "request_id": uuid.uuid4().hex,
            "operation": "handshake",
            "root": None,
            "params": {},
        }
        response = self._invoke(request)
        self._verify_response(response, "handshake", request["request_id"])
        if not response.get("ok", False):
            error = response.get("error") or {}
            raise EngineProtocolError(
                f"Rust engine handshake failed: {error.get('code', 'engine.error')}: "
                f"{error.get('message', 'unknown engine error')}"
            )
        capabilities = response.get("capabilities")
        if not isinstance(capabilities, dict) or not isinstance(capabilities.get("operations"), list):
            raise EngineProtocolError("Rust engine handshake omitted capabilities")
        operations = capabilities["operations"]
        if any(not isinstance(operation, str) for operation in operations):
            raise EngineProtocolError("Rust engine handshake returned malformed operation capabilities")
        self._operations = frozenset(operations)
        self._handshaken = True
        self._engine_metadata = {
            "protocol": response.get("protocol"),
            "protocol_version": response.get("protocol_version"),
            "product_version": response.get("engine_version"),
            "semantic_identity": response.get("semantic_identity"),
            "build_info": response.get("build_info"),
        }

    def _ensure_admitted(self) -> None:
        if self._closed:
            raise EngineProtocolError("engine client is closed")
        if self._session is not None:
            if self._process is None or self._process.poll() is not None:
                self._session.process_exited()
                raise EngineProtocolError(
                    "admitted Rust engine process exited; refusing to relaunch a different executable"
                )
            if self._session.phase is Phase.ADMITTED:
                return
            raise EngineProtocolError("Rust engine admission session is no longer usable")
        identity = self._expected_semantic_identity
        assert identity is not None
        from .runtime_dependencies import RuntimeDependencyError, verify_runtime_dependencies
        try:
            verify_runtime_dependencies()
        except RuntimeDependencyError as exc:
            raise EngineProtocolError(f"Python runtime dependency lock mismatch: {exc}") from exc
        self._session = SessionAdmissionState(
            protocol=PROTOCOL,
            protocol_version=PROTOCOL_VERSION,
            product_version=expected_product_version(),
            semantic_identity=identity,
        )
        self._start_session()
        handshake_request = self._request("handshake")
        try:
            handshake = self._session_exchange(handshake_request)
            self._session.accept_handshake(handshake)
            self._engine_metadata = {
                "protocol": handshake.get("protocol"),
                "protocol_version": handshake.get("protocol_version"),
                "product_version": handshake.get("engine_version"),
                "semantic_identity": handshake.get("semantic_identity"),
                "build_info": handshake.get("build_info"),
            }
            self._operations = self._session.operations
            admit_request = self._request(
                "admit", params=self._session.admission_parameters()
            )
            admitted = self._session_exchange(admit_request)
            self._session.accept_admission(admitted)
            self._handshaken = True
        except (AdmissionError, EngineError):
            self.close()
            raise

    def _request(
        self, operation: str, *, root: Path | None = None, params: dict[str, Any] | None = None
    ) -> dict[str, Any]:
        return {
            "protocol": PROTOCOL,
            "protocol_version": PROTOCOL_VERSION,
            "request_id": uuid.uuid4().hex,
            "operation": operation,
            "root": str(root.resolve()) if root is not None else None,
            "params": params or {},
        }

    def _start_session(self) -> None:
        creationflags = getattr(subprocess, "CREATE_NO_WINDOW", 0) if os.name == "nt" else 0
        before = _file_snapshot(self.engine)
        try:
            process = subprocess.Popen(
                self._spawn_command(),
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding="utf-8",
                errors="strict",
                bufsize=1,
                shell=False,
                creationflags=creationflags,
            )
        except FileNotFoundError as exc:
            raise EngineUnavailableError(f"unable to start Rust engine {self.engine}: {exc}") from exc
        except OSError as exc:
            raise EngineProtocolError(f"unable to start Rust engine {self.engine}: {exc}") from exc
        self._process = process
        image_path = _process_image_path(process.pid)
        image = _file_snapshot(image_path)
        self._launch_provenance = {
            "configured_path": str(self.engine),
            "pid": process.pid,
            "before_launch": before,
            "process_image_path": str(image_path) if image_path else None,
            "image_at_launch": image,
        }
        assert process.stdout is not None and process.stderr is not None
        threading.Thread(target=self._drain_stdout, args=(process.stdout,), daemon=True).start()
        threading.Thread(target=self._drain_stderr, args=(process.stderr,), daemon=True).start()

    def _drain_stdout(self, stream: Any) -> None:
        try:
            for line in stream:
                self._stdout_lines.put(line)
        finally:
            self._stdout_lines.put(None)

    def _drain_stderr(self, stream: Any) -> None:
        try:
            for line in stream:
                self._stderr_lines.append(line.rstrip("\r\n"))
                if len(self._stderr_lines) > 50:
                    del self._stderr_lines[:-50]
        except OSError:
            return

    def _session_exchange(self, request: dict[str, Any]) -> dict[str, Any]:
        process = self._process
        if process is None or process.poll() is not None:
            raise _EngineRequestNotSentError("Rust engine process exited before request dispatch")
        assert process.stdin is not None
        encoded = json.dumps(request, sort_keys=True, separators=(",", ":")) + "\n"
        try:
            process.stdin.write(encoded)
            process.stdin.flush()
            line = self._stdout_lines.get(timeout=self.timeout)
        except queue.Empty as exc:
            raise EngineProtocolError(
                f"Rust engine timed out after {self.timeout:g}s during {request['operation']}"
            ) from exc
        except (BrokenPipeError, OSError) as exc:
            raise EngineProtocolError(f"Rust engine transport failed: {exc}") from exc
        if line is None:
            detail = "\n".join(self._stderr_lines[-8:])
            raise EngineProtocolError(
                "Rust engine exited without a JSON response"
                + (f": {detail}" if detail else "")
            )
        try:
            response = json.loads(line)
        except json.JSONDecodeError as exc:
            raise EngineProtocolError(f"Rust engine returned malformed JSON: {exc.msg}") from exc
        if not isinstance(response, dict):
            raise EngineProtocolError("Rust engine response must be a JSON object")
        self._verify_response(
            response,
            request["operation"],
            request["request_id"],
            semantic_identity=self._expected_semantic_identity,
        )
        return response

    def _call_admitted(
        self, operation: str, *, root: Path | None, params: dict[str, Any] | None
    ) -> dict[str, Any]:
        with self._io_lock:
            self._ensure_admitted()
            assert self._session is not None
            try:
                self._session.authorize_operation(operation)
            except AdmissionError as exc:
                raise EngineProtocolError(str(exc)) from exc
            request = self._request(operation, root=root, params=params)
            try:
                response = self._session_exchange(request)
            except _EngineRequestNotSentError as exc:
                self._session.process_exited()
                raise EngineProtocolError(
                    "admitted Rust engine exited before operation dispatch; refusing to relaunch"
                ) from exc
            except EngineError as exc:
                self._session.process_exited()
                raise EngineOutcomeUnknownError(
                    f"Rust engine outcome is unknown after dispatching {operation}: {exc}"
                ) from exc
            if not response.get("ok", False):
                error = response.get("error") or {}
                raise EngineSemanticError(
                    f"Rust engine rejected {operation}: {error.get('code', 'engine.error')}: "
                    f"{error.get('message', 'unknown engine error')}"
                )
            return response

    def _invoke(self, request: dict[str, Any]) -> dict[str, Any]:
        encoded = json.dumps(request, sort_keys=True, separators=(",", ":")) + "\n"
        creationflags = getattr(subprocess, "CREATE_NO_WINDOW", 0) if os.name == "nt" else 0
        try:
            process = subprocess.Popen(
                [str(self.engine)],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True,
                encoding="utf-8",
                errors="strict",
                shell=False,
                creationflags=creationflags,
            )
            stdout, stderr = process.communicate(encoded, timeout=self.timeout)
        except FileNotFoundError as exc:
            raise EngineUnavailableError(f"unable to start Rust engine {self.engine}: {exc}") from exc
        except subprocess.TimeoutExpired as exc:
            process.kill()
            process.communicate()
            raise EngineProtocolError(
                f"Rust engine timed out after {self.timeout:g}s during {request['operation']}"
            ) from exc
        except OSError as exc:
            raise EngineProtocolError(f"unable to start Rust engine {self.engine}: {exc}") from exc

        response = self._decode_response(stdout, stderr, request["request_id"])
        self._verify_response(response, request["operation"], request["request_id"])
        if process.returncode != 0 and response.get("ok"):
            detail = stderr.strip() or "engine exited unsuccessfully"
            raise EngineProtocolError(f"Rust engine failed after a successful response: {detail}")
        return response

    def _decode_response(self, stdout: str, stderr: str, request_id: str) -> dict[str, Any]:
        text = stdout.strip()
        if not text:
            detail = stderr.strip()
            raise EngineProtocolError(
                f"Rust engine returned no JSON response for request {request_id}"
                + (f": {detail}" if detail else "")
            )
        try:
            decoder = json.JSONDecoder()
            response, end = decoder.raw_decode(text)
        except json.JSONDecodeError as exc:
            raise EngineProtocolError(f"Rust engine returned malformed JSON: {exc.msg}") from exc
        if text[end:].strip():
            raise EngineProtocolError("Rust engine returned more than one JSON response")
        if not isinstance(response, dict):
            raise EngineProtocolError("Rust engine response must be a JSON object")
        return response

    def _verify_response(
        self,
        response: dict[str, Any],
        operation: str,
        request_id: Any,
        *,
        semantic_identity: str | None = None,
    ) -> None:
        if response.get("protocol") != PROTOCOL:
            raise EngineProtocolError("Rust engine protocol name mismatch")
        if response.get("protocol_version") != PROTOCOL_VERSION:
            raise EngineProtocolError(
                f"Rust engine protocol major mismatch: {response.get('protocol_version')!r}"
            )
        if response.get("request_id") != request_id:
            raise EngineProtocolError(f"Rust engine request identity mismatch during {operation}")
        if response.get("engine_version") != expected_product_version():
            raise EngineProtocolError(
                "Rust engine product version mismatch: "
                f"{response.get('engine_version')!r} != {expected_product_version()!r}"
            )
        if semantic_identity is not None and response.get("semantic_identity") != semantic_identity:
            raise EngineProtocolError(
                f"Rust engine semantic identity mismatch during {operation}"
            )


def client() -> EngineClient:
    return EngineClient()


def validate_repository(root: Path, *, client: EngineClient | None = None) -> dict[str, Any]:
    """Return a structurally consistent canonical Rust validation result.

    This boundary is used by retained Python workflows when their safe execution
    depends on repository validity. It deliberately has no Python fallback.
    """
    engine = client or EngineClient()
    return _validated_result(engine.call("validate", root=root))


def validate_canonically(root: Path) -> tuple[bool, list[dict[str, Any]]]:
    result = validate_repository(root)
    return result["valid"], result["diagnostics"]


def write_dashboard_canonically(root: Path, *, client: EngineClient | None = None) -> None:
    """Write the derived dashboard through the canonical Rust projection."""
    engine = client or EngineClient()
    response = engine.call("dashboard.write", root=root)
    result = response.get("result")
    if not isinstance(result, dict) or result.get("path") != "audits/reports/dashboard.md":
        raise EngineProtocolError("Rust engine dashboard response omitted its canonical path")


def validated_mutation_result(response: dict[str, Any]) -> dict[str, Any]:
    """Reject malformed or internally inconsistent canonical mutation results."""
    if response.get("ok") is not True:
        raise EngineProtocolError("Rust engine mutation response is not successful")
    result = response.get("result")
    if not isinstance(result, dict) or not isinstance(result.get("success"), bool):
        raise EngineProtocolError("Rust engine mutation result omitted boolean success")
    paths = result.get("changed_paths")
    diagnostics = result.get("diagnostics")
    if not isinstance(paths, list) or any(not isinstance(path, str) for path in paths):
        raise EngineProtocolError("Rust engine mutation result returned malformed changed paths")
    if not isinstance(diagnostics, list):
        raise EngineProtocolError("Rust engine mutation result omitted diagnostics")
    top_level = response.get("diagnostics")
    if "diagnostics" not in response:
        if diagnostics:
            raise EngineProtocolError("Rust engine omitted non-empty top-level mutation diagnostics")
    elif not isinstance(top_level, list) or top_level != diagnostics:
        raise EngineProtocolError("Rust engine mutation diagnostics are inconsistent")
    if any(not isinstance(item, dict) or item.get("severity") not in {"error", "warning", "info"}
           for item in diagnostics):
        raise EngineProtocolError("Rust engine mutation returned malformed diagnostics")
    has_errors = any(item["severity"] == "error" for item in diagnostics)
    if result["success"] != (not has_errors):
        raise EngineProtocolError("Rust engine mutation success conflicts with its diagnostics")
    return result


def render_validation(response: dict[str, Any]) -> int:
    """Render the structured validation result using the historical CLI shape."""
    result = _validated_result(response)
    diagnostics = result["diagnostics"]
    for diagnostic in diagnostics:
        code = diagnostic.get("code", "engine.diagnostic")
        severity = str(diagnostic.get("severity", "error")).upper()
        location = diagnostic.get("path") or diagnostic.get("record") or "<repository>"
        print(f"{severity} [{code}] {location}: {diagnostic.get('message', '')}")
    valid = result["valid"]
    if valid:
        print("Repository governance validation passed.")
        return 0
    print(f"\nValidation failed with {result.get('error_count', len(diagnostics))} error(s).")
    return 1


def _validated_result(response: dict[str, Any]) -> dict[str, Any]:
    """Validate an already received validation response before interpreting it."""
    if response.get("ok") is not True:
        raise EngineProtocolError("Rust engine validation response is not successful")
    result = response.get("result")
    if not isinstance(result, dict) or not isinstance(result.get("valid"), bool):
        raise EngineProtocolError("Rust engine validation result omitted boolean validity")
    nested = result.get("diagnostics")
    if not isinstance(nested, list):
        raise EngineProtocolError("Rust engine validation result omitted diagnostics")
    diagnostics = response.get("diagnostics")
    if "diagnostics" not in response:
        if nested:
            raise EngineProtocolError("Rust engine omitted non-empty top-level diagnostics")
        diagnostics = nested
    elif not isinstance(diagnostics, list) or diagnostics != nested:
        raise EngineProtocolError("Rust engine validation diagnostics are inconsistent")
    if any(not isinstance(item, dict) or item.get("severity") not in {"error", "warning", "info"}
           for item in diagnostics):
        raise EngineProtocolError("Rust engine validation returned malformed diagnostics")
    errors = sum(item["severity"] == "error" for item in diagnostics)
    warnings = sum(item["severity"] == "warning" for item in diagnostics)
    error_count = result.get("error_count")
    warning_count = result.get("warning_count")
    if type(error_count) is not int or type(warning_count) is not int:
        raise EngineProtocolError("Rust engine validation result omitted diagnostic counts")
    if error_count != errors or warning_count != warnings:
        raise EngineProtocolError("Rust engine validation diagnostic counts are inconsistent")
    if result["valid"] != (error_count == 0):
        raise EngineProtocolError("Rust engine validity conflicts with its error count")
    return result


def main(argv: list[str] | None = None) -> int:
    """Small internal conformance adapter; it never implements validation."""
    import argparse

    parser = argparse.ArgumentParser(description="Run canonical RepoPact engine validation")
    parser.add_argument("validate", choices=["validate"])
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--engine", type=Path)
    args = parser.parse_args(argv)
    try:
        response = EngineClient(engine=args.engine).call("validate", root=args.root)
        return render_validation(response)
    except EngineError as exc:
        print(f"Rust engine compatibility error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
