# 074 — Extract RepoPact Core and Independent Workbench

> **Status:** Active (canonical ledger transferred to ForgeWireLabs/repopact-core on 2026-09-25)
> **Owner:** tooling
> **Lead:** tooling-owner
> **Affected scopes:** tooling, governance, docs, evidence
> **Created:** 2026-09-24
> **Provenance:** Concrete; original preflight and source history retained.

## Intent and canonical ownership

WI074 separates RepoPact Core from the Tauri desktop/mobile Workbench while
preserving one canonical governance authority. Core retains the `repopact`
distribution and CLI, canonical Rust semantic engine, schemas, validation,
graph, analysis, mutation, conformance, and supported headless admission
functionality. Workbench owns its Tauri application, UI, orchestration, native
integration, installers, and releases. Workbench has its own future ledger and
does not hold a duplicate WI074 record.

By the operator's 2026-09-25 authorization, ForgeWireLabs/repopact-core is the
canonical owner of ongoing WI074 work. This transfer preserves the accepted decisions, original
event timestamps, S2-S5 provenance and prior acceptance states; it does not
rewrite any historical checkout or commit. Decision 0075 records the ownership
disposition; the transfer manifest records source commits, paths and content
hashes for each imported record. The prior S5 lineage remains immutable
historical evidence.

## Authorization and chronology

The operator approved ADR-A through ADR-E and the consolidated architecture
plan at `2026-09-24T21:31:54Z`. The original proposal preflight remains
`2026-09-24T19:12:30Z`. S2 Core extraction preceded that approval and is not
represented as retroactively authorized. Decision 0073 preserves the original
chronology and is superseded by Decision 0074, which records acceptance of the
process deviation at `2026-09-25T02:39:02Z` and its corrective measures. The
transfer does not backdate activation or alter either record. See the
[approval evidence](../../../evidence/runs/20260924-213154-074-architecture-approval.json),
[Decision 0073](../../../decisions/0073-wi074-s2-pre-activation-chronology.md),
[Decision 0074](../../../decisions/0074-wi074-s2-pre-activation-deviation-disposition.md),
and [Decision 0075](../../../decisions/0075-wi074-canonical-ledger-transfer.md).

## Source and publication provenance

The immutable integrated source is `8f1ce8deb139287655afcc8479dc69dd621d8720`.
S2 Core is `6aff2c376efb5ddf236bd11cd1d700f873ec6a4f`; its 1,048-row source
manifest SHA-256 is
`8139aa2d3abed79bb8e00d5c6c94c8510cfc23b82402f2d82965d36985c76d59`.
S3 Workbench is `5678bdb1d04f8582fcf5ddc5b3fc8cb884199342`; its manifest
SHA-256 is
`2c02ea2e18c64ac489d21d4f4cf15b1f66f2d3c336c661514a513a750bc37150`.
S4 readiness is recorded at `eceba879952e830ba596ff13d410684e3ffb2afb`.
The source-only S5 publication closeout is from
`3b699ab7e1b23812813f878c85dc9baaead47997`.

The published Core baseline is
`e15fbcfa4a29e1e6df7339bfed56c15c19185f9a`; the published Workbench baseline
is `a1876b08b78f169a79597ec3099d9840ace55e06`. At transfer verification,
Actions were disabled in both repositories and each reported zero workflow
runs. S5 recorded a passing Linux Secret Service focused credential test and
full Workbench Rust suite (243 passed, 10 ignored, 0 failed), plus a fresh
public Workbench clone build resolving all eight Core crates to the single
published Core SHA. Those historical tests were not rerun as part of this
governance transfer. Android evidence remains APK identity/emulator-startup
only, not feature acceptance. Source-only publication passed its license
review; third-party notices remain a prerequisite for executable or installer
distribution. Details and provenance hashes are in the
[transfer summary](../../../evidence/WI074-transfer/20260925-s2-s5-summary.md)
and [manifest](../../../evidence/WI074-transfer/20260925-transfer-manifest.json).

## Approved architecture and boundaries

The consolidated plan and accepted ADR-A through ADR-E are retained under
their original identifiers in `decisions/0067` through `decisions/0072`.
Core remains the sole authority for governance semantics. Workbench consumes
Core; it must not implement a competing validator, graph authority, governance
interpreter, or mutation engine. The dependency direction is unidirectional.
Core and Workbench keep separate versions and platform evidence. Stable RepoPact
3.1.3 identity/artifacts remain unchanged; no PyPI publication or consumer
migration is part of this transfer.

Keep local-first validation authoritative. GitHub Actions remain disabled.
This task does not publish binaries, migrate ForgeWire or Proving Ground,
change the Workbench Core pin, alter PyPI, or begin unrelated acceptance work.

## Current acceptance state and next slice

The canonical 24-criterion matrix is in [work-item.json](work-item.json). Its
states are transferred without inflation: **5 satisfied, 19 pending**. In
particular, S2-S5 publication does not imply full Android feature acceptance,
cross-platform native acceptance, release readiness, downstream migration, or
rollback proof. Evidence references for satisfied criteria point to immutable
run IDs imported with provenance and portable artifact links.

The [next-slice directive](../../../evidence/WI074-transfer/next-slice.md)
defines the next bounded work: complete the Core-side governance-continuity
manifest and acceptance mapping, then pursue independently executable
platform/feature gates in dependency order. It is planning only; implementation
outside this authorized governance transfer requires the applicable work-item
authority.
