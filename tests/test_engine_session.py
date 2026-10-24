from __future__ import annotations

import unittest

from repopact.engine_client import PROTOCOL_VERSION
from repopact.engine_session import AdmissionError, Phase, SessionAdmissionState


PROTOCOL = "repopact-engine"
PRODUCT_VERSION = "3.1.4.dev0"
SEMANTIC_ID = "repopact-semantic-v1:sha256:" + "a" * 64


def state() -> SessionAdmissionState:
    return SessionAdmissionState(
        protocol=PROTOCOL,
        protocol_version=PROTOCOL_VERSION,
        product_version=PRODUCT_VERSION,
        semantic_identity=SEMANTIC_ID,
    )


def handshake(**overrides):
    response = {
        "ok": True,
        "protocol": PROTOCOL,
        "protocol_version": PROTOCOL_VERSION,
        "engine_version": PRODUCT_VERSION,
        "semantic_identity": SEMANTIC_ID,
        "capabilities": {
            "operations": ["validate", "dashboard.write"],
            "semantic_surfaces": ["work-items", "documentation-mappings"],
        },
    }
    response.update(overrides)
    return response


def admitted_response(semantic_identity=SEMANTIC_ID):
    return {
        "ok": True,
        "result": {
            "status": "admitted",
            "semantic_identity": semantic_identity,
        },
    }


class EngineSessionAdmissionStateTests(unittest.TestCase):
    def test_operation_is_blocked_until_same_session_admission_ack(self):
        machine = state()
        with self.assertRaisesRegex(AdmissionError, "process_started"):
            machine.authorize_operation("dashboard.write")

        machine.accept_handshake(handshake())
        self.assertEqual(machine.phase, Phase.HANDSHAKEN)
        with self.assertRaisesRegex(AdmissionError, "handshaken"):
            machine.authorize_operation("dashboard.write")

        parameters = machine.admission_parameters()
        self.assertEqual(parameters, {"semantic_identity": SEMANTIC_ID})
        self.assertEqual(machine.phase, Phase.ADMISSION_PENDING)
        with self.assertRaisesRegex(AdmissionError, "admission_pending"):
            machine.authorize_operation("dashboard.write")

        machine.accept_admission(admitted_response())
        self.assertEqual(machine.phase, Phase.ADMITTED)
        machine.authorize_operation("dashboard.write")

    def test_missing_or_stale_handshake_identity_fails_closed(self):
        for response in (
            handshake(semantic_identity=None),
            handshake(semantic_identity="repopact-semantic-v1:sha256:" + "b" * 64),
        ):
            machine = state()
            with self.assertRaisesRegex(AdmissionError, "semantic identity"):
                machine.accept_handshake(response)
            self.assertEqual(machine.phase, Phase.FAILED)
            with self.assertRaises(AdmissionError):
                machine.authorize_operation("validate")

    def test_legacy_identity_omission_never_reaches_admission(self):
        machine = state()
        legacy = handshake()
        del legacy["semantic_identity"]
        with self.assertRaises(AdmissionError):
            machine.accept_handshake(legacy)
        self.assertEqual(machine.phase, Phase.FAILED)
        with self.assertRaises(AdmissionError):
            machine.admission_parameters()

    def test_wrong_admission_acknowledgement_blocks_dispatch(self):
        machine = state()
        machine.accept_handshake(handshake())
        machine.admission_parameters()
        with self.assertRaisesRegex(AdmissionError, "did not acknowledge"):
            machine.accept_admission(admitted_response("repopact-semantic-v1:sha256:" + "b" * 64))
        with self.assertRaises(AdmissionError):
            machine.authorize_operation("dashboard.write")

    def test_process_exit_invalidates_admission_for_all_later_operations(self):
        machine = state()
        machine.accept_handshake(handshake())
        machine.admission_parameters()
        machine.accept_admission(admitted_response())
        machine.process_exited()
        self.assertEqual(machine.phase, Phase.FAILED)
        with self.assertRaisesRegex(AdmissionError, "failed"):
            machine.authorize_operation("validate")

    def test_handshake_and_admission_are_not_application_operations(self):
        machine = state()
        machine.accept_handshake(handshake())
        machine.admission_parameters()
        machine.accept_admission(admitted_response())
        with self.assertRaisesRegex(AdmissionError, "control messages"):
            machine.authorize_operation("admit")


if __name__ == "__main__":
    unittest.main()
