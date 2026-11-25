# WI079 AC-7 — exact positive/negative test inventory

**Status:** reviewed against executable test definitions and validated on a clean PR #27 source checkpoint `94e65a032da546f6a88c0f0d9cc9d48fd5a398a4` / tree `5a37de5d2acbf3c58eabdaeef46607e0ac2c4b52`. The repaired-finding regression passed first (1/1); the self-audit module passed 33/33 and boundary module 6/6 in the sanitized virtual environment. Exact-source local CI passed all five required steps and the optional frozen step: Python 516 passed/4 skipped, Rust workspace 98 passed/0 failed, canonical conformance 43/43, WI050 8/8. AC-7 remains pending for the specific unproven cases listed below; green CI does not substitute for them.

| Contract class | Specific executable method(s) | Evidence boundary |
| --- | --- | --- |
| Clean committed candidate | `tests/test_self_audit.py::CandidateAuditTests.test_empty_working_tree_is_bound_to_head_and_base`; `test_committed_change_updates_head_and_diff` | Historical 32/32 evidence plus PR27 focused evidence `20261125-079-pr27-ac7-focused`; full CI pending |
| Dirty staged/unstaged/untracked/deleted paths | `test_covers_staged_unstaged_untracked_and_deleted`; `test_dirty_content_and_index_drift_invalidate_report`; `test_staged_index_drift_invalidates_prior_report` | Historical focused evidence plus PR27 33/33; full CI pending |
| Omitted or changed nonignored untracked files | `test_report_requires_exhaustive_path_declaration`; `test_untracked_addition_invalidates_prior_report`; `tests/test_self_audit_verification_boundary.py::OptionalSelfAuditVerificationBoundaryTests.test_stale_report_after_untracked_change_is_denied` | PR27 self-audit 33/33 and actual required-profile 6/6 |
| Git rename, deletion, linked worktree | `test_git_rename_exposes_both_source_and_destination`; `test_covers_staged_unstaged_untracked_and_deleted`; `test_linked_worktree_uses_distinct_identity` | PR27 self-audit 33/33; Git rename is represented as both changed paths, not a false rename-semantic claim |
| Missing, malformed, duplicated or incomplete self-evidence | `test_report_without_attestation_does_not_pass`; `test_report_rejects_duplicate_path_claims`; `test_malformed_review_coverage_fails_cleanly`; `test_cli_rejects_missing_and_malformed_reports_cleanly`; boundary `test_missing_report_is_actually_invoked_and_denied` and `test_incomplete_self_report_is_denied_by_actual_profile` | PR27 self-audit 33/33 and required-profile 6/6 |
| False independent approval / forged claim field | `test_self_report_cannot_assert_independent_approval`; `test_finding_cannot_assert_independent_approval`; boundary `test_independent_approval_assertion_cannot_be_laundered` | Rejected structural field assertion; **not cryptographic adversary forgery resistance**. Same-principal forged reports remain a known limitation |
| Blocking finding and truthful incomplete/failure | `test_unresolved_findings_fail_and_fixed_findings_need_scenario`; `test_cli_reports_blocking_finding_as_failure`; `test_cli_reports_current_self_claim_without_pretending_independence` | PR27 self-audit 33/33; structurally current self-report remains instruction-only |
| **Repaired defect and fresh re-review** | **NEW** `test_repaired_finding_needs_fresh_snapshot_and_remains_self_reported` | **PASS** first (1/1) and again within sanitized self-audit 33/33. Demonstrates original blocker rejection, old snapshot rejection after changed bytes, fresh `fixed` finding acceptance, and instruction-only assurance. It validates a *reported* repair, not independent defect correctness |
| Required check returns nonzero | `tests/test_verification.py::VerificationRunnerTests.test_required_nonzero_is_failure`; actual boundary `test_incomplete_self_report_is_denied_by_actual_profile` and missing/stale/forged-report cases | Full Python suite 516 passed/4 skipped; boundary 6/6. Runner fixture proves required nonzero means failure/exit 1; actual WI079 boundary cases prove failing reports are denied |
| Required executable/capability unavailable | `tests/test_verification.py::VerificationRunnerTests.test_required_missing_executable_is_incomplete`; `test_declared_capability_is_aggregated`; `test_ci_storage_preflight_marks_every_step_incomplete_without_running` | Full Python suite passed. **Coverage limit:** these are generic runner fixtures; no actual WI079 self-audit profile case makes that step's executable/capability unavailable |
| Out-of-scope governance paths | `tests/test_validate_repo.py::test_unknown_affected_scope_is_rejected`; `test_tracked_path_must_have_exactly_one_owner_scope`; `tests/test_admission.py::test_invalid_lifecycle_and_scope_denied` | Full Python suite passed. These checks are **separate from the self-audit helper** and must not be attributed to it |
| Frozen-surface mutation | `tests/test_validate_repo.py::test_check_frozen_detects_working_tree_change`; exact-base `repopact check-frozen --base origin/main` | Full Python suite and standalone/profile frozen checks passed with no frozen changes; the fixture exercises the canonical guard, **not** a built-in self-audit policy |
| Conformance and compatibility | `tests/test_conformance.py::ConformanceTests.test_manifest_is_structurally_valid`; `test_rule_coverage_is_bidirectional_and_every_fixture_is_declared`; `test_manifest_matches_reference_suite`; `test_valid_fixture_is_accepted`; `test_invalid_fixtures_are_rejected`; `test_runner_reports_unexpected_fixture_violations`. Canonical cases include `satisfied-without-evidence`, `schema-invalid-evidence`, `verification-default-profile`, and `disjoint-active-scopes`; exact-source `python -m repopact.run_conformance` canonical 43/43 and WI050 8/8 | Full suite and conformance passed; provenance source and engine revision both `94e65a0…`. **Coverage limit:** canonical fixtures validate RepoPact governance records and do not exercise WI079 self-audit report claims or false independent-approval fields |
| No required profile on Git/transition | `tests/test_self_audit_verification_boundary.py::test_git_completion_marker_without_profile_is_not_intercepted` | PR27 boundary 6/6: direct Git commit succeeds without the optional profile. Explicit **negative enforcement** finding; does not imply canonical RepoPact accepted a valid completed item |

