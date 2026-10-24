# 047 — Documentation Impact and Code/Documentation Closure

> **Status**: ✅ Completed — independently accepted against the integrated Core default branch on 2026-10-23.
> **Owners**: governance-owner (lead); tooling-owner, docs-owner, evidence-owner, and work-coordinator affected.
> **Depends on**: none.
> **Decision**: [`0065`](../../../decisions/0065-documentation-closure-is-mandatory-at-work-item-completion.md).

## Intent

RepoPact currently requires governed work, acceptance criteria, evidence, reconciliation, and generated artifacts, but it does not yet require a code change to explicitly resolve documentation impact before closeout.

This creates a durable-state gap: implementation can change while user, API/CLI, configuration, architecture, operations, contributor, experimental-status, example, or generated documentation remains stale or its impact is simply never considered.

The target rule is:

> A work item that changes governed implementation code cannot complete until documentation impact is explicitly resolved. If documented behavior, interfaces, configuration, architecture, operations, maturity, or other durable contracts changed, the corresponding documentation must be created, updated, or regenerated and linked to concrete evidence. A no-documentation-impact outcome requires an explicit rationale; silence is not acceptable.

The purpose is not to force meaningless README churn for every internal refactor. The purpose is to make documentation impact a required closeout decision rather than an optional afterthought.

## Relationship to existing RepoPact work

This item is distinct from:

- **F-016** — parity between a work-item README and its canonical manifest;
- **WI044** — whether a completion/cutover claim has semantically sufficient evidence;
- **WI046** — whether verification is invoked/effective at an admission boundary.

WI047 adds a different closure dimension: whether the repository's durable explanation remains reconciled with changed implementation.

## Candidate model — not yet a decision

A governed code change should close in one of two states:

1. **documentation affected** — identify the affected documentation surfaces and prove they were created, updated, or regenerated; or
2. **documentation not affected** — record an explicit reviewable rationale.

An unresolved or omitted documentation-impact state must not permit a work item containing governed code changes to transition to completed.

Potential documentation surfaces include user behavior, public API/CLI, configuration, architecture/decisions, operations/runbooks, contributor/developer workflow, experimental or maturity status, examples, and generated documentation. Adopters remain free to define their own paths and layouts.

RepoPact should evaluate optional source-to-documentation mappings or equivalent relationships so it can detect structural freshness risks without pretending generic static analysis can prove arbitrary prose semantically correct.

## Non-goals

- Do not require arbitrary Markdown edits merely because source code changed.
- Do not assume every internal refactor affects documentation.
- Do not make README.md the universal documentation target.
- Do not duplicate F-016 representation-parity work.
- Do not make a particular CI provider the source of truth for documentation closure.
- Do not claim generic source analysis can prove that prose is semantically correct.
- Do not retroactively invent documentation-impact decisions for historical completed work.

## Implementation ordering

Contract and representation design come first. The work must compare possible placement at work-item, acceptance-criterion, evidence, and dedicated mapping/impact-record layers before changing schemas or validators.

After a design is accepted, implementation should cover validation, templates, workflow guidance, doctor/audit behavior, conformance, generated documentation freshness, evidence linkage, and negative tests proving unresolved documentation impact actually blocks closeout.

## Closeout

Every acceptance criterion in `work-item.json` must be linked to concrete evidence. Closeout must include both a positive code-plus-documentation case and a justified no-documentation-impact case, plus negative proof that code changes with unresolved documentation impact are rejected.

### What landed (2026-09-18)

Decision [`0065`](../../../decisions/0065-documentation-closure-is-mandatory-at-work-item-completion.md) accepts the model and the completion gate is implemented and enforced by both engines:

- `documentation_impact` work-item field (schema-validated shape: `affected` needs `surfaces`+`evidence`, `none` needs `rationale`) — `repopact/schemas/work-item.schema.json`.
- Opt-in, date-epoch-grandfathered enforcement at the `completed` transition, mirroring decision 0021's preflight mechanism — `repopact/validate_repo.py` (Python) and `rust/crates/repopact-validation/src/lib.rs` (canonical Rust engine), both wired to `governance/owners.json`'s `documentation_impact` block.
- `SPEC.md` rule 17 documents the contract and its relationship to F-016/WI044/rule 14.
- Two new conformance cases (`documentation-impact-missing-on-completed`, `documentation-impact-affected-unknown-evidence`) prove the negative direction on both the Python and canonical-Rust comparators (37/37 conformance cases pass).
- This work item is itself the first `documentation_impact: "affected"` record, evidenced by `evidence/runs/20260918-047-documentation-closure-implementation.json`.

