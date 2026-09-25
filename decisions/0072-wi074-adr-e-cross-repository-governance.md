---
id: 0072
title: WI074 ADR-E cross-repository governance
status: accepted
date: 2026-09-24
supersedes: []
---

# 0072: WI074 ADR-E — cross-repository governance

## Context

The original integrated repository has one established authoritative ledger.
After extraction, Core and Workbench need independent ownership without
duplicating historical records or losing provenance.

## Decision

Core remains canonical for Core-owned semantics and the inherited governance
records assigned to it by the S2 source map. Workbench receives its own
minimal governance contracts, ledger, decisions, evidence, and derived
dashboard for Workbench-owned implementation and future work; it does not copy
the full monorepo ledger. Preserve source IDs, timestamps, and history through
the complete path-level migration manifest, and retain original monorepo
history as immutable provenance. Each inherited record has one canonical
owner. Cross-repository changes declare dependencies and a lead; compatibility
changes and releases require approval by the owning repository. No S4 consumer
migration or S5 repository/publication action is authorized here.

## Alternatives considered

- Duplicating all historical records into both repositories: rejected because
  it creates competing authorities and divergent copies.
- Leaving Workbench without any repository-local governance: rejected because
  its future implementation state would again exist only in conversation or an
  external tracker.
- Moving every inherited record into Workbench: rejected because Core owns the
  canonical governance engine and the source map assigns records by owner.

## Consequences

The S3 manifest records each Workbench source path/hash and links its provenance
to the immutable integrated revision and exact Core pin. The new Workbench
ledger is intentionally limited to Workbench-owned work and decisions.
