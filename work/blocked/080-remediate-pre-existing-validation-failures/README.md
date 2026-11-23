# WI080 — Remediate Pre-Existing Validation Failures and Restore Green Local Baseline

**Current status:** blocked on the unresolved intermittent Rust mutation failure; the exact-head local CI profile passed. **Lead:** governance-owner; tooling/evidence/docs/work affected. **Blocking implementation owner:** tooling-owner. **Human gate:** independent operator acceptance remains open.

## Decision and source evidence

During WI079 validation on Windows, Codex demonstrated that three failures reproduced outside WI079 or remained intermittent. The operator clarified that **classifying a failure as pre-existing is not its disposition**: the project must fix it, not leave it as a permanent reason that the canonical baseline stays red. Keep WI079's scope and its true results intact; this work item owns baseline remediation.

The evidence and exact starting candidate are in [WI079 PR #18](https://github.com/ForgeWireLabs/repopact-core/pull/18), including `20261122-wi079-baseline-isolation`, `20261123-wi079-exact-head-isolation`, and the failed local `verify ci` runs. Source main was `0ac343a2bf25f56a47e8f42798eca938844199fa` at investigation.

| Problem | Established observation | Remedy required |
| --- | --- | --- |
| Research freshness | `research/metadata.json` expires on 2026-11-21 (`verified_on=2026-10-22`). The 14 registered main-branch research documents have not been reverified since. WI079 registers a fifteenth candidate protocol. | Re-review the actual registered claims and supporting evidence; correct stale claims, produce dated evidence, and only then advance dates per policy 002. No expiry suppression. |
| WI076 provenance ID | `repopact/run_conformance.py::_write_provenance` uses a second `datetime.now` for an auto filename while the record ID uses a separate earlier `recorded_at`. | Derive a stable default filename and evidence ID from the same validated timestamp, preserve explicit `--provenance-out` and existing artifact history; add regressions. |
| Rust mutation test | `consumed_work_record_drift_is_stale` failed in two full CI profiles but passed filtered/package/full-workspace controlled reruns on main and WI079. | Identify the intermittent mechanism: filesystem fixture collisions, resource concurrency, inherited Git context, timing, or genuine production semantics. Repair root cause and prove repeatable reruns. No test skip or weakened assertion. |

## Implementation lanes

1. **Research integrity:** execute document-by-document semantic review using the canonical current research registry and evidence. Maintain a matrix of verified claims, sources, commands, revisions, changes, unresolved external dependencies, and review decision. No blanket date bump. This lane owns research remediation evidence, not an artificial pass.
2. **Conformance provenance:** make the default filename and embedded ID derive from one `recorded_at` UTC timestamp. Keep requested output paths supported (their basenames may intentionally differ); reject malformed time rather than silently generating ambiguous IDs. Add standalone and runner-level tests. No historical artifact rewrite.
3. **Rust intermittency:** run repeated clean, sanitized and controlled-inherited-env tests with unique isolated build and fixture paths. Capture evidence of actual failures; use minimal changes. If reproducibility is low, maintain a bounded diagnostic work package with explicit cause hypotheses rather than declaring success from reruns.
4. **Canonical local CI:** on an exact-head, clean worktree and matching engine, run the full configured `repopact verify ci`, canonical `validate`, Rust workspace, Python suite, conformance, and source freshness. Preserve failed runs and regenerate derived outputs with canonical generators.
5. **WI079 re-check:** after baseline repairs, run an exact-head integration check of WI079 against the resolved baseline. Do not merge unrelated maintenance into its branch silently.

## No false-success policy

A failure marked “pre-existing,” “flaky,” “environmental,” or “unrelated” is **not resolved**. It needs an owned remediation record and evidence-based closure. Existing failures are not automatic grounds to reject an unrelated, otherwise valid review indefinitely, but the *project baseline* cannot be labeled healthy until the failures are actually repaired or explicitly bounded by an operator risk decision. Avoid permanent skips, softened tests, hidden excludes, manipulated review clocks, and green dashboards generated from invalid records.

## CI budget restriction

CI runs **locally through 2026-12-01 inclusive**; do not enable `REPOPACT_GITHUB_CI` or `REPOPACT_GITHUB_CD`. On or after 2026-12-02, hosted CI can be reconsidered only by a new explicit operator decision. Existing hosted actions are opt-in, not a required execution lane.

## Completion

All ten `work-item.json` criteria require concrete, linked evidence; no claim of closure from the draft plan or preliminary fixed source. Preserve RepoPact source records, generated semantic identity, dashboard, research review dates and prior failure logs without fabricated timestamps. Operator and frozen-surface policy remain binding.


## Closeout checkpoint — 2026-11-23

The authorization-time observations and plan above are historical. This checkpoint records the current outcome and supersedes the earlier “unvalidated” status.

| Lane | Outcome | Evidence |
| --- | --- | --- |
| Research freshness | Reviewed all 14 registered documents and their cited evidence. Corrected F-015's stale status and dated current-state text; left unsupported or external claims bounded. Only after that review, set `verified_on=2026-11-23` and `review_by=2026-12-23`, the maximum 30-day window. The research validator passed. A PyPI description still points to the archived source URL; that external package metadata remains follow-up, not a reason to claim it was changed. | `20261123-080-research-freshness-review` |
| WI076 evidence provenance | The six regression tests pass. Real conformance passed 43/43 canonical cases and 8/8 WI050 vectors with a release engine whose embedded source revision is `7f007d75fb393f6d68f22b96ecaabd78bccee9ce`; source and engine revisions match. The generated filename stem equals its embedded record ID. Existing evidence remained create-only and unchanged. | `20261123-080-wi076-provenance-validation`; `20261123T181925821732Z-wi076-ac6-engine-conformance`; `20261123-080-wi076-overwrite-denial.log` |
| Rust mutation intermittency | The original failed run only records the missing `mutation.plan-stale` diagnostic; it omits the baseline plan state, fixture root, and `MutationResult`. The bounded campaign ran the focused test 80 times across sanitized/inherited Git environments and serial/parallel modes, plus three full workspace profiles (397 passed in each); no failure reproduced. The exact local CI profile also passed. Diagnostic-only assertions now prove the baseline plan is applicable and print the fixture root and full mutation result if it fails again. No production Rust behavior was changed, and the original root cause remains unresolved. | `20261123-080-rust-drift-diagnostic-campaign`; `20261123-wi079-exact-head-isolation`; `20261123-182000-636282-verify-ci` |
| WI079 fifteenth research document | Reviewed separately from the 14-document freshness update. The prospective protocol is explicitly planned/not run; it makes no result or reviewer-equivalence claim. Its pilot is low-power/descriptive, and its task hashes, model/toolchain pins, adjudication rubric, and telemetry must be frozen in a benchmark manifest before any separately authorized execution. No run, spend, or production integration was initiated. | `20261123-080-wi079-protocol-review` |
| PR reconciliation | Updated both PR descriptions and verified both remain open drafts. PR #18 was not rebased or otherwise changed; its accepted-baseline rebase prerequisite is recorded. The WI079 fifteenth protocol review is pinned to PR head `0562293d1f4a789055f6035204a60550ff29fd9e`. GitHub reported no workflow runs for the WI080 push. | `20261123-080-pr-reconciliation` |
| Exact local CI | From a clean-at-start Windows checkout at commit `7f007d75fb393f6d68f22b96ecaabd78bccee9ce`, tree `c08acfb4bb6b368c4de134aed501a1f2cb03c1f0`, the canonical `ci` profile returned 0 after 1663.734 seconds. All five required steps passed; the optional frozen-surface report passed. The Python step discovered 454 test cases and returned 0. The CI run generated the conformance report, verify-ci manifest, and dashboard update; those generated changes explain its after-run dirty flag. All inherited `GIT_*` variables were removed for the run; hosted CI/CD variables were unset. | `20261123-182000-636282-verify-ci` |

The current disposition is: research freshness corrected; WI076 provenance writer fixed and validated; Rust intermittency **blocked/unresolved**. A green exact profile is recorded truthfully, but passing reruns do not prove the original intermittent defect fixed. AC-6 and AC-10 remain pending. `tooling-owner` owns the next diagnostic: if the failure recurs, preserve the exact Cargo command and process environment, then capture plan applicability/diagnostics, repository identity, fixture path, matching read-fact paths and before/after hashes, full `MutationResult`, thread/process IDs, and cleanup state from the same failed invocation. The human operator retains independent acceptance authority; no risk waiver was requested or inferred.

CI remains local-only through 2026-12-01 inclusive. PR #19 and PR #18 remain drafts; no hosted verification, merge, release, or publication is authorized.
