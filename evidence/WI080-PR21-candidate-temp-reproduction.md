# WI080 PR #21 — Candidate temporary-directory pre-fix reproduction

**Recorded:** 2026-11-24T03:39:29Z

**Pre-fix source revision:** `94924281c929e7a4ec5da42c42638624c6259725` (PR #20 base)

**Environment:** Windows 10.0.19045.0, Rust/Cargo 1.96.0
**Run record:** [`20261124-080-pr21-candidate-temp-repro.json`](runs/20261124-080-pr21-candidate-temp-repro.json)

## Controlled reproduction

A detached disposable worktree at the pre-fix revision received a temporary test-only probe in `rust/crates/repopact-mutation/src/lib.rs`. Its scratch directory and Cargo target were under that disposable checkout. No user-owned temporary directory was accessed. The temporary source change was restored after the run.

The probe used the old timestamp-only candidate path format with a fixed test stamp and repeated the old `fs::create_dir_all` operation twice. The second call succeeded on the same nonempty directory, and the first invocation's marker remained there. It then copied the valid fixture into that shared candidate, opened a repository snapshot, removed the shared path to model the other invocation's cleanup, and ran the actual `repopact_validation::validate_snapshot` check against the already-open snapshot.

Command:

```text
cargo test --manifest-path rust/Cargo.toml -p repopact-mutation tests::timestamp_only_candidate_reuse_and_cleanup_can_report_scope_missing -- --exact --nocapture
```

Observed result:

```text
running 1 test
test tests::timestamp_only_candidate_reuse_and_cleanup_can_report_scope_missing ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 31 filtered out
```

The assertion observed diagnostic code `audit-registry.scope-missing` with message `audit scope does not exist: .` after the modeled cleanup. This confirms that the old shared-path reuse and cleanup interleaving can produce the diagnostic. The test used a fixed stamp to make path reuse deterministic; it did not wait for a real clock collision or call the entire old `validate_candidate()` function concurrently.

## Causal limit

This reproduction establishes a concrete source-level failure mechanism for the `audit-registry.scope-missing` diagnostic. It does **not** establish that this mechanism caused the historical `consumed_work_record_drift_is_stale` failure. That exact historical failure was not reproduced by this probe, so WI080 AC-6 and AC-10 remain pending and WI080 remains blocked.
