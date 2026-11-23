# WI080 — Local repair, verification and stop conditions

**State:** implementation started, validation pending. This file is a handoff, not evidence that checks passed.

## Execution authority

Fix all three inherited failure classes; do not stop at proving they reproduce on `main`. Use a clean isolated Windows worktree and record the exact remote PR #19 head before starting. Preserve the original `C:\Projects\repopact-wi079-local` and other worktrees; do not reset, clean, or modify them. Keep CI entirely local through 2026-12-01 inclusive and do not enable hosted CI/CD.

## Lane A — WI076 conformance writer

1. Inspect `repopact/run_conformance.py` and `tests/test_conformance_provenance_identity.py`. The draft changes derive default filename and record `id` from `recorded_at` once; explicit output filenames remain caller-chosen and all outputs are create-only.
2. Run `python -m unittest discover -s tests -p test_conformance_provenance_identity.py -v`.
3. Compile the touched Python modules and run existing provenance/conformance tests against a matching Rust engine built from the exact source revision.
4. Verify a real default-generated conformance record has `Path.stem == record['id']`, matching timestamp, schema validity and complete provenance; a second write to the same path must not overwrite evidence. Preserve historical mismatch records untouched.
5. Repair any integration/compatibility regression; do not suppress the mismatch by excluding checks.

## Lane B — real research freshness review

1. Read `governance/policies/002-semantic-claim-freshness.md`, `research/metadata.json` and **each of the 14 main-branch registered research documents**. Build a review matrix: path, claims, supporting evidence/run/source, currentness, discrepancies, required edits, verified-by, review outcome.
2. Reexecute permitted source checks and inspect contradictory/current claims, especially comparison studies, H14/H15, submission/release and dated status claims. Mark unsupported or untested claims explicitly rather than inferring proof.
3. Make necessary source corrections. Create real dated review evidence and link to the matrix and actual commands. Only after substantive review may `verified_on` and `review_by` be advanced; the window must obey the 30-day limit.
4. When reconciling WI079 later, additionally review its *fifteenth* draft research document. Do not silently extend a previous 14-document certification to the newly added document.

## Lane C — intermittent Rust mutation test

1. Reproduce `repopact-mutation::tests::consumed_work_record_drift_is_stale` using pinned source, controlled Git environment and actual failing CI profile conditions.
2. Repeat filtered and package runs with full `--nocapture` diagnostics, then run the workspace both serial and default-parallel. Compare clean and inherited Git configurations; record process IDs, fixture roots, directory cleanup, timing, disk contention and failure diagnostics where possible.
3. If test flakiness reproduces, identify and minimally repair its cause. Add a regression that would fail on the old behavior, not merely broader retries or larger sleeps. Do not skip or weaken assertions.
4. If it does not reproduce under a bounded campaign, record precisely what was tried and why the cause remains unresolved. Do not claim the failure fixed merely from passing reruns; preserve the blocked remediation gate.

## Closeout checks on one exact-head snapshot

- Regenerate semantic identity and dashboard using canonical generators only; review the resulting diff.
- Run `repopact validate`, `repopact verify ci`, full Python suite, real engine conformance, `cargo test --workspace --locked --manifest-path rust/Cargo.toml`, `git diff --check`, frozen-surface and documentation impact checks.
- Preserve failed and previous evidence; attach new exact-head evidence to relevant WI080 AC IDs without fabricating satisfied states.
- Commit and push only scoped changes; leave PR #19 draft until an actually green baseline and required governance review. Do not merge PR #18 or publish software.
- Report exact full SHA/tree, test counts, environment, changed paths, latest CI outcome, remaining failure hypotheses and review matrix. Reconcile WI079 after baseline repairs; never declare inherited errors permanently acceptable because WI079 did not cause them.

## Failure dispositions

`PRE-EXISTING` is a **cause label**, not a final state. Final disposition must be `FIXED` with regression and complete evidence, or `BLOCKED` with an accountable open work item, explicit owner and a concrete next diagnostic. A green status must represent checks actually run against the same source and environment.
