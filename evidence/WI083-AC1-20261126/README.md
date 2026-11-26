# WI083 AC-1 corrected-baseline comparison

## Outcome

The preregistered same-engine command-sequence comparison completed with two
warmups per side and ten alternating-order matched pairs. All 24 sequences
completed, all 240 RepoPact CLI invocations exited 0, and no samples were
excluded. The historical tutorial's literal failure remains preserved in
`../WI083-AC7-20261126/`; it was not rerun or described as a completed path.

| Sequence | Total CLI commands | Explicit verification commands | Median command-time sum | p95 command-time sum |
| --- | ---: | ---: | ---: | ---: |
| Corrected historical baseline | 13 | 7 (5 `validate`, 2 `verify`) | 10,235.011 ms | 10,419.941 ms |
| Current tutorial | 7 | 4 (3 `validate`, 1 `verify`) | 5,835.166 ms | 5,939.742 ms |

The ten paired corrected-minus-current time deltas had a 4,400.678 ms median
and a 4,570.877 ms p95. Percentiles use nearest rank. Counts describe explicit
tutorial invocations; any validation internal to `init` is not counted as a
separate command or verification invocation. The historical source itself
remains 10 total explicit commands and 7 explicit verification commands. The
corrected baseline retained all ten and inserted three dashboard refreshes
before validations made stale by the invariant edit, work-item edit, and
evidence link. The current path remains 7 total commands.

These measurements isolate deterministic command sequences on one pinned
engine/toolchain. They do not measure human time, productivity, effectiveness,
causality, or historical-binary performance.

## Final-state comparison

The preregistered final-state equivalence check failed in all ten measured
pairs and both warmups. The only mismatched path was
`work/completed/001-wire-up-the-health-check/work-item.json`; every other path
matched after the preregistered normalization. The raw evidence records raw
and normalized hashes but not the file contents, so the underlying field
difference cannot be identified from this run. No new normalization was
introduced after seeing the result and no samples were repeated. Therefore
AC-1 remains pending: comparable final governed state was not proven.

The linked work item preserves this outcome as partial evidence. AC-6 remains
pending and was not studied.

## Provenance and reproduction

- Preregistration commit: `1d6462a6e13e0501efd83800db92588cca206854`, tree
  `1018d5f399605f6f794929204676cca382c6443d`.
- Harness: `compare_tutorial_sequences.py`, SHA-256
  `a29b2f7711430b0057847e26dc802b5d75b5a370f9d3fd7b4edfc42b6d96d1a6`.
- Raw results: `raw-results.json`, SHA-256
  `da59922853927c5f8cc12ffe1b68324e19c127cb225049acc4f6d16eddb17d74`
  over the canonical Git blob bytes.
- Historical tutorial: `b9dec03ad59c2ef7d782eda9f2b3220747e8ba3c`, tree
  `a01027d89c3be3440067a80a32185d1a0db869f4`, blob
  `b47c922147d8798d936279d88f0725e04d9a5282`, SHA-256
  `db6c850b8936f5da3b0f7c37dc4fa3014a2ebf43e325daa83d0ab76a7951a9e8`.
- Current tutorial: `733e23a8a091727908dbd3c690fb7169f91b7287`, tree
  `656543a1472f29b1380227621fe842231179264b`, blob
  `871065f026d64e96df80ffb417eb181e92c23987`, SHA-256
  `9e5c161bb4082ca15236dfa2bba36ca98a75d684985a607579725412f0e745da`.
- Engine source `805f86ccf46d2311bbe19121cad22904b5993c9d`; SHA-256
  `63a78d027472020dd6d1aa3a6e2a7d49ed092f529dcf1efa77283a8e1d0ca81b`;
  semantic identity
  `repopact-semantic-v1:sha256:a11c42f9a8129a186a44a0e7822d9f2edb1512a7bc13fbe945ede32d8acfb255`.
- Python 3.12.4, RepoPact 3.1.4.dev0, jsonschema 4.26.0, Rust/Cargo
  1.96.0, Windows 10 build 19045, `x86_64-pc-windows-msvc`; zero model calls.

Reproduce only the registered batch with the installed compatible environment:

```powershell
python evidence/WI083-AC1-20261126/compare_tutorial_sequences.py --run
```

The fixed raw record includes every command's argv, stdout, stderr, return
code, elapsed time, scripted edit hashes, and final-state hashes.

## Candidate checks and reuse

After linking the partial evidence and refreshing the WI083 dashboard,
`repopact validate --root .` passed and
`repopact check-frozen --base origin/main` reported no frozen-surface changes.
The source-to-candidate delta from the existing full local `ci` profile at
`805f86ccf46d2311bbe19121cad22904b5993c9d` is confined to `audits/`,
`evidence/`, and `work/`; there are no runtime, test, or documentation source
changes. The successful `20261126-wi083-ac7-clean-final-ci` profile and the
existing `20261126-wi083-final-legacy-python-comparator` result therefore
remain applicable to unchanged checks. No build, unit suite, full profile, or
legacy comparator was repeated. The repository's hosted governance workflow
is opt-in through `REPOPACT_GITHUB_CI`; the repository currently has no Actions
variables, so the draft PR will not dispatch hosted validation.
