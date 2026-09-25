# 074 — Extract RepoPact Core and Independent Workbench

> **Status:** Proposed  
> **Owner:** tooling  
> **Lead:** tooling-owner  
> **Affected scopes:** tooling, governance, docs, evidence  
> **Created:** 2026-09-24  
> **Provenance:** Concrete for the approved architectural direction; implementation and migration details require verification.

## Intent

Plan a separation into two independently buildable, tested, versioned, and
releasable products without creating competing semantic authorities:

- `ForgeWireLabs/repopact-core`, retaining the existing PyPI distribution name
  `repopact`, public CLI, canonical Rust semantic engine/protocol, schemas,
  validation, graph, analysis, mutation, conformance, and supported headless
  admission functionality.
- `ForgeWireLabs/repopact-workbench`, a standalone Tauri 2 desktop/mobile
  application consuming independently versioned core components and owning its
  UI, application orchestration, native integration, installers, and releases.

The Proving Ground remains a separate research/benchmark project consuming
explicitly identified core releases. The governing principle is one canonical
governance engine with independently released consumers.

This proposal authorizes planning and architecture only. It does not authorize
physical extraction, repository creation, publication, destructive migration,
or changes to hosted workflows.

## Source baseline and preservation

The proposed source is the local RepoPact checkout at WI072 commit
`92251565459dd28e3f74d44782ec14ad41758508`, ahead of the last verified remote
`main` (`686c986de9f8fa1e73679e5d34d51cfc99af5363`). Completed WI073 and its
uncommitted evidence, adopter-registry changes, and dashboard belong to the
original dirty checkout and must remain untouched. Verified committed-history
and uncommitted-state backups are retained outside this repository. GitHub Git
transport has returned HTTP 403; do not work around that restriction.

RepoPact 3.1.3 is the current stable dependency. Its published identity and
artifacts are immutable. The research review dated 2026-09-24 is recorded in
`research/README.md`; manuscript and historical evidence are not rewritten to
claim a 3.1.3 study.

## Proposed product boundaries

### RepoPact Core

Core remains the sole authority for governance schemas and semantics, record
interpretation, repository discovery/snapshots, validation, work lifecycle and
mutation, graph and analysis, engine protocol, CLI compatibility, conformance,
local-first verification/release tooling, and supported headless security
primitives. It must build and run without the Workbench repository, Node, React,
or Tauri. The PyPI name and fail-closed engine compatibility remain intact.

### RepoPact Workbench

Workbench owns the Tauri 2 shell, React/TypeScript frontend, desktop sessions,
watchers, UI and orchestration, mobile integration, application diagnostics,
installers, and application releases. It consumes exact, independently
identifiable core interfaces; it must not implement a competing validator,
graph authority, governance interpreter, or mutation engine. Python/pip is not a
runtime prerequisite. Offline/local operation remains supported; GitHub
authentication is optional and secret storage stays platform-native.

The dependency direction is unidirectional: Workbench consumes Core; Python
adopters, ForgeWire, and the Proving Ground consume the `repopact` distribution.
Core must not depend on Workbench.

## Architecture questions requiring evidence and approval

No choice below is approved merely by this proposal. The first architecture
slice must compare alternatives and record evidence-backed decisions:

1. **Release topology and packaging:** retain the PyPI `repopact` identity;
   define independently versioned Workbench releases and a source/artifact/API/
   protocol/schema/conformance compatibility manifest.
2. **Rust distribution:** evaluate a deliberately small set of stable published
   crates versus immutable full-SHA Git dependencies. Address API stability,
   ordering, reproducibility, offline builds, vendoring, Cargo locks, and
   provenance; do not publish every internal crate by default.
3. **Canonical interface:** narrow Workbench's internal domain dependencies
   behind stable typed Core interfaces while preserving the versioned,
   fail-closed engine handshake and defining a separate Workbench/Core
   compatibility contract.
4. **Optional/platform functionality:** map admission, protected guard,
   confinement, sandbox, remote providers, credentials, and Android/mobile
   integration to real consumers. Preserve the distinction between `pre-action`
   and `sandbox/process-enforced`; moving code is not security proof.
5. **Cross-repository governance:** define canonical ownership/migration for
   each work item, decision, evidence item, policy, schema, and frozen rule;
   preserve identifiers and provenance without duplicating active authorities.

## Migration sequence and authorization boundary

- **S0 — Preservation/preflight:** verify refs, backups, LFS/submodules,
  workflows/releases/hosted metadata, release identity, source provenance, and
  migration manifest before extracting files.
- **S1 — Dependency audit and architecture:** inventory complete Rust and
  Python dependency graphs, wheel/sdist/CLI, Tauri/Android entry points, tests,
  docs, schemas, ownership, risks, alternatives, compatibility matrix, and
  migration manifest. Draft ADR-A through ADR-E and obtain operator approval.
  Stop before repository creation or code separation.
- **S2 — Standalone Core:** prove clean standalone build/install, package and
  engine identity, CLI/conformance compatibility, and no Workbench dependency.
  Do not publish.
- **S3 — Standalone Workbench:** consume exact immutable Core dependencies;
  preserve desktop/mobile functionality, offline operation, secret boundaries,
  and honest platform evidence.
- **S4 — Consumer migration:** migrate the Proving Ground to explicitly pinned
  Core releases and consider ForgeWire only after its independent compatibility
  acceptance and separate authorization. Preserve research claims and history.
- **S5 — Release/publication:** separate authorization required. Only then may
  new repositories, access rules, releases, or publication be considered;
  Actions remain disabled or explicitly opt-in and cost-controlled.

## Non-goals and stop conditions

This work does not rewrite RepoPact governance, import all Proving Ground
experiments into Core, require Python for Tauri, replace Maturin without
evidence, automatically upgrade ForgeWire, introduce mandatory cloud services,
make GitHub Actions authoritative, or expand security claims beyond proof. It
does not resolve the personal-account GitHub restriction.

Stop before destructive or externally visible work if preservation is
incomplete, a required remote is unavailable, version drift is unexplained,
secrets are at risk, or an architecture/release decision lacks operator
approval. Never reset, force-push, delete original refs, create repositories,
publish packages, enable workflows, invoke paid services, or merge superseded
work as a shortcut.

## Acceptance and closeout

The binding 24-criterion checklist is in
[`work-item.json`](work-item.json); all criteria begin **pending**. The required
first-slice report, evidence boundaries, release and platform limitations, and
operator decisions are part of that specification. Criteria may be satisfied
only by immutable evidence recording actual commands, environment, revision,
scope, results, and limitations. Planning or registration alone does not satisfy
implementation criteria. Physical extraction requires a separately approved
consolidated architecture/migration plan.
