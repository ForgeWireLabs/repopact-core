# 085 — Recover interrupted ordinary mutations safely

> **Status**: Active
> **Lead**: tooling-owner
> **Owner scope**: tooling
> **Affected scopes**: tooling, governance, work, docs, evidence
> **Depends on**: none

## Intent

Make the supported ordinary typed mutation path recoverable after its process is terminated, while preserving WI084's containment, captured-state, preimage, transaction, and independently validated post-state guarantees. Keep recovery explicit and conservative so interrupted work never authorizes a stale plan or overwrites uncertain external data.

## Authority and registration

The repository operator authorized this batch in the current request citing [PR #15 coordinator authorization](https://github.com/ForgeWireLabs/repopact-core/pull/15#issuecomment-7230402419). Decision [0080](../../../decisions/0080-ordinary-process-interruption-recovery-for-typed-mutations.md) records the accepted bounded architecture and its limits. WI085 was allocated through the canonical Rust work creation path on isolated WI084 main `1223cab5562f2f4c5a77acb5e6ce8d033aa5a0cb`, tree `685528fcfc59601b6c32470d3cd07a43dfebf8b1`. The dirty historical PR15 worktree is preserved and is not the implementation base.

## Scope

- Supported ordinary Rust typed work-item create, edit, and lifecycle transition through the existing mutation/engine/CLI admission path.
- A bounded, private `work/.repopact-transactions/ordinary-v1` journal and one stable per-working-root lock shared with proposal commit/admission.
- Stable idempotency keys, read-only status, explicit recovery, bounded terminal receipts, and conservative ambiguity handling.
- Guard the overlapping Rust `dashboard.write` direct writer. Preserve proposal receipts and exact proposal-stage cleanup. Do not claim that unrelated Python, graph, or out-of-band writers participate in a universal repository lock.
- Add synchronized child-process termination tests, including Windows and Linux where existing local environments support them.
- Update the API/CLI and operator documentation, semantic identity, scoped audit traceability, evidence, and generated dashboard; prepare one locally validated draft PR.

## Writer inventory and recovery boundary

The ordinary mutation core is the only writer covered by this transaction format. Its finite planned write set includes work-item JSON/README writes or a complete work-item directory move, plus `audits/reports/dashboard.md`. The `dashboard.write` engine operation is a direct writer to that same dashboard path, so it must acquire the same root lock and reject a pending ordinary transaction. Proposal commit already acquires the mutation lock; it will reject pending ordinary state while retaining its distinct stage and receipt format. Graph output is under `build/`, and evidence-run creation writes `evidence/`; these targets do not overlap the ordinary write set. Other bootstrap/import/takeover and out-of-band tools are nonparticipants. Operators must pause external writers to transaction paths while explicit recovery is running. No universal repository-write lock is claimed.

## State and safety contract

Decision 0080 defines the state machine, operation ownership, request-key binding, root identity, byte/entry bounds, lock scope, terminal receipt rules, and no-power-loss limit. In short: fully verify and publish `PREPARED` before the first live write; checkpoint ordered operations and captured before/after state; publish `COMMITTED` only after live validation; on explicit recovery reverse only paths that still exactly match this operation's after-state; preserve ambiguous data and report recovery required. A key with changed intent conflicts; a same-intent terminal replay returns its recorded outcome without another mutation. Missing state never proves no operation occurred.

Historical PR15 sources may be consulted for isolated serialization/publication patterns. Blind root restore, lossy case-folded identity, per-user temp journals, prefix deletion, no-op ACL handling, automatic recovery, stale-plan replay, and obsolete semantics are excluded.

## Acceptance criteria

- [x] **AC-1** — accepted authority, WI084-based design, writer overlap, states, bounds, and explicit non-goals are recorded before runtime edits. See `20261127-wi085-design-authority-preflight`.
- [ ] **AC-2** — implement bounded journals, same-root admission and stable-key outcomes, explicit status/recovery, and scoped guards without changing proposal cleanup or receipts.
- [ ] **AC-3** — prove synchronized real child-process termination, safe/idempotent recovery, key behavior, lock/root/path checks, and preservation of uncertain or unrelated data; report per-platform coverage honestly.
- [ ] **AC-4** — run focused regressions and one clean declared local CI profile plus the required separate legacy Python comparator; reconcile semantic identity, docs, audits, evidence, dashboard, canonical validation and frozen checks; publish one draft PR with exact source/tree evidence.

## Validation and closeout

Evidence runs record exact command, executable/process-image provenance, source identity, exit status, and artifacts. A focused test run is not represented as the completion profile. The final report names any unavailable OS coverage and retains affected criteria pending if required evidence cannot be produced. The full directory remains active until all accepted criteria have linked evidence; then the coordinator can complete it and regenerate the dashboard.
