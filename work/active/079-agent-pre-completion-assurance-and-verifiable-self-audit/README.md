# 079 — Agent Pre-Completion Assurance and Verifiable Self-Audit Evidence

> **Status:** active (operator-authorized engineering as of 2026-11-22); not completed or release-cleared.
> **Lead:** governance-owner. **Affected:** governance, tooling, evidence, docs, work.
> **Depends on:** none. **Created:** 2026-11-22.

## Intent and observed gap

A coordinated 2026-11-22 set of draft PRs proposes a bounded adversarial self-audit instruction to the root `AGENTS.md` of 20 active repositories. That immediately improves guidance, but **an agent's `Self-Audit: PASS` statement does not establish that an audit happened, that its candidate is current, or that its review is correct.** Instructions do not provide a process or merge boundary.

RepoPact already has work-item acceptance criteria, evidence runs, local verification profiles, canonical Rust semantics, audits, and optional admission. This item investigates and, following an explicit design decision, implements the *smallest optional generic mechanism* that allows adopter policies to require current, attributable pre-completion review evidence for supported operations. Do not build a second validator, force every adopter to install an agent framework, or confuse review evidence with approval.

This is **not** a rewrite of [WI044](../../proposed/044-typed-capability-completion-and-replacement-cutover-evidence-contracts/README.md) (typed product-completion and cutover claims) or H14 (admission checkpoint coverage, invocation, effectiveness). These are adjacent models to reconcile, not silently absorb. WI078 is separately reserved for agent workspace/build cleanup on `codex/wi078-govern-agent-workspace-cleanup`.

## Review contract to evaluate

1. **Candidate boundary:** resolved comparison base, repository and worktree identity, HEAD/tree when available, staged/unstaged state, relevant untracked files, renames/deletions and excluded/unsupported surfaces. A review of a dirty tree cannot masquerade as one of an immutable commit. A post-review mutation invalidates or appropriately narrows prior review/test evidence.
2. **Adversarial inspection:** concrete defects in correctness, error handling, concurrency, permissions/security, contracts, persistence, performance/resource leaks, cleanup, and scope. Findings identify location, plausible failure, and evidence. Do not manufacture stylistic defects or change unrelated work.
3. **Bounded remediation:** in-scope fix, focused tests, re-review of changed surfaces; default maximum two repair cycles, with explicit blocked/incomplete handoff instead of infinite churn or relaxed tests.
4. **Truthful reporting:** separate *self-reported review*, *observed command execution*, *human/independent review*, *operator authorization*, and *enforcement boundary proof*. A green test and a review statement prove different things. `PASS` is not defect freedom, release clearance, or independent approval.
5. **Conditional closeout:** only opted-in repositories and supported host/workflow boundaries may treat missing or stale review evidence as a completion blocker. Bare `AGENTS.md` remains instruction-only; without demonstrated invocation and effective rejection, never claim fail-closed enforcement.

## Decision sequence

**Phase A — baseline and architecture.** Audit current RepoPact schema/CLI/verification/evidence behavior; review the root-contract draft rollout; evaluate at least three placements (template only, typed evidence + existing verify, optional completion policy). Record why a candidate does or does not warrant a new schema/semantic check. Compare against WI044 and H14. Make policy versioning and old-adopter compatibility explicit before implementation.

**Phase B — bounded pilot and integration.** The selected opt-in pilot uses a read-only Python/Git candidate helper and RepoPact's existing verification profiles without adding canonical semantics. A future **binding completion-policy rule**, if justified, requires an explicit design/authority decision and canonical Rust implementation with compatibility/conformance. Do not widen mutation authorization or require a model vendor.

**Phase C — adversarial tests and proof.** Run positive and negative cases: no-findings review; defect fixed and retested; stale candidate; unknown base; untracked/staged omission; altered file after inspection; fabricated or absent review; missing/failed required checks; out-of-scope or frozen change; attempted self-approval; and release-gate bypass. Explicitly demonstrate where a supported adapter actually intercepts completion and where same-principal bypass remains possible. Compare instruction-only and opt-in behavior.

