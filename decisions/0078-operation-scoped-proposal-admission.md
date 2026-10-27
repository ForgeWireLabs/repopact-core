---
id: 0078
title: Operation-scoped proposal admission and recovery
status: accepted
date: 2026-10-27
supersedes: []
---

# 0078: Operation-scoped proposal admission and recovery

## Context

The proposal operation is a low-authority lifecycle step: it records proposed
intent but grants no implementation or release authority. Running the complete
repository validator through temporary copies for both candidate planning and
post-mutation validation couples proposal latency to unrelated maintenance
state. The prior CLI also omitted its required preflight confirmation and
misreported the engine's structured rejection as a protocol error.

## Decision

`work.propose` uses an operation-scoped admission boundary. It still checks the
repository identity, supported surface, contracts, binding invariants, frozen
surface, owner scopes, candidate schema and identity, global work IDs, work
references, dependency cycles, and enabled scope-conflict rules. Broken
authoritative evidence and assurance references, sensitive-evidence errors,
and authorization/integrity errors remain blocking.

Unrelated work-record findings are returned as diagnostics without vetoing a
proposal. Proposal admission runs only review-expiry, audit-registry expiry,
and dashboard freshness projections as non-blocking warnings. It does not
traverse release, full verification, graph, adoption,
research, or documentation-mapping audits; the full validate operation remains
canonical for those repository-wide checks.
The proposal request requires explicit preflight confirmation and a caller
idempotency key. A receipt binds that key to a canonical request digest.
Retrying the same payload returns the associated item; reusing the key for a
different payload fails. The item, README, and receipt are written to a
same-volume staging directory; each staged file is passed to `sync_all()`
before one directory rename publishes the transaction. This rename is the
atomic-visibility boundary: during normal operation, the proposal becomes
visible as a complete item, README, and receipt together. The containing
directories are not synced after the rename, and Windows write-through is not
requested explicitly, so the implementation does not promise that the rename
survives sudden power loss. Power-loss persistence has not been tested.

Proposal commits and ordinary RepoPact mutations share a repository-scoped
lock with a two-second acquisition bound. A proposal rechecks its read set
after staging and immediately before publish; ordinary mutations check their
planned read set after acquiring the same lock. The client proposal deadline is
ten seconds. A timeout or invalid/lost operation response is an unknown
outcome, so the operator checks the read-only status operation. Status scans
all lifecycle directories, and a valid receipt remains resolvable after its
item moves to another lifecycle state. A matching malformed receipt, duplicate
key, or receipt whose work-item record is missing or inconsistent fails
closed. Malformed JSON or a missing/invalid key also fails closed when its key
cannot be identified; a valid receipt for a different key does not block
lookup. A missing receipt
reports `absent`, but cannot distinguish a request that never committed from a
receipt later removed from a committed item. Inspect the work ledger and
history before retrying an unknown outcome; do not retry if a corresponding
item exists without its receipt. A derived dashboard is regenerated after the
authoritative commit; failure is a warning and does not reverse or misstate the
commit.

`work.proposal-plan` is read-only and returns diagnostics and phase metrics.
`work.proposal-status` reads receipt state. Both have bounded client
deadlines. Phase measurements include snapshot, lock wait, receipt lookup,
staleness check, authority checks, work schema, dependency graph, maintenance
diagnostics, staging, atomic commit, dashboard regeneration, engine execution,
Python orchestration, record counts, source-file counts, and external
integration time (zero when none is used).

## Lifecycle rule disposition

This change scopes proposal admission only. The remaining lifecycle operations
retain their established canonical validation and operation-specific gates;
the table records that boundary so proposal optimization does not silently
change their contracts.

| Rule family | Propose / read-only plan | Amend proposed item | Activate or transition | Record evidence / complete | Release verification |
| --- | --- | --- | --- | --- | --- |
| Repository identity, supported surfaces, contracts, invariants, frozen paths, owner authorization | Block on applicable authority failures | Existing full validation and title-only proposed-item restriction | Existing full validation and transition gates | Existing full validation and evidence gates | Existing full validation and release gates |
| Work schema, unique IDs, record references, dependency graph, scope conflicts | Candidate and global work-ledger checks block | Existing full validation | Existing full validation | Existing full validation | Existing full validation |
| Evidence records, canonical evidence references, sensitive-evidence guardrails | Broken references and sensitive-evidence findings block | Existing full validation | Existing full validation | Existing full validation | Existing full validation |
| Assurance mapping references and review freshness | Broken canonical references block; expired review is a warning; review-claim and digest audits are not traversed | Existing full validation; review expiry remains governed by validator severity | Existing full validation | Existing full validation | Existing full validation |
| Release/version checks, verification profiles, graph/adopter/research/documentation audits | Not traversed during proposal admission | Existing full validation | Existing full validation | Existing full validation | Existing full validation plus explicit release checks |
| Dashboard and audit-registry freshness | Missing/stale dashboard and expired registry freshness are warnings; dashboard regenerates after commit | Existing full validation; an error in derived state still prevents generic mutation commit | Existing full validation | Existing full validation | Existing full validation |

Canonical full validation still reports every repository-wide diagnostic for
its designated assurance boundary. A warning remains non-blocking according to
the validator; this decision does not reclassify warnings for other operations.
## Performance contract

Routine proposals should finish in seconds. The initial target is p95 below
5,000 ms on a representative ForgeWire-scale fixture under recorded hardware
and workload conditions, with a 10,000 ms client deadline. Synthetic or Core
repository measurements are useful regression evidence but do not satisfy the
ForgeWire-scale target. Peak engine working set is measured where the host
exposes it; unavailable measurements must be reported as unavailable.

## Alternatives considered

- Keep full repository validation and increase the timeout: rejected because
  it preserves unrelated maintenance coupling and leaves no phase diagnosis.
- Skip work-ledger integrity checks: rejected because IDs, references, scopes,
  and dependency cycles protect repository consistency.
- Treat the dashboard as authoritative admission state: rejected because it is
  a derived projection and can be regenerated from source records.
- Retry after a timeout without a durable key: rejected because the caller
  cannot know whether the transaction committed.

## Limits

The operation-scoped result is not a replacement for canonical full validation.
Acceptance of this Core contract does not close WI077: its actual Fabric
reproduction and ForgeWire-scale performance measurement remain pending. The
target is not established until measured against the actual ForgeWire
governance dataset. A client timeout may still leave the outcome unknown; a
valid receipt resolves it, while absent or corrupt receipt state may require
operator reconciliation.
