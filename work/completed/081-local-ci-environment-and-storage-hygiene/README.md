# WI081 — Local CI Environment Sensitivity and Validation Storage Hygiene

**Status:** completed in PR #23 candidate; canonical completion validation pending before merge. **Owner:** tooling-owner, with governance/evidence/work/docs as affected scopes.

## Context

During WI080 PR #21 Windows validation (2026-11-24), the first full local CI profile failed in Python synthetic Cargo metadata tests when the executor inherited a non-default `CARGO_TARGET_DIR`. A separate output-capture attempt was interrupted by disk exhaustion. Later exact-head canonical CI passed after the environment and storage conditions were corrected, **but those earlier failures remain real and are not waived by the later success**.

Reference: [PR #21](https://github.com/ForgeWireLabs/repopact-core/pull/21) and immutable `evidence/runs/20261124-080-pr21-local-ci.json` (FAILED) and `20261124-080-pr21-local-ci-clean.json` (PASSED). Full raw interrupted output is local to the host and must not be invented as a complete run. WI080 remains separately `blocked` on an unproven historical Rust test root cause; WI081 does **not** close it.

## Repair scope

**A. Cargo target environment.** Identify the exact Python synthetic Cargo metadata fixtures/tests affected by inherited `CARGO_TARGET_DIR`. Establish a failing focused reproduction before changing code. Repair the relevant setup or verifier without hiding the environment from real Cargo operations or weakening assertions. Test with both default and external target roots and capture source/environment identity without secrets.

**B. Build and worktree storage.** Measure actual growth. Inventory ownership for build dirs, temporary test fixtures, evidence and local worktrees. Design safe, explicit reclamation for exclusively tool-owned disposable artifacts with dry-run previews, ownership and dirty checks. Never remove an unknown path, user worktree, original checkout, immutable evidence, or retained failure reproduction. Do not use broad `git clean`, recursive host temp deletion, or silent overwrite. Preserve needed compiled dependencies when rebuild cost is excessive.

**C. Failure capture.** Low-space or interrupted output collection must produce an explicit incomplete/failed outcome and preserve logs and metadata already written. Keep file output bounded and do not generate duplicate large evidence by default. No partial or unverified run gets `PASS`.

**D. Canonical local verification.** Run focused negatives and positives followed by Python suite, locked Rust workspace, matching-source conformance, canonical `repopact verify ci`, `repopact validate`, semantic identity check, documentation mappings, frozen-surface and `git diff --check`. Preserve both original and new evidence and clearly identify exact tested source SHA/tree.

## Local storage tooling

`python scripts/local_validation_storage.py --root . inventory` measures the
known Cargo targets, Python build/venv directories, immutable evidence and Git
registered worktrees with bounded traversal. `preflight` checks the repository
and active Cargo target volumes before CI; the default minimum is 12 GiB free
per volume. The command records no file contents beyond exact hashes when a
new tool-owned Cargo target is explicitly sealed.

Cleanup commands are dry-run by default. `create-target` and `create-worktree`
only register paths they create themselves in a repository-common-dir
ownership manifest under `.git`. `seal-target` captures a bounded exact file
inventory. `cleanup-target` requires that inventory to remain exact;
`cleanup-worktree` requires the registered detached commit, a clean checkout,
no ignored/untracked user files, and a bounded size. Pass `--apply` only for a
path shown by the ownership registry and a successful dry-run. Unknown paths,
symlinks/junctions, dirty checkouts, changed build outputs and all paths under
`evidence/` are refused. No automatic deletion or host-wide temporary cleanup
is performed.

Evidence-backed `repopact verify ci` writes a fixed-size in-progress marker
before starting and bounded completed-step tails as the profile runs. An
interruption, storage failure, short output write or failed evidence write
returns exit code 2 and leaves status incomplete. Immutable prior run records
are not rewritten.

The active evidence ID is passed only to that invocation's child processes.
Fixture materialization excludes only its validated state/log marker pair, and
canonical conformance provenance reports that pair separately from source
changes. Stale or malformed markers and unrelated untracked files remain part
of the dirty-source result. This prevents an evidence-backed CI invocation
from copying its own journal into test fixtures or making its exact source
revision appear dirty.

## Boundaries

Local CI mandatory through December 1, 2026 inclusive; no automatic hosted enablement, no release, no publication. WI079 draft PR #18 requires a separate conflict-aware rebase against newly merged `main`; avoid touching that branch as part of WI081. No claim that historical WI080 intermittency was explained by a newly identified environmental or storage issue.

## Completion

AC-1 through AC-8 have linked concrete evidence. The operator accepted RepoPact Core-only cleanup integration in independent PR #22 review 6049772262; Workbench/Fabric ownership remains separate. Completion in this PR still requires canonical exact-head verification and merge. The earlier failed CI and WI080 historical Rust uncertainty remain unresolved.

## Independent integration review — 2026-11-24

PR #22 was reviewed at head `2473c5611a4a59ea2336f1c7a8ed1e6a23aaf711` (tree `60647f7aa6203e68f582c59cb2dc4071f94d6222`) and merged as `5d185faac83b68596715ce99824dd4b717eb104e`. The actual reviewed source was tested in clean exact-source default and controlled external-target local CI profiles. Independent GitHub review: https://github.com/ForgeWireLabs/repopact-core/pull/22#pullrequestreview-6049772262. Durable AC-8 evidence: `evidence/runs/20261124-081-ac8-core-only-integration-review.json`.

**Operator integration boundary:** RepoPact Core accepts ownership-registered, tool-created disposable targets and detached worktrees with dry-run-first cleanup. No Workbench/Fabric adoption, no automatic reclamation of pre-existing builds, and no cleanup of user workspaces, native Fabric installations, retained state, or externally owned worktrees. Workbench retains its own process/artifact authority; Fabric's native uninstall/export and repository-separation authority stays governed by Fabric issues #30 and #56. Any future integration requires its own work item and physical/product acceptance.

AC-8 is documented and linked; all eight criteria are satisfied. The complete five-file WI081 directory is staged under `work/completed/` with only this README, work-item status, and validation addendum modified. Canonical dashboard regeneration, complete exact-source validation, and independent final review are still required before merging PR #23. This completion does not explain WI080's historical Rust intermittency, enable hosted CI, or authorize a release. Local-only CI remains binding through 2026-12-01 inclusive.
