# WI081 evidence-backed CI self-contamination — 2026-11-24

## Preserved first full-profile result

The initial local `repopact verify ci` run used source commit
`8d1bc9771847a00feabd41bba7138606abaa4044` and tree
`1872b961fbefd3bfc8aa84ea958ef1c53a90dd7d`, with `CARGO_TARGET_DIR` unset.
It completed in 1,738.672 seconds with exit code 1. Its evidence remains a
failure and is not replaced by the later repair tests.

| Step | Result | Evidence |
| --- | --- | --- |
| `validate` | passed, exit 0 | `evidence/runs/20261124-081-final-ci-default-e.json` |
| `python-tests` | failed, exit 1; 475 tests, one error, five skipped | same run record and bounded capture |
| `conformance` | failed, exit 1; 43/43 canonical cases and 8/8 WI050 vectors passed, but engine provenance was incomplete | `evidence/runs/20261124T210841054492Z-wi076-ac6-engine-conformance.json` |
| `rust-workspace` | passed, exit 0 | same run record |
| `spec-freshness` | passed, exit 0 | same run record |
| `frozen-surface-report` | passed, exit 0 | same run record |

Python's exact error was `FileExistsError` in
`test_unfinished_capture_overrides_effective_status_to_incomplete` while
creating `evidence/runs/.in-progress`. The test fixture copier included the
active verification run's untracked marker pair. The marker was produced by
the same profile being tested, so this was a WI081 self-contamination defect.

Conformance reported the exact source revision dirty because its status check
counted both the active marker pair and the outer diagnostic log. The outer
log had been placed inside the disposable checkout before `verify ci` began;
that was a validation-harness contamination. The product defect was that
conformance had no narrow way to distinguish the current run's journal from
actual source changes. The original run correctly remained `fail`; both causes
are addressed by the repair and will be checked from clean exact-head
checkouts with outer logs stored outside those checkouts.

## Repair and regression evidence

Only the current invocation's validated journal pair is now passed to child
processes and excluded from fixture/source identity checks. The state file must
match its ID, format, running phase, and expected log path, and the paired log
must exist within its size bound. A stale or malformed marker, another
invocation's marker, or any unrelated untracked file remains visible as dirty
source. The repaired conformance record will list the two excluded runtime
paths explicitly.

The shared fixture materializer now excludes only that exact live pair while
continuing to include unrelated non-ignored evidence. A regression verifies
that conformance still rejects an unrelated source change and orphan marker.
The duplicate `test_unfinished_capture_overrides_effective_status_to_incomplete`
definition was removed so the assertion exists only once.

Focused logs:

| Check | Result | SHA-256 |
| --- | --- | --- |
| `tests.test_dev_fixtures`, `tests.test_verify_status`, `tests.test_conformance_provenance_identity` | 30 passed | `93752a0704e3092958a300a81038c0ce1b3b1b55b09b429421fb415ec067353b` |
| Capture identity and fixture-focused set | 35 passed | `dba0c9068a98da49b27c3a2d83aeb747412bb7a4f0767d3f07fb73c746ea42c7` |
| Current-marker lifecycle, fixture and provenance regressions | 7 passed | `7ce686a50713c07752c60fe386314196eebb8bfef61376a61ebb1486f526a02d` |

These are targeted repair checks, not substitutes for the complete Python,
Rust, conformance, and exact-head local-CI reruns.

## Immutable failed-run artifacts

| File | SHA-256 |
| --- | --- |
| `evidence/runs/20261124-081-final-ci-default-e.log` | `895d7a181db6427e8237d69361e6c69a0d94b3a027c6d9637f18a41602fb2c81` |
| `evidence/runs/20261124-081-final-ci-default-e.json` | `ccbbc62139a06bd46d37c75363dace4755eba7b9bb1af2c9cdfaad7f527fbb52` |
| `evidence/runs/20261124-081-final-ci-default-e.capture.jsonl` | `c44d2f3af52247a93d402ca3128d5e123812f59805d6989f0efbca9d9c287948` |
| `evidence/runs/20261124T210841054492Z-wi076-ac6-engine-conformance.json` | `e5095a2c71f53b325cf5ab43b0d24603857ae401bed61df00a9392c462acf318` |
