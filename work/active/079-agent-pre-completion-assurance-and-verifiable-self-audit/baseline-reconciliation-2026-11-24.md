# WI079 — reconciled baseline candidate (2026-11-24 CST / 2026-11-25 UTC)

**Status:** source-only GitHub integration staging. **NOT VALIDATED** and **NOT ready to merge**. Original WI079 [draft PR #18](https://github.com/ForgeWireLabs/repopact-core/pull/18) remains untouched at `0562293d1f4a789055f6035204a60550ff29fd9e`.

## Exact inputs

- Accepted Core main: `efa051525cf3c7c4621b695044dbeda38bf3530b`, tree `aa3998d4d858754ecc89de6fc86fbf210cf40cb4`, including merged WI080 fixture/plan isolation, provenance/fmt fixes, WI081 local CI and completion.
- Historic WI079 PR #18 head: `0562293d1f4a789055f6035204a60550ff29fd9e`; merge base `0ac343a2bf25f56a47e8f42798eca938844199fa`.
- GitHub compared the two branches from their common merge base. Exactly six paths had changes on **both**: dashboard, three semantic manifest/identity paths, documentation mappings and research metadata. The other 26 WI079 file changes were copied into a new branch over accepted main using their **unchanged original Git blob SHAs**, preserving all twelve preexisting historical evidence records as they were (including failed CI reports).

## Scoped reconciliations already made

- `research/metadata.json`: added WI079's fifteenth prospective protocol to the existing 14-document list. Retained `verified_on=2026-11-23`, `review_by=2026-12-23` and adopter visibility review date `2026-11-23` from WI080 main. The combined aggregate date is conservative; the new protocol was independently inspected afterward without falsifying the review of the original fourteen documents.
- `research/wi079-self-audit-benchmark-protocol.md` explicitly says **PLANNED / NOT RUN**. Static review `20261125-079-protocol-static-review` covers its three arms, twelve planned task fixtures, H0/H1/H2 and limits; no task manifest is frozen, no model run is authorized, and WI079 AC-9 remains **pending**.
- WI079 source, tests, template, guide, docs, work item and historical evidence were preserved from PR #18, without rewriting or claiming the original CI failures as passes.
- The five generated conflict surfaces were intentionally kept at accepted-main values during this staging operation. They are now **stale for the combined candidate** until the canonical generator, documentation review/snapshot tooling and dashboard are run. They must not be manually picked from either branch.

## Local validation and stop conditions

1. Verify exact staged branch HEAD, read every applicable `AGENTS.md`, and inspect the final Git diff versus accepted main. Confirm no WI080 or WI081 code was reverted and no old research review date was restored.
2. Inspect the protocol, 15-document metadata and audit `docs` scope notes. Review mappings `documentation-freshness-tooling` and `engine-semantic-compatibility` against actual merged source and docs. Do not manufacture a docs freshness pass by changing snapshot hashes without review.
3. Run `scripts/generate_engine_semantic_identity.py --update`, canonical documentation mapping snapshots, then canonical dashboard generation, with a source-matching Rust engine. Preserve all unrelated prior immutable evidence.
4. Run focused `tests/test_self_audit.py` including CLI smoke and structural negative cases, default/external Cargo target environment checks as applicable, `cargo fmt --all -- --check`, full Python suite, Rust workspace, conformance, `repopact validate`, frozen-surface verification, semantic identity and full local `repopact verify ci` from a **clean exact-source** checkout. Record source SHA/tree before generated evidence commits.
5. This is experimental `self-report-current` with `assurance=instruction-only`, not an independent review, actual agent reasoning proof, binding Rust completion policy, H14 effective gate, research superiority result or release clearance. **Do not run the planned comparative model experiment.**
6. Preserve any failing profile and new production defect; do not waive because a failure was inherited. Keep WI080 `blocked` AC-6/AC-10 with historical causality unresolved. CI local-only through 2026-12-01 inclusive.
7. Push validated changes only to this **new draft reconciliation PR**. Do not force-push, merge, rebase or close the historical PR #18 until a separate exact-head integration decision.

## Why this approach

The historical PR #18 branch was 46 commits ahead and 45 commits behind the now-repaired main. The durable evidence in #18 is valid historical context but its semantic identity and expired research dates are not valid integration inputs. A new source-level candidate avoids overwriting that branch and makes the provenance and final acceptance of the merged prototype independently reviewable.

## Subsequent scoped reliability repair (pending host validation)

The independent source review noticed that historical `repopact/self_audit.py::_git` passed all inherited `GIT_*` environment overrides into subprocesses even though the helper explicitly accepts a repository root and base. That allows an enclosing agent/CI shell to redirect the Git directory/index/worktree, causing false failure or an incorrect candidate identity. This is the same general class of inherited-Git-environment interference observed in WI079's historical tests, not a demonstrated explanation of WI080's Rust intermittency.

The staged candidate now filters inherited `GIT_*` values and re-enables only controlled `GIT_TERMINAL_PROMPT=0` and `GIT_OPTIONAL_LOCKS=0`; the test fixture's Git invocation uses the same isolation. A new `test_inherited_git_overrides_cannot_redirect_candidate` injects contradictory `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE` and `GIT_CONFIG_COUNT` selectors and asserts that the snapshot and fresh report check still target the intended repository. The focused suite has **32 test definitions**, including this **UNRUN new regression**. The original PR #18 implementation and 31-test evidence are preserved at their historical SHA; they do not validate the new change. Run the new negative regression and exact-head local CI before any integration.
