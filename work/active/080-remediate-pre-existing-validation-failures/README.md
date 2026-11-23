# WI080 — Remediate Pre-Existing Validation Failures and Restore Green Local Baseline

**Status:** active; operator-authorized on 2026-11-23. **Lead:** governance-owner; tooling/evidence/docs/work affected. **Depends on:** none.

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
