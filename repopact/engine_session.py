"""Pure admission state machine for one RepoPact engine process session."""

from __future__ import annotations

from enum import Enum, auto
import re
from typing import Any


class AdmissionError(RuntimeError):
    """An engine session did not prove compatibility before dispatch."""


class Phase(Enum):
    PROCESS_STARTED = auto()
    HANDSHAKEN = auto()
    ADMISSION_PENDING = auto()
    ADMITTED = auto()
    FAILED = auto()
    CLOSED = auto()


class SessionAdmissionState:
    """Track the fail-closed handshake/admission boundary for one process.

    Instances are process-scoped. A caller must discard the state when its
    engine process exits and must never use it to authorize a later launch.
    """

    def __init__(
        self,
        *,
        protocol: str,
        protocol_version: int,
        product_version: str,
        semantic_identity: str,
    ) -> None:
        if not re.fullmatch(r"repopact-semantic-v1:sha256:[0-9a-f]{64}", semantic_identity):
            raise AdmissionError("expected semantic identity is malformed")
        self.protocol = protocol
        self.protocol_version = protocol_version
        self.product_version = product_version
        self.semantic_identity = semantic_identity
        self.phase = Phase.PROCESS_STARTED
        self.operations: frozenset[str] = frozenset()
        self.semantic_surfaces: frozenset[str] = frozenset()

    def accept_handshake(self, response: dict[str, Any]) -> None:
        self._require(Phase.PROCESS_STARTED, "handshake")
        try:
            if response.get("ok") is not True:
                raise AdmissionError("engine handshake was not successful")
            if response.get("protocol") != self.protocol:
                raise AdmissionError("engine protocol name mismatch")
            if response.get("protocol_version") != self.protocol_version:
                raise AdmissionError("engine protocol major mismatch")
            if response.get("engine_version") != self.product_version:
                raise AdmissionError("engine product version mismatch")
            if response.get("semantic_identity") != self.semantic_identity:
                raise AdmissionError("engine semantic identity missing or mismatched")
            capabilities = response.get("capabilities")
            if not isinstance(capabilities, dict):
                raise AdmissionError("engine handshake omitted capabilities")
            operations = capabilities.get("operations")
            surfaces = capabilities.get("semantic_surfaces")
            if not isinstance(operations, list) or any(
                not isinstance(operation, str) or not operation for operation in operations
            ):
                raise AdmissionError("engine handshake returned malformed operation capabilities")
            if not isinstance(surfaces, list) or any(
                not isinstance(surface, str) or not surface for surface in surfaces
            ):
                raise AdmissionError("engine handshake returned malformed semantic surfaces")
            self.operations = frozenset(operations)
            self.semantic_surfaces = frozenset(surfaces)
            self.phase = Phase.HANDSHAKEN
        except AdmissionError:
            self.phase = Phase.FAILED
            raise

    def admission_parameters(self) -> dict[str, str]:
        self._require(Phase.HANDSHAKEN, "admission request")
        self.phase = Phase.ADMISSION_PENDING
        return {"semantic_identity": self.semantic_identity}

    def accept_admission(self, response: dict[str, Any]) -> None:
        self._require(Phase.ADMISSION_PENDING, "admission acknowledgement")
        result = response.get("result")
        if (
            response.get("ok") is not True
            or not isinstance(result, dict)
            or result.get("status") != "admitted"
            or result.get("semantic_identity") != self.semantic_identity
        ):
            self.phase = Phase.FAILED
            raise AdmissionError("engine did not acknowledge this semantic identity")
        self.phase = Phase.ADMITTED

    def authorize_operation(self, operation: str) -> None:
        self._require(Phase.ADMITTED, "operation dispatch")
        if operation in {"handshake", "admit"}:
            raise AdmissionError("session control messages are not application operations")
        if operation not in self.operations:
            raise AdmissionError(f"engine does not advertise operation: {operation}")

    def process_exited(self) -> None:
        if self.phase is not Phase.CLOSED:
            self.phase = Phase.FAILED

    def close(self) -> None:
        self.phase = Phase.CLOSED

    def _require(self, phase: Phase, action: str) -> None:
        if self.phase is not phase:
            raise AdmissionError(
                f"cannot perform {action} while engine session is {self.phase.name.lower()}"
            )