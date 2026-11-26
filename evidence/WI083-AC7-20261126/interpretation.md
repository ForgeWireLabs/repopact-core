# WI083 tutorial mechanism timing (2026-11-26)

This experiment measured deterministic RepoPact CLI command sequences in disposable fixtures on one Windows host. It used zero model calls. It is mechanism timing only: it does not measure human task time, productivity, or effectiveness.

## Sources and source command counts

- Baseline: `b9dec03ad59c2ef7d782eda9f2b3220747e8ba3c`, tree `a01027d89c3be3440067a80a32185d1a0db869f4`.
- Post-WI083: `559ed9a86a1c399826f9505e5412d11c54d099ab`, tree `8be9c62cda36506623e828a267538e5d62849195`.
- Baseline tutorial text: init 1, new 1, validate 5, verify 2, dashboard 1 (10 explicit RepoPact invocations total).
- Post-WI083 tutorial text: init 1, new 1, validate 3, verify 1, dashboard 1 (7 explicit invocations total).

## Literal sequence outcomes

Three alternating-order fixture pairs were run with the same scripted edits. Every baseline run failed at `validate-after-governance-edits` after the invariant edit and before the work item was created. Exact stdout:

```
ERROR [dashboard.stale] audits/reports/dashboard.md: generated dashboard is stale; run `repopact dashboard --root .` and commit the result

Validation failed with 1 error(s).
```

The baseline's three runs each executed only 3 CLI commands (init, validate-after-init, failing validate-after-governance-edits); no candidate implementation, evidence, or completion steps ran. Failure output and per-command raw timings are preserved unchanged in `raw-results.json`.

All three post-WI083 runs passed end-to-end. Each executed 7 commands: init, new, pre-implementation validate, evidence-bearing verify, post-evidence validate, dashboard, and final validate. Command elapsed-time sums were 5,975.065 ms, 5,926.085 ms, and 5,831.309 ms (median 5,926.085 ms). Baseline partial command sums were 2,294.706 ms, 2,137.211 ms, and 1,995.912 ms (median 2,137.211 ms).

## Interpretation and environment

A matched elapsed-time comparison is invalid: the literal baseline fails before candidate work and completion, whereas the updated sequence completes. Do not interpret the unmatched duration sums as a speedup/regression or productivity measurement. The raw JSON retains the original computed `paired_deltas`; those deltas compare incomplete and complete runs and must be disregarded.

Host: Windows 10 Pro build 19045; Python 3.12.4; RepoPact `3.1.4.dev0`; JSONSchema 4.26.0; Rust/Cargo 1.96.0; x86_64-pc-windows-msvc; 180-second harness timeout. Engine SHA-256: `f9d67aa342f1782fb559e16a052ed83739362fa464d7bc1213d0e2231e2979ae`. Engine semantic identity: `repopact-semantic-v1:sha256:a11c42f9a8129a186a44a0e7822d9f2edb1512a7bc13fbe945ede32d8acfb255`. Full environment metadata, commands, timestamps, exact outputs, edits, counts, and per-command times are in `raw-results.json`.
