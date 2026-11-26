# WI083 local throughput measurements

Measured on 2026-11-25. These are local observations for this candidate, not a
productivity, effectiveness, or causal claim.

## Tutorial command counts

Compared the tutorial at baseline `b9dec03` with the candidate tutorial in
`docs/adopt-repopact.md`. Counting only explicit verification commands (and
excluding install, initialization, work-item creation, dashboard generation,
and user edits), the baseline path called `repopact validate` five times and
`repopact verify` twice: seven verification invocations. The candidate calls
`repopact validate` three times and `repopact verify` once: four invocations.
Initialization already validates its seeded repository, so its standalone
post-init validation was redundant. The first verification run now records
evidence rather than running an unrecorded pass and repeating it for evidence.

No end-to-end tutorial elapsed time was captured. That path includes user edits
and implementation work whose duration was not controlled. AC-1 remains
pending until a matched elapsed-time observation is available.

## Verification contract handling

The source-level baseline is `repopact/verification.py` at `b9dec03`; the
candidate is the current checkout. The baseline recorded path loads, parses, and
schema-validates the contract twice. The candidate does that once, then hashes
the config and schema bytes once before execution to preserve drift detection.

Reproduction command:

```powershell
.venv\Scripts\python.exe evidence\wi083-contract-load-benchmark.py --baseline b9dec03 --warmups 10 --samples 5 --iterations 250
```

Environment: Python 3.12.4, Windows 10 build 19045. The verification config
SHA-256 was
`f75cda4df4a50a57a8d030f6c8180b81e425fd3fc4944a5c1445ca26ec5bfb1d`; the
schema SHA-256 was
`22fe35615d69a1ea31e46176e259faf7c9f42febb9992d2eb532fbbebdbd62a3`.
The five-sample median was 12.1296 ms per baseline operation and 6.6822 ms per
candidate operation, a 5.4474 ms or 44.91% reduction in this contract-handling
microbenchmark. It excludes profile step execution, evidence writes, dashboard
refresh, and the rest of the CLI. It is not an end-to-end speedup claim.

## Rust mutation candidate path

The clean baseline fixture was detached at
`b9dec03ad59c2ef7d782eda9f2b3220747e8ba3c` in
`C:\Projects\repopact-core-wi083-rust-baseline-20261125`. The clean source
inventory was 1,130 files / 19,149,551 bytes. The temporary measurement test
added 2,092 bytes without changing the file count. Toolchain: Cargo 1.96.0 and
rustc 1.96.0 on Windows; OS build, CPU, and load conditions were not captured.

Command from the detached baseline fixture:

```powershell
$env:CARGO_TARGET_DIR='C:\Projects\repopact-core-wi083-rust-target-20261125'; cargo test --locked --manifest-path rust\Cargo.toml -p repopact-mutation measure_wi083_baseline_candidate_copy -- --nocapture
```

Captured output:

```text
baseline_candidate_copy files=1130 bytes=19151643 copy_ms=2137
baseline_validate_candidate ms=9167 dashboard_bytes=1636 diagnostics=0 blocking=0
test result: ok. 1 passed; 0 failed; 32 filtered out
```

`copy_ms=2137` timed only `copy_tree(root, temp)`, not a complete mutation.
`validate_candidate` timed its copy, empty-operation application, dashboard
render/write, full validation, and cleanup. Both numbers include the temporary
2,092-byte probe. They are instrumented probe timings, not uninstrumented
production timings. No safe copy reduction was identified: doing that requires
an overlay that preserves complete diagnostics, Git/worktree facts, stale
read-set and preimage checks, frozen-surface checks, rollback, cleanup, and
independent live post-state validation. No Rust source change was made.

## Validation observations

The focused Python regressions and bootstrap test passed (30 tests). An initial
canonical validation after refreshing semantic identity failed because the
`engine-semantic-compatibility` documentation mapping still held its prior
source digest. The mapped guide was reviewed; the canonical snapshot returned
source digest
`838e8e78b56706010965a75857a49cfe26baabf5edf3d723bc808f2a5fd3657a` and the
same documentation digest, so only the source snapshot was refreshed. The next
canonical validation passed. Final full-profile and legacy-comparator results
are recorded in their own evidence runs.
