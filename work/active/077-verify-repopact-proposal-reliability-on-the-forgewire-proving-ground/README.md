# 077 — Verify RepoPact proposal reliability on the ForgeWire proving ground

> **Status**: active
> **Priority**: high
> **Owner scope**: tooling (lead)
> **Affected scopes**: tooling, governance, docs, evidence
> **Depends on**: none

## Intent

This work item verifies and corrects the remaining operational bottlenecks observed after the Core governance-throughput correction. PR #13 and Decision 0078 established operation-scoped proposal admission while retaining canonical full validation as the repository-wide assurance boundary. That implementation and its Core-only benchmark are prior work; they do not satisfy this proving-ground work item.

The initial ForgeWire proving-ground run is recorded in the [results report](../../../evidence/incidents/2026-10-27-wi077-forgewire-proving-ground-results.json) and [evidence manifest](../../../evidence/runs/20261027-077-forgewire-proving-ground.json). Ten measured ForgeWire-scale proposals committed, but p95 was 9,464.412 ms, above the 5,000 ms target. Concurrent CLI proposals produced lock timeouts and one unknown outcome; same-key recovery committed without a duplicate. The full-size lifecycle transition exceeded its 60-second bound.

The original Fabric incident trace and contemporaneous host/repository evidence were unavailable. AC-1 therefore remains pending and the historical cause remains unresolved. This work item is active; its current evidence is partial and none of its acceptance criteria is satisfied.

## Decisions

Decision 0078 scopes proposal admission while retaining canonical full validation as the repository-wide assurance boundary. It remains accepted. Any change to normative rules requires a separate versioned decision and conformance update.

## Scope

The tooling owner leads. Governance, documentation, and evidence owners are affected. Diagnose snapshot acquisition and lock hold/wait behavior, verify the default installed-engine path, and trace lifecycle transition latency independently. Make only targeted corrections supported by measurements and correctness tests. Preserve cross-process serialization, stale-state rejection, atomic publication, idempotent recovery, lifecycle authorization, and canonical full validation.

The Core benchmark is not a substitute for ForgeWire-scale results. Keep slow samples and timeout observations in the record. Do not introduce a repository-wide cache without profiling evidence and sound invalidation semantics.

## Acceptance criteria

See `work-item.json` for the three pending, evidence-linked criteria covering the unavailable Fabric incident reproduction, small/medium/ForgeWire-scale performance, and adversarial acceptance. The current ForgeWire p95 misses its target, and the actual-scale concurrency and lifecycle investigations remain incomplete.

## Closeout

Keep this item active until the criteria are either satisfied with linked evidence or explicitly revised with operator approval. Record unavailable inputs and unsuccessful measurements without claiming success. Resolve documentation impact before marking this work complete. Then move the whole item directory to `work/completed/` and regenerate the dashboard.
