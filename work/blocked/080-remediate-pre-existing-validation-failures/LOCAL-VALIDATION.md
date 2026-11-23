# WI080 — Local repair, verification and stop conditions

**State:** blocked after validation. The handoff below records the required lanes; the dated execution record at the end links the actual outcomes. The intermittent Rust root cause remains unresolved.

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


## Latest execution record — 2026-11-23

- Isolated worktree: `C:\Projects\repopact-wi080-local`; original WI079 worktree preserved and read-only during this work.
- Exact local CI source snapshot: commit `7f007d75fb393f6d68f22b96ecaabd78bccee9ce`, tree `c08acfb4bb6b368c4de134aed501a1f2cb03c1f0`, clean before the profile. Canonical report: `evidence/runs/20261123-182000-636282-verify-ci.json`; profile exit 0 in 1663.734 seconds. Five required steps passed, optional frozen-surface report passed. Python discovery counted 454 cases; profile step exit 0. The report's `dirty=true` is after-run state from conformance evidence and dashboard generation; the only changed tracked file was `audits/reports/dashboard.md`, with the conformance and verify-ci reports newly created.
- Environment: Windows x86_64, Python 3.12.4, Cargo/rustc 1.96.0. The conformance engine was built from that exact source revision; engine SHA-256 `776a5fa3bc1c67a55229379c6b02197ab203b809f439b211c09c941ea96407dc`, semantic identity `repopact-semantic-v1:sha256:4c6581205fea24be2d56a88f6efa5da00b226a783ef407a665203d0d10f2f157`. All inherited `GIT_*` variables were removed from the profile process; `REPOPACT_GITHUB_CI` and `REPOPACT_GITHUB_CD` were unset.
- WI079 fifteenth-document review: `20261123-080-wi079-protocol-review`. The new candidate protocol is still unregistered, planned/not run, and excluded from the 14-document verification-date update; before a separately authorized run, freeze fixture hashes, model/toolchain pins, adjudication rubric, and telemetry in its manifest.
- Conformance report: `evidence/runs/20261123T181925821732Z-wi076-ac6-engine-conformance.json`; 43/43 canonical and 8/8 WI050 passed, provenance complete, source revision matches engine revision, generated filename and embedded ID match.
- Rust controlled campaign: 80 focused passes across sanitized/inherited and serial/parallel modes; three separate full workspace runs passed 397/397 each. The canonical CI Rust workspace step also exited 0. The original failure did not recur, so its cause remains unresolved; AC-6 remains pending and WI080 is blocked under `tooling-owner` pending the diagnostic plan above and independent operator acceptance.
- Research and provenance evidence: `20261123-080-research-freshness-review`, `20261123-080-wi076-provenance-validation`, `20261123-080-rust-drift-diagnostic-campaign`.
- Hosted CI/CD remained disabled. PR #19 and PR #18 remain drafts; neither was merged and no release was published.
