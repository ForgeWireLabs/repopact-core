---
id: 0069
title: WI074 ADR-B immutable Rust dependencies
status: accepted
date: 2026-09-24
supersedes: []
---

# 0069: WI074 ADR-B — immutable Rust dependencies

## Context

The extracted Workbench currently consumes several internal domain crates.
S3 must build independently of the integrated monorepo without copying Core
source or publishing internal crates.

## Decision

For the local S3 proof, use Git dependencies pinned to the full Core commit
`6aff2c376efb5ddf236bd11cd1d700f873ec6a4f`. Their source URL is the local
Core Git checkout and is recorded verbatim in the S3 manifest. Every direct
Core crate dependency uses the same URL and `rev`; transitive workspace
dependencies resolve from that pinned Core source. Cargo metadata and tree
checks must prove identity and detect duplicates. The resulting local
lock/source configuration is machine-specific and must not be described as
portable.

Do not use a branch/tag selector, original-monorepo path dependency, copied
Core crate, or registry publication. Once an approved Core remote exists, a
separate authorized migration may replace the local URL with the same full SHA
and must regenerate and verify the lockfile.

## Alternatives considered

- Floating branch or tag: rejected because the resolved source can change.
- Local path dependency into the integrated source: rejected because it is not
  an independent build.
- Vendored/copied Core crates: rejected because of provenance and semantic
  duplication risk.
- Publishing internal crates: deferred; it adds release/API obligations and
  is outside the local-only S3 authorization.

## Consequences

The S3 checkout is reproducible only where the pinned local Git repository is
available. Portable clean-clone and offline distribution proof remains a later
acceptance requirement.
