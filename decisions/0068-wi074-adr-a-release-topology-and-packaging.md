---
id: 0068
title: WI074 ADR-A release topology and packaging
status: accepted
date: 2026-09-24
supersedes: []
---

# 0068: WI074 ADR-A — release topology and packaging

## Context

Core already ships the stable `repopact` Python distribution and CLI together
with its matching native engine. Workbench is a desktop/mobile application with
different runtime, platform, and release needs.

## Decision

Keep the `repopact` distribution and CLI identity in Core; preserve stable
3.1.3 artifacts unchanged. Workbench has an independent application version,
release cadence, and compatibility record. The compatibility/release manifest
identifies Core version and source commit, each Rust crate source identity,
wheel and sdist hashes, embedded engine hash, protocol version, schema and
conformance versions, tested Workbench version, target platform, and evidence.
S3 creates no release or published artifact.

## Alternatives considered

- Replacing the `repopact` PyPI identity: rejected because existing Python
  adopters depend on it.
- Bundling Workbench into the Core package or sharing a release train: rejected
  because their runtimes and platform release evidence differ.
- Changing or republishing stable 3.1.3 artifacts: rejected; those artifacts
  remain immutable.

## Consequences

Compatibility claims must be explicit and evidence-backed. A Core version
alone does not prove a Workbench/platform combination. Publication and
distribution hosting remain separately authorized.
