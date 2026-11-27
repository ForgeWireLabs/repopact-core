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

## Closeout

Preflight: registered as WI084 against main `298828de6ae91f34dedd4f7cc6d98587f0959704`, tree `5fd33f98201f0ff313a199dc99ba74d0fb21ca8f`, before runtime edits. The dirty original checkout and PR #15 branch were not changed.

Scope is limited to complete checked preimage capture, fail-closed handling for unsafe or ambiguous paths, and rollback of transaction-owned state. Preserve proposal semantics, plan identity/read-set checks, mutation locking, stale checks, and independent postvalidation. No crash/power-loss or arbitrary concurrent-writer guarantees are claimed.

Link actual evidence to every acceptance criterion before closeout. Keep this work item active until those links are recorded; regenerate the dashboard after governed changes.
