---
id: 0070
title: WI074 ADR-C canonical semantic interface
status: accepted
date: 2026-09-24
supersedes: []
---

# 0070: WI074 ADR-C — canonical semantic interface

## Context

The integrated desktop API and UI call existing Rust domain crates. Splitting
the workspace must not create a second governance implementation or couple S3
to an unreviewed Core API redesign.

## Decision

Core remains the sole authority for schemas, record semantics, validation,
graph/analysis, mutation, and the headless engine protocol. Workbench may own
desktop sessions, filesystem watchers, UI state, presentation DTOs, and local
acceleration. S3 preserves existing direct domain-crate interfaces and pins
them to the same immutable Core revision; do not redesign the Core façade in
this slice. Preserve the engine's versioned fail-closed handshake. Record a
distinct Workbench/Core compatibility contract in addition to the existing
Python/engine protocol identity.

## Alternatives considered

- Implementing validation, graph, or mutation semantics in Workbench: rejected
  because it would create competing authorities.
- Redesigning the Core façade while extracting Workbench: deferred; it broadens
  scope and confounds extraction defects with API redesign.
- Treating the Python/engine handshake as proof of Workbench compatibility:
  rejected because those are separate consumer interfaces.

## Consequences

Workbench tests must exercise DTO/type compatibility against the pinned Core.
A genuine defect in that interface is reported with reproduction evidence
before changing Core or introducing a compatibility shim.
