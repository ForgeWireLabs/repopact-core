# WI079 — Optional local verification boundary

**Status: exercised locally on 2026-11-25; AC-8 remains pending. No binding completion policy.**

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

Observed outcomes on clean exact source `78569dc029f557f54870abb918ecafd3a68ad635` (tree `33e042101d27587ae4b1acc47502ce11f804e4c8`):

| Case | Self-audit check | Required local profile | Observed scope |
| --- | --- | --- | --- |
| Report absent | Exit 2 / incomplete | `fail`, exit 1; required step executed | The actual runner invoked and denied the missing-evidence check |
| Report incomplete | Exit 2 / incomplete | `fail`, exit 1; required step executed | Self-report did not attest completion |
| Report stale after untracked edit | Initially current, then exit 2 / stale | `fail`, exit 1; required step executed | Candidate freshness rejected changed paths |
| Extra `independent_approval` claim | Exit 2 / incomplete | `fail`, exit 1; required step executed | Report fields did not establish approval |
| Structurally current synthetic self-report | Exit 0 | `pass`, exit 0; required step executed and passed | Local structural match **only**, `assurance=instruction-only` |
| Direct Git commit of `completed` work-item marker without invoking profile | Git succeeds; subsequent check exit 2 | Subsequent required profile `fail`, exit 1 | **Negative coverage:** plain Git did not intercept or authorize completion |

A failed required command has `verification.status=fail` while `coverage.satisfied=true` in host mode, because **coverage** measures whether an applicable required step executed, not whether it passed. Treat `status`, executed-step outcome and `coverage` as distinct dimensions. A Git commit of a `status=completed` marker is not proof RepoPact's canonical validator accepted that item; this test deliberately documents the absent invocation boundary.

## Recorded execution (2026-11-25)

- The six boundary tests passed with exit 0. Their exact test names and output are preserved in `evidence/runs/20261125-079-pr26-optional-boundary-focused.log`; the original 32 self-audit regressions also passed with exit 0 and are preserved in `evidence/runs/20261125-079-pr26-self-audit-32.log`.
- The complete local `ci` profile passed with exit 0 from a clean start at the source commit/tree above. All five required steps passed; the optional frozen-surface report also passed. The Python suite ran 515 tests with 4 skips, canonical conformance passed 43/43 cases and 8/8 WI050 vectors with complete engine provenance, the locked Rust workspace passed, and spec freshness passed. Evidence: `20261125-079-pr26-clean-exact-head-ci` and `20261125T163939159831Z-wi076-ac6-engine-conformance`.
- Before the full profile, Windows `repopact validate`, semantic-input `--check`, generated SPEC, all four documentation-mapping snapshots, rustfmt, exact PR25-base frozen-surface check, and `git diff --check` exited 0. The focused test process inherited 23 named `GIT_*` variables (names only are recorded); the full CI profile removed inherited `GIT_*`, left `CARGO_TARGET_DIR` unset, and did not set either hosted CI/CD flag.
- The CI profile proves an actual **opt-in local profile** invocation and its required-step behavior. It does not intercept ordinary Git commits or demonstrate protected-host/H14 enforcement. AC-8 remains pending; these results are linked as scoped evidence, not as criterion satisfaction.

## Remaining acceptance barriers

- **AC-8:** the clean exact-head tests and local CI are now linked as evidence of **opt-in local profile effectiveness**. The complete criterion remains pending: these results do not establish a protected GitHub merge requirement, canonical work-item transition interception, or H14 coverage/effectiveness where no actual checkpoint is installed. Never reword a demonstrated *absence of interposition* as a secure gate.
- **AC-9:** prospective A/B/C comparison remains **NOT RUN**. No budget/model execution is authorized; all escaped-defect, false-stop, churn and quality claims remain unmeasured.
- **Other WI079 criteria:** remain pending until separately reviewed evidence supports them. Do not mark full WI079 completed from these six fixture tests.
- **WI080:** blocked on historical Rust failure AC-6/AC-10; these tests do not provide causal attribution.
- **CI/release:** hosted CI/CD remains disabled through 2026-12-01 inclusive; no merge, publication, operator-approved completion or release from this candidate.

**Source and evidence:** historical WI079 draft PR #18 and verified prototype PR #25 remain unchanged. This addendum and tests are a separate draft stacked on PR #25, requiring local exact-head Python/verification checks and final evidence before integration.
