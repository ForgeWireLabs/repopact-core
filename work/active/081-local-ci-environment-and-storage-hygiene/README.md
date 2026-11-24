# WI081 — Local CI Environment Sensitivity and Validation Storage Hygiene

**Status:** active, not completed. **Owner:** tooling-owner, with governance/evidence/work/docs as affected scopes.

## Context

During WI080 PR #21 Windows validation (2026-11-24), the first full local CI profile failed in Python synthetic Cargo metadata tests when the executor inherited a non-default `CARGO_TARGET_DIR`. A separate output-capture attempt was interrupted by disk exhaustion. Later exact-head canonical CI passed after the environment and storage conditions were corrected, **but those earlier failures remain real and are not waived by the later success**.

Reference: [PR #21](https://github.com/ForgeWireLabs/repopact-core/pull/21) and immutable `evidence/runs/20261124-080-pr21-local-ci.json` (FAILED) and `20261124-080-pr21-local-ci-clean.json` (PASSED). Full raw interrupted output is local to the host and must not be invented as a complete run. WI080 remains separately `blocked` on an unproven historical Rust test root cause; WI081 does **not** close it.

## Repair scope

**A. Cargo target environment.** Identify the exact Python synthetic Cargo metadata fixtures/tests affected by inherited `CARGO_TARGET_DIR`. Establish a failing focused reproduction before changing code. Repair the relevant setup or verifier without hiding the environment from real Cargo operations or weakening assertions. Test with both default and external target roots and capture source/environment identity without secrets.

**B. Build and worktree storage.** Measure actual growth. Inventory ownership for build dirs, temporary test fixtures, evidence and local worktrees. Design safe, explicit reclamation for exclusively tool-owned disposable artifacts with dry-run previews, ownership and dirty checks. Never remove an unknown path, user worktree, original checkout, immutable evidence, or retained failure reproduction. Do not use broad `git clean`, recursive host temp deletion, or silent overwrite. Preserve needed compiled dependencies when rebuild cost is excessive.

**C. Failure capture.** Low-space or interrupted output collection must produce an explicit incomplete/failed outcome and preserve logs and metadata already written. Keep file output bounded and do not generate duplicate large evidence by default. No partial or unverified run gets `PASS`.

**D. Canonical local verification.** Run focused negatives and positives followed by Python suite, locked Rust workspace, matching-source conformance, canonical `repopact verify ci`, `repopact validate`, semantic identity check, documentation mappings, frozen-surface and `git diff --check`. Preserve both original and new evidence and clearly identify exact tested source SHA/tree.

## Boundaries

Local CI mandatory through December 1, 2026 inclusive; no automatic hosted enablement, no release, no publication. WI079 draft PR #18 requires a separate conflict-aware rebase against newly merged `main`; avoid touching that branch as part of WI081. No claim that historical WI080 intermittency was explained by a newly identified environmental or storage issue.

## Completion

All eight acceptance criteria in `work-item.json` require linked concrete evidence, review and operator decision. Do not mark completed based on work-item registration or a subsequent passing rerun.