**Exact source already validated:**

- PR #25: `fe853f02ddd4f99538f912b68977049fb8532783` / tree `19f980e4c52ead9801adab2a5a7ad2fcfe285b16`, 509 Python passed/4 skipped, 32 focused pass, five required local CI steps and optional frozen pass.
- PR #26: `78569dc029f557f54870abb918ecafd3a68ad635` / tree `33e042101d27587ae4b1acc47502ce11f804e4c8`, 515 Python tests (4 skipped), six real optional-profile tests and 32 original focused pass, five required local CI steps and optional frozen pass.
- PR #27 exact local CI: `94e65a032da546f6a88c0f0d9cc9d48fd5a398a4` / tree `5a37de5d2acbf3c58eabdaeef46607e0ac2c4b52`, evidence `20261125-079-pr27-exact-local-ci`; output transcript `evidence/runs/20261125-079-pr27-exact-local-ci.stdout.log`.

**AC-7 disposition:** keep **pending**. The 33 self-audit tests, six actual-profile boundary tests, generic unavailable-check fixtures, and canonical 43-case conformance suite all pass, but the required contract is not fully demonstrated: the WI079 profile has no unavailable-self-audit-executable case, and canonical conformance fixtures do not exercise WI079 self-audit report claims. Add scoped fixtures and retained evidence before marking AC-7 satisfied. Same-principal report forgery and the optional Git boundary remain separate limits and do not prove independent authority or protected completion enforcement.

**Preserved invocation diagnostic:** the first `cargo fmt --all -- --check` attempt ran from the repository root without a root `Cargo.toml`, so Cargo metadata exited 1 before rustfmt ran. The corrected `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` passed. This was a command-path error, not a formatting defect; both outcomes are recorded in `evidence/runs/20261125-079-pr27-rustfmt-preflight.log`.

AC-8 remains an independently pending protected-boundary issue. These tests prove opt-in local behavior and truthfully document the absence of unconditional Git/merge interposition; they do not establish H14 coverage. Research AC-9 is still **NOT RUN**. Hosted CI/CD is local-only through 2026-12-01 inclusive.

## PR #28 final fixture validation — 2026-11-25

**Exact tested source:** `de2aa0e09001dbcdc6fdaab63f51c2b612a5b8e3` / tree
`5ff5cc9ea28bfed7490cd8851e16534a16a7fd0c`, clean at the start of the local
profile. The matching Windows engine reported source revision
`de2aa0e09001dbcdc6fdaab63f51c2b612a5b8e3` and semantic identity
`repopact-semantic-v1:sha256:5298450478cb065378969da70c40851b457f16a95f0da89fd51afe3453db6f44`.

Focused evidence `20261125-079-pr28-ac7-focused` records 33/33 self-audit
tests, 7/7 actual-profile boundary tests, and 3/3 corpus tests, all exit 0.
The unavailable-executable boundary case passed by observing an unavailable
required step, no helper exit code, zero executed/passed required steps, one
unavailable required step, unsatisfied coverage, and profile exit 2.

Exact-source local CI evidence is `20261125-079-pr28-exact-local-ci`: 2,242.203
seconds, exit 0, all five required steps passed, and the optional frozen-surface
step passed. The complete Python suite ran 520 tests with 4 skipped. Locked Rust
workspace tests passed (98 tests). SPEC freshness passed; frozen-surface check
reported no changes. Canonical conformance recorded complete process-image
provenance and passed 43/43 Rust cases, 8/8 WI050 vectors, and 10/10 WI079
reference vectors. The conformance record is
`20261125T210423501522Z-wi076-ac6-engine-conformance`.

