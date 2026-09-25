# Next bounded WI074 implementation slice

## Slice: close Core governance continuity and source preservation

Start from the canonical Core `main` after this governance transfer is
published. Work only in a fresh branch/worktree and preserve existing source
and backup material.

1. Independently inventory all source backup bundles, refs, tags and reachable
   objects; hash each backup and record limitations. Reconcile the immutable
   S2 source map (1,048 entries) against the Core and Workbench extraction
   manifests, reporting missing, duplicate and deliberately archived paths.
2. Reconcile inherited decisions, evidence, licenses and frozen-surface rules
   against the accepted S2 owner mapping. Do not import unrelated ledger
   history, raw secret-scan output, private backups, or excluded signer tools.
3. Run canonical Core validation, schema/work-item reference checks, dashboard
   generation, and frozen-surface validation. Attach actual run manifests and
   source/destination hashes; reconcile RPS-001, RPS-002 and RPS-017 only when
   each criterion's complete wording is proved.
4. Leave all other acceptance criteria in their current state unless new
   criterion-specific evidence justifies a change. In particular, do not infer
   Android feature acceptance from startup, cross-platform support from a
   Windows/Linux build, or rollback/release readiness from publication.

After that slice, order independent work by dependency: Core packaging/CLI and
3.1.3 conformance (RPS-004/005/006/014); admission/security (RPS-012); then
Workbench runtime, parity, authentication, and platform acceptance
(RPS-009/010/011/013). Reproducibility, release/rollback, and documentation
gates follow (RPS-015/016/020/022). Proving Ground and ForgeWire criteria
(RPS-018/019) remain separate consumer migrations requiring their own
authorization and compatibility evidence. No binaries, PyPI releases, hosted
Actions, or consumer changes are included in this directive.
