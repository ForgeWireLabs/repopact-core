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

Evidence runs record exact command, executable/process-image provenance, source identity, exit status, and artifacts. A focused test run is not represented as the completion profile. Platform coverage is reported by environment and scope; the WSL run proves mutation-package behavior only, not a full Linux profile. All accepted criteria now have linked evidence, and the coordinator moved the completed directory and regenerated the dashboard.

## Completed implementation and validation

The bounded ordinary-v1 transaction store, root-bound admission lock, conservative explicit recovery, stable-key replay/conflict outcomes, and read-only status are implemented in the Rust mutation engine. Transaction ownership remains distinct from proposal receipts and proposal-stage cleanup. The private repository-local journal and lock directory is ignored by Git; it contains runtime state only.

Synchronized child-process termination tests killed actual child processes across the transaction boundaries and verified explicit recovery on both Windows and WSL Debian. Evidence `20261128-wi085-mutation-subprocess-platforms` records the focused mutation-package runs; Linux coverage is package-only, not a full Linux profile.

The clean C1 candidate (`17e828f1301699bb7ce692fecc5e7f6c58e0c576`, tree `32568a6640ee9acfbb36042d74a55448190f9bc6`) passed the local `ci` profile in 2,546.609 seconds. Its required validation, Python suite, canonical conformance, Rust workspace, and SPEC freshness stages passed; its optional frozen-surface report also passed. See `20261127-wi085-ci-windows-passing.json`.

The separately required clean-source legacy comparator passed with complete engine process-image provenance: Rust 43/43, Python compatibility 43/43, WI050 8/8, and WI079 reference 10/10. WI079 remains instruction-only evidence. See `20261128-wi085-legacy-python-comparator.json` and its linked conformance artifact.

A first post-profile legacy-comparator attempt passed the same vectors but returned incomplete provenance because the newly recorded profile evidence made the source tree dirty. Its partial artifact, `20261128T032540634595Z-wi076-ac6-engine-conformance.json`, is preserved. The comparator was rerun from clean C1 with matching engine build provenance and passed; the successful run is `20261128T032944884645Z-wi076-ac6-engine-conformance.json`.

The complete local CI profile is `20261127-wi085-ci-windows-passing`; the focused cross-platform child-process evidence is `20261128-wi085-mutation-subprocess-platforms`. Final completed-work dashboard, canonical validation, semantic-identity check, and frozen-surface result are recorded in `20261128-wi085-closeout-validation`. No hosted CI/CD, merge, or release was performed.

## Coordinator corrections — active

Coordinator review reopened this existing item via the Rust `repopact-mutation` transition API on 2026-11-28, moving the item from `completed` back to `active`. Previous validation evidence and the earlier closeout record remain preserved; AC-2 through AC-4 are pending the correction batch and later coordinator-authorized assurance.

Correction authority: [PR #35 consolidated review](https://github.com/ForgeWireLabs/repopact-core/pull/35#issuecomment-7241654203). Required bounded changes:

1. Constrain recovery decisions to durable phase/cursor state; preserve live states matching unissued future operations; make rollback cursors exact; checkpoint recovery before each action; test interrupted recovery, no-op/move and ambiguous ownership boundaries.
2. Report false/unconfirmed receipt persistence as unsuccessful and nonterminal, distinguish durable receipt plus cleanup-pending from unknown receipt state, and test raw Rust, engine, and Python outcomes including after-publish errors and lost responses.
3. Replace human `icacls` parsing with conservative native SID/ACE inspection for directories and files; secure new storage before writing; preserve existing unknown ACLs/contents; keep proposal-anchor migration separate; test disposable ACL negatives and normal permissions without altering real repository ACLs.
4. Bound operation count and aggregate projection memory before allocation; validate typed paths, roots, parents, and lossless path representation; verify prepared read-back; check runtime-state exclusions with actual repository readers; document receipt-cap handling and measured existing-fixture storage overhead; add missing cross-process and child-transition interruption cases.

No full CI profile or legacy comparator is authorized in this batch. The candidate is for coordinator review after focused Windows and Linux mutation-package regression evidence.
