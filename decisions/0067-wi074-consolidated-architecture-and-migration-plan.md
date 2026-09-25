---
id: 0067
title: WI074 consolidated architecture and migration plan
status: accepted
date: 2026-09-24
supersedes: []
---

# 0067: WI074 consolidated architecture and migration plan

## Context

WI074 separates RepoPact Core from the Tauri desktop/mobile Workbench while
preserving one canonical governance authority. The accepted plan is bounded by
the immutable integrated source `8f1ce8deb139287655afcc8479dc69dd621d8720`,
the standalone Core S2 commit `6aff2c376efb5ddf236bd11cd1d700f873ec6a4f`, and
the complete 1,048-path S2 source map. The operator approved this plan at
`2026-09-24T21:31:54Z`; the linked approval evidence authorizes S3 prospectively.

## Decision

1. Retain the `repopact` PyPI/CLI identity and immutable stable 3.1.3 artifacts
   in Core. Workbench has a separate application version and release lifecycle.
   A versioned compatibility/release manifest must identify Core source and
   crate revisions, package/artifact hashes, engine protocol, schema and
   conformance identities, tested Workbench version, and platform evidence.
   S3 is local-only and does not publish or enable hosted workflows.
2. For S3, consume Core through immutable Git dependencies at full revision
   `6aff2c376efb5ddf236bd11cd1d700f873ec6a4f`. The bootstrap URL is the local
   Core Git checkout recorded in the S3 manifest. Every directly imported Core
   crate must use that same URL and revision; transitive workspace crates must
   resolve from that pinned Core tree. No floating branch, copied Core source,
   or registry publication is permitted. This local lock/source configuration
   is not portable; replacing the local URL requires a later approved Core
   remote and dependency-migration evidence.
3. Core alone owns governance schemas, interpretation, validation, graph,
   analysis, mutation, and engine semantics. Workbench owns UI, session state,
   filesystem observation, and presentation. S3 preserves the existing domain
   crate dependency structure; it does not redesign the Core façade. Keep the
   engine handshake fail-closed and define Workbench/Core compatibility
   separately from the Python/engine protocol. A demonstrated Core interface
   defect is reported before any Core change.
4. Preserve the shipped Core headless admission, guard, confinement, and
   sandbox capabilities in Core. Workbench owns the Tauri shell, desktop API,
   mobile acquisition, SAF, platform credential integration, and optional
   remote/GitHub providers. GitHub login remains optional; secrets remain in
   native credential facilities and never enter frontend code or build output.
   Code movement alone does not change an assurance claim. Platform support is
   reported only for targets actually built and tested.
5. Core and Workbench maintain distinct authoritative ledgers for their own
   future work. Do not mirror the entire historical Core ledger into Workbench.
   Preserve original IDs, timestamps, evidence, and source ancestry; use the
   S2 path map as the migration authority and assign each inherited record one
   canonical owner. Workbench-specific governance must not compete with Core
   semantics. Coordinated changes use explicit cross-repository dependencies.

## Migration sequence

- **S2 (completed before formal activation):** retain Core at its exact S2
  commit; do not rewrite its history or claim this approval authorized S2.
- **S3 (authorized by this approval):** create a local Workbench branch from
  the integrated source baseline; extract and hash-reconcile the 194 manifest
  rows owned by Workbench; establish a standalone Cargo workspace, frontend
  metadata, lockfiles, build scripts, and minimal Workbench governance; pin
  Core as above; verify Rust/Cargo identity, frontend types/tests/build, Windows
  Tauri build, and supported repository/session/graph/mutation/watcher paths.
  Attempt Android only if the local SDK and device/emulator are available.
- **S4/S5:** consumer migration, remote repository creation, publication,
  hosted automation, and release operations remain separately authorized work.

## Stop conditions and boundaries

Do not modify the integrated source or immutable S2 Core checkout, create
remotes, push, publish, enable Actions, access credentials, invoke paid
services, migrate ForgeWire/Proving Ground, or start later slices. Do not claim
Linux/macOS/Android/iOS support without native evidence. Stop on a dependency
identity mismatch, missing source path/hash, or demonstrated API defect that
would require changing Core.

## Alternatives

- Floating Git refs or uncontrolled tags: rejected because they do not identify
  the approved Core source immutably.
- Copying internal Core crates into Workbench: rejected because it creates a
  second semantic authority and breaks provenance.
- Publishing internal crates during S3: rejected because publication and
  remote distribution are outside this local proof and require separate
  authorization.
- Rewriting the Core façade during extraction: deferred; S3 preserves existing
  interfaces and reports demonstrated incompatibilities instead.

## Approval and chronology

The operator's explicit approval and its actual UTC timestamp are recorded in
`evidence/runs/20260924-213154-074-architecture-approval.json`. A separate
deferred decision records that S2 commits predate formal WI074 activation and
that this S3 approval does not retroactively authorize S2.