**Phase D — research and adoption.** Pre-register matched tasks in three conditions: ordinary verification, bounded self-audit, and separate independent reviewer. Measure defect escape, spurious findings, task success, added diff/churn, time and token/compute cost. Preserve negative results. Document opt-in configuration, `AGENTS.md` nesting, candidate invalidation, verification state, and evidence closure. Do not automatically merge any adopter PR or change an existing release gate.

## Existing rollout reference (separate work)

- RepoPact Core root contract: [draft PR #17](https://github.com/ForgeWireLabs/repopact-core/pull/17).
- Fabric standalone: [draft PR #102](https://github.com/ForgeWireLabs/forgewire-fabric/pull/102).
- Bonsai Low-Bit Lab: [draft PR #28](https://github.com/JeremyShows/Bonsai_low_bit_lab/pull/28), whose root `AGENTS.md` is a frozen surface.
- The broader rollout covers 20 active projects; those PRs are instruction-level and do not satisfy this item's executable acceptance criteria.

## Non-goals and hard boundaries

- No mandatory global model-based review, second coding agent, or online/hosted service.
- No unbounded repair loop or edits beyond an authorized work item.
- No fabricated verification records, auto-approval, or self-issued operator receipts.
- No claim of security confinement against a fully privileged same-principal agent.
- No retroactive rewriting of completed work/evidence or required migration of legacy adopters.
- No immediate universal enforcement by merely adding `AGENTS.md` text.

## Current branch implementation and evidence (historical snapshots through 2026-11-25)

The operator authorized WI079 engineering after the initial proposal. The draft contains a **read-only, opt-in Python candidate fingerprint helper**, top-level `repopact self-audit` CLI dispatch, 31 focused test methods, an [adopter guide](../../../docs/guides/pre-completion-assurance.md), a [design reconciliation](design-reconciliation.md), and a [preregistered comparison protocol](../../../research/wi079-self-audit-benchmark-protocol.md).

- Exact GitHub source blobs were initially reproduced in an isolated Linux container; **24/24** executable tests passed there, with the full CLI-dispatch test omitted. Historical evidence remains in `20261122-wi079-isolated-self-audit-24` and earlier runs `20261122-wi079-isolated-self-audit-19`, `20261122-wi079-isolated-self-audit-22`, and `20261122-wi079-isolated-self-audit-23`.
- On the pinned Windows worktree, Python compilation passed and the focused suite passed twice at **31/31** (final run 37.198 s). The top-level capture/check smoke also passed, including a PowerShell UTF-8 BOM report. The smoke report was explicitly synthetic; its result was `self-report-current` at `instruction-only` assurance. Evidence: `20261122-wi079-windows-focused-regression` and `20261123-wi079-windows-final-focused` (partial and passed, concrete respectively).
- Local adversarial review added bounded external-report reads, UTF-8 BOM handling, JSON object/root and exact finding-field checks, and safe incomplete handling for path-resolution/symlink-loop errors. The focused regressions cover those cases. This does not independently establish review quality or host invocation before completion.
- Detached, same-host comparisons confirmed the research-freshness diagnostic and WI076 filename/ID mismatch on both clean `main` and the WI079 candidate; both are pre-existing baseline failures. The old WI076 artifact is preserved unchanged outside this worktree. The first controlled Rust test, package, and workspace comparisons passed on both snapshots, including a candidate replay with inherited Git variables; a later exact-head full CI profile recorded the same test failure once, while controlled rechecks passed again. Its cause remains unresolved. The original CI manifest remains failed and unchanged. Detailed commands, outcomes, and log paths are in [local validation](LOCAL-VALIDATION.md) and evidence `20261122-wi079-baseline-isolation`.
- The fresh-snapshot candidate replay of local CI removed all inherited `GIT_*` variables and finished **FAIL**: 2/5 required steps passed, 3/5 failed, overall exit 1 (`20261123-021446-176011-verify-ci`). Rust workspace, spec freshness, and the optional frozen report passed. Validation and 11 of 479 Python tests failed on the same expired freshness diagnostic; conformance vectors all passed, but dirty-tree provenance was incomplete and the pre-existing WI076 filename/ID mismatch recurred. The order-sensitive Git initialization cascade reproduced only with injected `GIT_CONFIG_*` and passed 55/55 after sanitization. No research date was advanced. Hosted CI/CD remains disabled through December 1 inclusive.
- A second sanitized profile ran on clean-at-start commit `81690fd3385487545cd78bac05dff976432bee27` and also finished **FAIL** (1/5 required steps passed). The 11 Python failures again came from expired research freshness. The profile used a release engine built at `0065c5b44aea09e2c09cb69cd0a3bcf509ef556b`, so its conformance provenance was correctly incomplete against `81690fd`; a clean exact-engine rerun then passed all 43/43 and 8/8 vectors with complete provenance while reproducing the WI076 filename/ID mismatch. Rust exited 101 in that profile, but the exact test, all 30 mutation tests, and full workspace passed on both main and candidate in controlled sanitized rechecks; the cause remains unresolved. Evidence: `20261123-143717-209916-verify-ci` and `20261123-wi079-exact-head-isolation`.
- On clean-at-start checkpoint `c9ba39ed345a6598b7f2111f019115a55aa8d164` (tree `785f161733347f3322ca9ef5b2849b0a838eb0d2`), Windows `py_compile`, 31/31 focused tests, a top-level UTF-8-BOM synthetic capture/check, semantic identity `--check`, and frozen-surface check passed with inherited `GIT_*` removed. The smoke remained instruction-only and did not attest to a real review. Canonical `validate` still failed only on the expired research-freshness condition. Evidence: `20261123-wi079-windows-exact-head-focused`.
- On clean PR26 source `78569dc029f557f54870abb918ecafd3a68ad635` (tree `33e042101d27587ae4b1acc47502ce11f804e4c8`), all six optional-profile boundary tests and the original 32 self-audit regressions passed. The actual local `ci` profile then passed all six declared steps, including 515 Python tests (4 skipped), 43/43 canonical conformance cases, 8/8 WI050 vectors, the locked Rust workspace, spec freshness, and the optional frozen report; conformance provenance identifies an engine built from the exact source revision. Evidence: `20261125-079-pr26-optional-boundary-focused`, `20261125-079-pr26-clean-exact-head-ci`, and `20261125T163939159831Z-wi076-ac6-engine-conformance`. AC-8 remains pending because this proves only the opt-in local profile, not canonical transition interception or a protected/H14 gate.
- The prototype does not prove independent review, operator approval, admission closure, release readiness, or an effective protected completion boundary. The matched-agent benchmark is **NOT RUN**. Windows focused behavior was exercised; macOS behavior remains **NOT RUN**.
- All ten acceptance criteria remain pending and WI079 remains active. The exact-head local CI profile is now represented as passed evidence, but the profile remains opt-in and does not create a completion boundary. Hosted CI/CD remains disabled through December 1 inclusive; no merge, tag, or release is claimed.

## Known process deviation and outstanding approval

The initial exploratory helper was committed before the complete option comparison/design reconciliation was written. The order is preserved here instead of being backdated or represented as a preregistered implementation decision. AC-2 therefore remains pending and requires explicit technical review of the selected minimal pilot before any binding Rust/schema contract is implemented. Existing source experiments do not authorize new protected-surface changes.

## Closeout requirements

The ten criteria in `work-item.json` remain pending. Before any canonical schema, binding completion-policy, or other protected semantic change, record architectural approval in the existing governance workflow. Completion requires criterion-linked concrete evidence, documented limits and negative cases, resolved documentation-impact state, canonical RepoPact validation, and generator-produced dashboard/spec updates (never hand-edited). A draft work item alone is not an executable assurance feature.
