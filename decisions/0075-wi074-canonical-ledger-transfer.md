---
id: 0075
title: WI074 canonical governance ledger transfer to ForgeWireLabs Core
status: accepted
date: 2026-09-25
supersedes: []
---

# 0075: WI074 canonical governance ledger transfer to ForgeWireLabs Core

## Context

The S2 source manifest assigns WI074's inherited governance records to Core.
The published `ForgeWireLabs/repopact-core` tree had the original WI074
registration in `proposed`, while the immutable S5 reconciliation lineage
contained its active state, accepted ADR-A through ADR-E, Decision 0074, and
later evidence. Workbench has no duplicate WI074 record. The operator
authorized Core as the canonical owner and authorized a provenance-preserving
sanitized transfer on 2026-09-25.

## Decision

`ForgeWireLabs/repopact-core` is the single canonical ledger for ongoing WI074
acceptance and governance. Transfer the existing work item to Core's `active`
ledger and retain its existing identifier and 24 acceptance criteria. Import
decisions 0067-0074 under their original identifiers, preserving their content,
dates, approval/deviation timestamps, and status relationships. Keep the
historical S2-S5 lineage and source repositories as immutable provenance; do
not create a second active WI074 record in Workbench or treat the old monorepo
as an ongoing authority.

The Core work item retains the S5 state: RPS-003, RPS-008, RPS-021, RPS-023,
and RPS-024 are satisfied; the other 19 criteria remain pending. Decision 0074
remains the disposition of the S2-before-activation deviation. Nothing in this
transfer backdates authorization or changes the original preflight or S2/S3
commits.

The transfer manifest records source repository lineage, source commit/path,
source SHA-256, destination, destination SHA-256, sanitization status and
rationale, original event/approval time where applicable, and actual transfer
time. Machine-specific paths and former-account metadata are not copied into
the public transfer records. Redacted S5 summaries link to the underlying
immutable source records by commit and content hash.

## Consequences

Future WI074 acceptance-state changes, decisions, and evidence are governed in
Core under its registered owner scopes. Cross-repository criteria continue to
name Workbench, Proving Ground, or ForgeWire as affected owners, but no consumer
is changed by this decision. GitHub Actions remain disabled. This governance
transfer authorizes no binary/package publication or consumer migration.

## Approval and chronology

The operator authorized canonical ownership and sanitized publication in the
2026-09-25 transfer instruction. The actual transfer timestamp and approval
evidence is recorded in
`evidence/runs/20260925-074-canonical-governance-transfer.json` and
`evidence/WI074-transfer/20260925-transfer-manifest.json`. The original
architecture approval remains `2026-09-24T21:31:54Z`; it is not replaced by
this transfer authorization.
