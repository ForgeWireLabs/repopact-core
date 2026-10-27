# 077 — Verify RepoPact proposal reliability on the ForgeWire proving ground

> **Status**: proposed
> **Priority**: high
> **Owner scope**: tooling (lead)
> **Affected scopes**: tooling, governance, docs, evidence
> **Depends on**: none

## Intent

This follow-up closes the remaining proving-ground portion of the governance throughput correction. The Core implementation, Core benchmark, and regression evidence already present in this branch are documented in [Decision 0078](../../../decisions/0078-operation-scoped-proposal-admission.md) and the [engineering incident record](../../../evidence/incidents/2026-10-27-governance-throughput-correction.md). They are not represented as implementation performed under this newly registered item.

The private Fabric repository, governance dataset, invocation trace, and host telemetry were unavailable during the initial correction. This item remains proposed until the proving-ground inputs are available and the actual Fabric behavior and ForgeWire-scale performance are verified.

## Decisions

Decision 0078 scopes proposal admission while retaining canonical full validation as the repository-wide assurance boundary. Any change to normative rules requires a separate versioned decision and conformance update.

## Scope

The tooling owner leads. Governance, documentation, and evidence owners are affected. The work item is limited to actual Fabric reproduction, ForgeWire-scale performance verification, and adversarial proving-ground acceptance.

## Acceptance criteria

See `work-item.json` for the three pending, evidence-linked acceptance criteria covering Fabric reproduction, small/medium/ForgeWire-scale measurements, and adversarial acceptance behavior.

## Closeout

Record unavailable inputs and unsuccessful target measurements without claiming success. Link actual evidence to every acceptance criterion and resolve documentation impact before marking this work complete. Then move the whole item directory to `work/completed/` and regenerate the dashboard.
