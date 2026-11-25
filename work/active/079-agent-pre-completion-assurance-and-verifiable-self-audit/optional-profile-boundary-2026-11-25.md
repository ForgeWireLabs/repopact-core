# WI079 — Optional local verification boundary

**Status: staged tests, NOT RUN. No binding completion policy.**

This bounded addendum is stacked on independently inspected draft PR #25, clean source `fe853f02ddd4f99538f912b68977049fb8532783` and pushed head `acaebd00ee1b95dc8d287dd8356e4e5693096c2a`. PR #25's full local CI proves the optional helper's structural behavior, but its repository `ci` profile does **not** declare a required `self-audit check` step. Therefore the green result is not proof of an effective pre-completion checkpoint.

## Test-only integration boundary

`tests/test_self_audit_verification_boundary.py` uses a disposable local Git repository and a valid, host-scoped `governance/verification.json` with one explicitly required argv step:

```json
{
  "id": "self-report-freshness",
  "argv": [
    "{python}", "-m", "repopact.self_audit", "check",
    "--root", "{root}", "--report", "<external-disposable-file>"
  ],
  "required": true,
  "timeout_seconds": 120
}
```

The test executes `verify_cli.main(... --json)` as the actual local RepoPact runner; the runner invokes a child Python process with the source package on `PYTHONPATH`. It restores the caller's environment and avoids inherited `GIT_*` overrides inside the disposable fixture. It does not patch the real completion engine or add a default profile step.

Expected outcomes to **verify, not claim** until execution:

| Case | Self-audit check | Required local profile | Expected scope |
| --- | --- | --- | --- |
| Report absent | Exit 2 / incomplete | `fail`, exit 1 | Actual required check was invoked and denied |
| Report incomplete | Exit 2 / incomplete | `fail`, exit 1 | Self-report has not attested completion |
| Report stale after untracked edit | Exit 2 / stale | `fail`, exit 1 | Candidate freshness rejects changed paths |
| Extra `independent_approval` claim | Exit 2 / incomplete | `fail`, exit 1 | No approval laundering via report fields |
| Structurally current synthetic self-report | Exit 0 | `pass`, exit 0 | Local structural match **only**, `assurance=instruction-only` |
| Direct Git commit of `completed` work-item marker without invoking profile | Git succeeds; subsequent check exit 2 | `fail`, exit 1 | **Negative coverage:** plain Git did not intercept/authorize completion |

A failed required command has `verification.status=fail` while `coverage.satisfied=true` in host mode, because **coverage** measures whether an applicable required step executed, not whether it passed. Treat `status`, executed-step outcome and `coverage` as distinct dimensions. A Git commit of a `status=completed` marker is not proof RepoPact's canonical validator accepted that item; this test deliberately documents the absent invocation boundary.

## Remaining acceptance barriers

- **AC-8:** execute the tests on a clean exact-head source, confirm positive and negative effects using the actual runner, and preserve source/review provenance as evidence. This can prove only **opt-in local profile effectiveness**. It cannot establish a protected GitHub merge requirement, canonical work-item transition interception, or H14 coverage/effectiveness where no actual checkpoint is installed. Never reword a demonstrated *absence of interposition* as a secure gate.
- **AC-9:** prospective A/B/C comparison remains **NOT RUN**. No budget/model execution is authorized; all escaped-defect, false-stop, churn and quality claims remain unmeasured.
- **Other WI079 criteria:** remain pending until separately reviewed evidence supports them. Do not mark full WI079 completed from these six fixture tests.
- **WI080:** blocked on historical Rust failure AC-6/AC-10; these tests do not provide causal attribution.
- **CI/release:** hosted CI/CD remains disabled through 2026-12-01 inclusive; no merge, publication, operator-approved completion or release from this candidate.

**Source and evidence:** historical WI079 draft PR #18 and verified prototype PR #25 remain unchanged. This addendum and tests are a separate draft stacked on PR #25, requiring local exact-head Python/verification checks and final evidence before integration.
