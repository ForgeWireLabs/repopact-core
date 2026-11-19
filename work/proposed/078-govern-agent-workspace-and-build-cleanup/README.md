# 078 — Govern agent workspace and build cleanup

**Status** Proposed

**Lead** Tooling owner. Review scopes are governance, work, evidence, and docs.

## Intent

Parallel agent runs create isolated Git worktrees, build intermediates, and
temporary output. A run can finish or stop unexpectedly while these resources
remain on disk. Give operators a way to see what is consuming space and give a
runner enough ownership and retention information to reclaim eligible resources
after ordinary closeout or a crash.

Start with a single Windows host and one representative ForgeWire Fabric
Cargo build workflow. Measure existing storage usage, the bytes actually
reclaimed, and build time before and after cleanup, including the cost of
rebuilding discarded intermediates.

## Design boundary

- RepoPact Core defines the work-item relationship, cleanup eligibility, and
  retention rules. Keep machine-local paths, run activity, and disk readings
  in host-local state. Link durable cleanup evidence from the work item as
  appropriate.
- The local runner registers the isolated worktree and dedicated Cargo target
  directory before use, tracks liveness, and applies the rules within configured
  roots. The Windows pilot uses a free-space floor on the destination volume and
  a single heavy-build slot among participating runs.
- RepoPact Workbench can later present the same inventory and cleanup reasons.
  Live memory-pressure scheduling and multi-host placement belong to a runner
  or a Fabric integration and can be evaluated after the pilot.

## Pilot workflow

1. Inventory worktrees and build directories under configured roots. Report
   sizes, known owners, eligible paths, and reasons for preserving other paths.
   Existing resources of unknown ownership remain report-only.
2. Attach a run ID, work-item ID, checkout commit, and approved resource paths
   to each managed run. Store retention for failed-run logs, evidence, pinned
   artifacts, and reproducible intermediates explicitly.
3. Reclaim eligible, expired output at closeout or on restart. Recheck identity,
   liveness, path containment, and current retention immediately before applying
   a cleanup plan. Keep dirty or locked worktrees, worktrees with unresolved
   activity or commits that lack a retained ref, and worktrees with ignored
   files outside registered disposable paths. Use Git's normal worktree
   removal. Preserve branch refs, global caches, user-managed paths, and evidence
   and artifacts covered by retention.
4. Report each action or reason for deferral and the number of bytes actually
   reclaimed. Keep failed operations retryable, including Windows sharing
   violations, and recover from interruption during cleanup.

The machine-readable acceptance criteria are in
[work-item.json](work-item.json). Link pilot measurements and failure-mode
results before completing the item.
