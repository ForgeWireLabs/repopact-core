# 084 — Preserve external data during failed mutations

> **Status**: active
> **Owner scope**: tooling
> **Affected scopes**: tooling, governance, work, evidence
> **Depends on**: none

## Intent

Repair mutation rollback so an incomplete preimage cannot erase data and a failed apply does not restore paths that the transaction never owned. Follow Decision0040 and the authorized bounded repair in [PR #15 discussion](https://github.com/ForgeWireLabs/repopact-core/pull/15#issuecomment-7206149667), without editing that PR's completed WI076 records or evidence.

## Decisions

Decision0040 remains authoritative: stale plans fail closed; an ordinary transaction either restores its prestate or returns an explicit recoverable failure. No durable journal or process-death recovery is introduced.

## Scope

The tooling owner leads changes to mutation implementation and focused tests, plus the related documentation snapshot update. Governance, work, and evidence scopes are affected by the mapping, canonical record, and validation evidence.

## Acceptance criteria

- **AC-1** — Incomplete or ambiguous preimage capture fails closed before the first mutation write; regression tests prove external bytes remain unchanged.
- **AC-2** — Capture complete, consistent, repository-contained preimages under the mutation lock; distinguish absent paths from unreadable, failed enumeration, unsupported type, symlink/reparse, and inconsistent state, and reject before writes.
- **AC-3** — Rollback changes only transaction-owned after-state, preserves external and unowned bytes/directories, reports conflicts as recovery-required failures, and never reports success or false `rolled_back` when restoration is unproven.
- **AC-4** — Focused regression coverage verifies create/edit/transition success; capture failures; injected failures before and after writes/moves; external drift before operation zero and during apply; subtrees, unrelated content, and symlink/reparse outside sentinels.
- **AC-5** — Record focused tests, the required local validation profile and legacy comparator, dashboard reconciliation, canonical validation, and frozen-surface check while preserving plan identity/read-set/lock/stale checks/postvalidation/proposal semantics. No process-death, power-loss, or arbitrary uncooperative-writer recovery claim.
- **AC-6** — Reject multiply-linked regular files and unsupported reparse points at targeted roots, captured descendants, and rollback ownership rechecks; fail closed when link metadata is unavailable or unsupported. A same-volume hard-link outside sentinel remains byte-for-byte unchanged, with a host-supported junction/reparse negative case where available, and no new filesystem dependency is introduced.
- **AC-7** — Reconcile captured preimages and observed roots with original plan read facts and checked root states, reject unreadable or ambiguous observations as distinct from absence, and recheck applicable facts before the first write. Deterministic post-stale-check/pre-capture edits, direct file-read failure, captured-state mismatch, and exact rollback assertions prove fail-closed behavior while preserving external bytes.

## Coordinator review reopening

The [bounded coordinator review](https://github.com/ForgeWireLabs/repopact-core/pull/34#issuecomment-7224414238) reopened WI084 before runtime changes. It identified that hard-linked regular files can alias an outside sentinel even when bytes match, Windows reparse points extend beyond symlinks, and capture can accept state changed after the initial stale-plan check by seeding its expected state from the new capture. AC-1 through AC-5 retain their prior evidence links and are pending revalidation where the corrections overlap; AC-6 and AC-7 track the new containment and reconciliation regressions. Earlier successes and failures remain recorded.

## Reopened correction implementation

Captured regular files now require exactly one hard link. Unix uses `nlink`; Windows queries link information from an open file handle and fails closed if that query fails. Path-chain and captured-entry checks reject Windows reparse points as well as symlinks. The same containment checks run again before rollback restores transaction-owned paths.

The executor compares captured paths with the original plan read facts, uses checked observations that distinguish unreadable paths from absent ones, and rechecks all plan facts immediately before the first write. Rollback is reported as complete only when a fresh capture exactly matches the original preimage. No guarantee is made against an external writer after the final observation.

Focused evidence `20261127-084-wi084-corrections-focused` records 42 mutation tests passing, including direct and descendant hard-link sentinels, a hard link introduced before rollback, a valid edit between stale checking and capture, injected file-read failure, exact restoration, and an actual Windows junction. The final local CI profile and explicit legacy comparator for this reopening are recorded in the reopened correction closeout below.

## Prior closeout (before coordinator reopening)

Preflight: registered as WI084 against main `298828de6ae91f34dedd4f7cc6d98587f0959704`, tree `5fd33f98201f0ff313a199dc99ba74d0fb21ca8f`, before runtime edits. The dirty original checkout and PR #15 branch were not changed.

Scope is limited to complete checked preimage capture, fail-closed handling for unsafe or ambiguous paths, and rollback of transaction-owned state. Preserve proposal semantics, plan identity/read-set checks, mutation locking, stale checks, and independent postvalidation. No crash/power-loss or arbitrary concurrent-writer guarantees are claimed.

## Implementation and validation

The mutation executor captures complete checked preimages while holding the mutation lock, rejects incomplete or unsafe captures before writing, and verifies transaction-owned after-state before rollback. Rollback restores or removes only paths the transaction can prove it owns; changed or ambiguous state is preserved and reported as recovery required. Existing plan identity/read-set, locking, stale-plan, postvalidation, and proposal behavior remain in place.

`docs/guides/governance-recovery.md`, the audit inventory, alignment report, and documentation mapping snapshot were updated. The documented guarantee remains limited to ordinary in-process transaction rollback; process-death, power-loss, and arbitrary-writer recovery are not claimed.

Acceptance evidence:

- `20261127-084-mutation-preservation-focused-rust-corrected` — 36 mutation tests passed, including preimage failures, exact restoration, external drift, symlink/move cases, and transaction preservation.
- `20261127-084-mutation-preservation-legacy-python` — canonical Rust conformance 43/43, Python compatibility comparator 43/43, WI050 8/8, and WI079 10/10.
- `20261127-084-mutation-preservation-final-ci-validated` — all required CI profile stages passed on a clean exact-head checkout, including Python tests, Rust workspace, canonical validation, conformance, spec freshness, and frozen-surface verification.
- `20261127-084-mutation-preservation-closeout-validate` — quick canonical validation passed with all acceptance criteria and documentation impact linked.

Earlier failed validation attempts remain preserved as evidence. The completed profile ran with a D: repository and Cargo target and C: fixture temp; its exact head was `834994f90b54b5df25fd4abc9a66cc2e6a6ba7cb` before evidence reconciliation. The dashboard is regenerated from committed evidence only.

This prior closeout marked WI084 complete on the then-current evidence. The coordinator review subsequently reopened the same work item; the corrected closeout is recorded below.


## Reopened correction closeout

The coordinator review corrections are now reconciled against WI084's seven acceptance criteria. The work item retains all earlier evidence and failures, with the correction-specific results added:

- `20261127-084-wi084-corrections-focused` — 42 mutation tests passed, including direct and descendant hard-link sentinels, the rollback hard-link recheck, valid external edits between stale check and capture, injected file-read failure, captured-state mismatch, exact rollback, and a real Windows junction.
- `20261127-084-wi084-corrections-final-ci-validated` — all five required CI profile steps passed from clean commit `21d6f586343e65ae0262d64a76386a59ed56d792`; repository validation, Python tests, canonical conformance with complete process-image provenance, Rust workspace tests, spec freshness, and the frozen-surface report passed.
- `20261127-084-wi084-corrections-legacy-python` — explicit Python compatibility comparator passed 43/43 alongside canonical Rust conformance 43/43, WI050 admission 8/8, and WI079 reference corpus 10/10 (instruction-only).
- `20261127-084-wi084-corrections-closeout` — the canonical transaction moved WI084 to completed; post-transition repository validation, frozen-surface verification, and semantic-input freshness checks passed.

The initial profile failure, provenance-only retry, and interrupted attempts remain separately recorded as failed or partial; none is counted as passing evidence. The dashboard is generated from the reconciled records. WI084 makes no guarantee for external writes after the last observation, process death, or power loss.



## Coordinator follow-up reopening — AC-7

The coordinator's follow-up review on PR #34 identified one missing local comparison in `capture_preimages_with`: it checks that the surrounding root observations match, but did not compare the complete middle preimage entries against them. This can accept A/A during the first observation, B/B during capture, and A/A during the final observation. Root digests also omit empty directories, so the comparison must include complete entries and roots.

WI084 was reopened through the canonical lifecycle before runtime edits. Only the existing AC-7 is pending; its prior evidence and all other acceptance criteria remain unchanged. The correction is limited to the existing three checked passes and deterministic regressions for changing reads, stable reads, and empty-directory mismatch. No additional traversal pass or public `PathState` change is in scope.


### AC-7 comparison correction

`capture_preimages_with` now retains full `Preimages` values for the existing before, middle, and after checked passes, including every root and directory/file entry. It rejects unless the captured middle entries exactly match both surrounding observations; it adds no traversal pass and does not change `PathState` or the read-set contract.

Before the correction, the deterministic file test returned the middle `BBBB` bytes between two `AAAA` observations. The empty-directory test returned a middle snapshot containing a child directory absent from both boundary observations. After the correction, all 45 mutation crate tests passed, including those negative cases, a stable-file positive case, and unchanged sentinel bytes. Focused evidence: `20261127-084-wi084-preimage-consistency-focused`. The applicable local CI profile and explicit legacy comparator remain to be recorded for this runtime change.
