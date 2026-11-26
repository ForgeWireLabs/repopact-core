# WI083 AC-7 paired optimization experiment

Date: 2026-11-26. Base source: `559ed9a86a1c399826f9505e5412d11c54d099ab` (tree `8be9c62cda36506623e828a267538e5d62849195`). The controlled fixture used that exact HEAD plus only the two prospective Rust source files. The baseline and candidate ran against the same unchanged 1,220-file / 19,624,629-byte fixture; the sorted per-file length manifest matched byte-for-byte. Candidate source hashes were repository `21C614B3AEEEF6F2462FA125C260A2CBA0AA1FE3D0D433A422BF0ADBDDFDB3F9` and mutation `444059C5D78DCFCEBCF66A4E770BB52C6F559BD6C610D25C66219AF8DE3226B2`. The candidate source patch and exact source snapshots are retained beside the raw logs.

## Protocol and environment

Each algorithm ran two warmups followed by ten measured samples for each of three representative plan-only cases: active-item create, WI083 edit, and active-to-blocked transition. No operation was applied. Each of the 30 plans per algorithm was applicable and reported zero diagnostics and zero blockers. Timings use test-only instrumentation in Cargo's unoptimized test profile on Windows 10 Pro build 19045 x64, Intel Core i7-6820HQ (4 cores / 8 threads), Rust/Cargo 1.96.0. The baseline and candidate harness commands, source overlays, fixture hashes, and raw output are retained in this directory. The historical clean-baseline and tutorial replay files are exploratory/partial and are excluded from the gate calculation.

The implementation under evaluation opened one candidate snapshot after the outer planning snapshot (2 logical snapshots per plan, down from 3). Repository copying remained enabled. Production mutation still independently validates live post-state; the experiment did not apply plans or substitute snapshot validation for that boundary. Peak process working set over each benchmark process was 96,481,280 bytes baseline and 96,575,488 bytes candidate.

## Results

| Case | Baseline median / p95 | Candidate median / p95 | Median change |
|---|---:|---:|---:|
| Active create | 8,543.164 / 8,686.662 ms | 10,132.901 / 12,301.085 ms | 18.608% slower |
| WI083 edit | 9,549.364 / 12,958.396 ms | 9,291.299 / 10,610.265 ms | 2.702% faster |
| Active to blocked | 11,353.594 / 13,001.532 ms | 8,234.639 / 9,424.424 ms | 27.471% faster |
| Pooled 30 measured samples | 9,549.364 / 12,958.396 ms | 9,140.374 / 11,344.890 ms | 4.283% faster |

The pre-registered gate required at least 15% pooled median improvement and no representative p95 regression greater than 5%. The candidate missed the pooled threshold and active-create p95 regressed 41.5%. Therefore the optimization was rejected and both production Rust source files were restored exactly to the base revision. No prototype runtime behavior is shipped. The disparate per-case results and all samples/outliers are retained; none were excluded.

## Equivalence and regression evidence

Focused prototype tests passed: repository dashboard refresh equals a fresh snapshot (1/1), and single-snapshot candidate validation matches the legacy dashboard/ordered diagnostics across stale or missing dashboard, malformed evidence, nested repository and linked-worktree markers, symlink where supported, and injected I/O failure (1/1). The symlink case ran successfully on this host. Raw Cargo output is `focused-prototype-regressions.txt`. These tests establish the bounded validation equivalence checked by the prototype; they do not claim an applied mutation or live post-state test. The final full local CI profile and legacy comparator are run against the restored, final candidate and linked separately under AC-5.

The separately captured tutorial replay is partial: literal pre-WI083 commands stop at `repopact validate` because the old tutorial leaves the generated dashboard stale. The two sides therefore did not complete comparable work and their elapsed times are not used as evidence of productivity or speedup. AC-1 stays pending.

## Reproduction and retained artifacts

Use the paired PowerShell scripts and their referenced immutable fixture paths as the recorded invocation; they verify base HEAD, exact two-file overlay, harness diff, and candidate file hashes before running the instrumented Cargo test. Each script executes the harness measurement with one test thread and captures raw output. `paired-overlay-comparison-summary.txt` contains the aggregation; `paired-overlay-file-length-manifest.csv` contains the matched copy manifest. `candidate-overlay-harness.patch` and `paired-overlay-baseline-harness.patch` capture the harness-only instrumentation. `candidate-overlay-source.patch` and candidate source snapshots preserve the rejected implementation for audit. Historical unoverlaid baseline and tutorial replay artifacts are explicitly exploratory/partial.