| WI079 reference vector | Actual result | Exit | Status |
| --- | --- | ---: | --- |
| `WI079-current-report` | `self-report-current`, instruction-only | 0 | PASS |
| `WI079-missing-report` | `incomplete` | 2 | PASS |
| `WI079-incomplete-report` | `incomplete` | 2 | PASS |
| `WI079-stale-report` | `incomplete` | 2 | PASS |
| `WI079-omitted-untracked` | `incomplete` | 2 | PASS |
| `WI079-root-approval` | `incomplete` | 2 | PASS |
| `WI079-review-approval` | `incomplete` | 2 | PASS |
| `WI079-finding-approval` | `incomplete` | 2 | PASS |
| `WI079-blocking-finding` | `fail` | 1 | PASS |
| `WI079-reported-fixed` | `self-report-current`, instruction-only | 0 | PASS |

The reference vectors are Python-helper behavior checks; they do not claim
independent review, adversarial forgery resistance, or protected completion
enforcement. The unavailable executable case validates the optional local
profile's incomplete semantics. Those limits remain relevant to AC-8 and do not
turn an instruction-only self-report into authority.

**AC-7 recommendation:** the previously missing unavailable-executable profile
case and WI079 conformance vectors now have passing exact-source evidence, and
the broader positive/negative test inventory passed. Recommend AC-7 for final
review as eligible for satisfaction; keep its work-item state **pending** until
that review. AC-2, AC-8, AC-9, and AC-10 remain pending; the comparative model
experiment remains **NOT RUN**. Hosted CI/CD was not enabled.


## Historical follow-up candidate — PR27 checkpoint

PR #27 merged at `a53f499ab4e147ae3248aa48a545d8a6deac37ac`
(tree `d6b9a18305b30f7595ec089a4b6659854fddcd2d`). Five criteria were
satisfied on main; at this PR27 checkpoint the AC-7 disposition remained
unchanged and the following new code was **staged, NOT RUN**:

- `tests/test_self_audit_verification_boundary.py::test_unavailable_self_audit_executable_is_incomplete`:
  configure the actual required WI079 step with an absent absolute interpreter
  path in an isolated repository. Generate a structurally current external report
  for the modified candidate; then invoke the real profile runner. Expected:
  step unavailable, no helper exit code, zero required steps executed/passed,
  one required unavailable, coverage unsatisfied, profile incomplete/exit 2.
  This isolates executable availability from stale/missing report rejection.
- `repopact/self_audit_conformance.py`: ten isolated executable reference
  vectors (current, missing, incomplete, stale, omitted untracked, root/review/
  finding approval assertions, blocker, reported-fixed). Expected outcomes
  are fixed independently of the implementation. Each uses a disposable Git
  repo and the actual helper CLI. Missing Git or malformed output fails the case.
- `repopact/run_conformance.py`: default conformance command runs WI079
  alongside, but separately from, the 43 canonical Rust governance fixtures
  and eight WI050 vectors. The provenance record includes
  `conformance.wi079_self_audit_corpus` with per-vector results and explicit
  Python-reference/instruction-only labels; failed vectors make the combined
  command fail. No new canonical Rust rule, frozen schema, or mandatory
  self-audit completion check is introduced. The custom comparator path does
  not claim to exercise this reference-helper corpus.
- `tests/test_self_audit_conformance.py`: three new unit/integration tests
  for the complete vector set, missing Git, and combined failure aggregation.
  Mocked aggregation tests do not attest an actual Rust run.

At that checkpoint, the planned follow-up was to regenerate canonical semantic
identity, review the documentation snapshot, build a matching-source engine,
then run the 33 self-audit methods, **seven** optional-boundary methods, three
new corpus tests, and normal full local CI. Retain the individual ten-vector
outcomes, exact source commit/tree, and conformance provenance. This follow-up
was executed on PR28 as recorded in the final fixture validation section above.
If and only if all actual cases pass and the matrix is independently reviewed,
AC-7 may be reconsidered for satisfaction. AC-2, AC-8, AC-9 and AC-10 remain
pending. No comparative model calls, protected policy, hosted CI/CD, or release
is authorized.

## Final closeout source corrections — staged, NOT RUN

The original AC-7 matrix and passing PR28 results remain historical evidence.
The subsequent source review reproduced a net-diff omission for cancelling
staged/unstaged edits and an unhashable finding-status exception. The staged
correction uses the union of base-to-HEAD, HEAD-to-index and index-to-worktree
paths; invalid non-string finding statuses produce the controlled FAIL/exit 1
contract. New executable methods:

- `test_net_reversal_does_not_hide_review_layers` (staged/unstaged,
  committed/staged and committed/unstaged net reversals; omitted path rejected;
  complete path declaration remains instruction-only).
- `test_cli_rejects_unhashable_finding_status_without_traceback`
  (list/dict/list-of-string values rejected as structured FAIL, no assurance).

Review/reproduction evidence: `20261125-079-operator-disposition-and-documentation-review`.
These methods are new and not yet run. Final closeout requires the complete
35-method self-audit suite, seven boundary methods, three corpus methods,
and actual local exact-source CI including all existing conformance vectors.
No protected enforcement is claimed. AC-2/AC-9 waivers and AC-8 scoped
acceptance are recorded separately in Decision 0079. AC-10 remains pending.