### Open at the 2026-09-18 checkpoint (superseded by the follow-up below)

- Adopter-declared source-to-documentation mappings / structural freshness detection (AC-7) and generated-documentation staleness wired into `doctor` (AC-8, AC-10) are deliberately deferred to a follow-up decision — see decision 0065 section E. Implementing them now, with zero lived experience of what real `documentation_impact` records look like, risked exactly the unfalsifiable generic-analysis claim the decision disclaims.
- A second, explicitly adopter-neutral validation fixture beyond the conformance corpus (AC-12) is not yet built.
- `repopact new` does not yet scaffold a `documentation_impact` stub, and there is no dedicated CLI/doctor surfacing of *which* completed items are missing it ahead of the hard rejection (part of AC-14).

### Implementation follow-up (2026-10-22)

The follow-up design is recorded in decision [`0076`](../../../decisions/0076-structural-documentation-freshness-mappings.md). It adds opt-in, adopter-declared source-to-documentation mappings with a versioned schema, a read-only `repopact documentation snapshot` command, template support, structural freshness checks, generated-output staleness detection, and non-destructive doctor diagnostics. Mapping configuration remains optional; the mechanism reports changed inputs and outputs without claiming semantic prose verification.

The ForgeWire-shaped fixture and a separate adopter-neutral fixture exercise mapping behavior. Conformance now covers accepted and stale mappings, generated documentation drift, documentation-impact evidence requirements, and no-impact cases. Rust and Python conformance each pass 43/43; WI050 passes 8/8. The clean repository mapping inspection reports no stale mappings. The generated snapshot records source digest `90ae66ac8b020b02179617abca52c4a81529213e00017858f451b90e204cbe59` and documentation digest `6a7b56a721f7965e9cafedb5f980758d186016f15bb4be426b429c9c0d8b618b`.

The complete Python suite ran 385 tests: 373 passed, 9 failed, and 3 existing tests were skipped. The nine failures reproduce the unchanged Core baseline and are confined to expired governance audit scopes 026/027; the audit dates were not renewed by this implementation. Canonical validation consequently remains partial until those scope records receive their own substantive review. The new criteria are linked to evidence in the work-item manifest, but WI047 remains active for independent governance review and is not closed by the documentation PR sequence.

At this 2026-10-22 checkpoint, the focused implementation was in [draft Core PR #8](https://github.com/ForgeWireLabs/repopact-core/pull/8), separate from the four documentation PRs, and still required integration after the dependency sequence. That status was superseded by the merge and acceptance recorded below.

## Independent post-merge acceptance (2026-10-23)

WI047 was independently reviewed against integrated Core main
`8dac0def8c0e34d892b37d8673d543237928cea1`. All 14 acceptance criteria are
linked to [`20261023-047-independent-acceptance`](../../../evidence/runs/20261023-047-independent-acceptance.json).
The review covered the opt-in source-to-documentation mappings, Rust/Python
conformance, generated-output freshness, non-destructive doctor behavior,
negative conformance, schema authorization and compatibility, evidence
provenance, and documentation coverage.

The 2026-10-22 implementation follow-up above is an immutable pre-integration
checkpoint. Its statement that audit dates were not renewed described that
checkpoint; Core #7 subsequently changed scopes 026 and 027 to
`last_reviewed: 2026-10-22` and `next_review: 2027-01-20` after the substantive
WI075 review. Those dates are not advanced again by this closeout. WI075 remains
limited to the bounded ForgeLink adopter re-review: 16 unrelated ForgeLink audit
scopes remain expired, so this acceptance does not claim ForgeLink-wide
governance is clean.

Core #8 added the frozen `repopact/schemas/documentation-mappings.schema.json`
through the operator-approved merge at `52974409b0dbc6e0fe25aec7dfe12ec3b5ae1f0b`.
Optional mapping configuration preserves compatibility for existing
repositories that do not opt in; schema version 1 and the migration boundary
are recorded in Decision 0076. Historical evidence and the
2026-10-22 engineering-readiness report remain unchanged.
