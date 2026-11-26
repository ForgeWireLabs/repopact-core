# WI079 final validation and closeout — 2026-11-25

## Exact source and engine

The clean source candidate at the start of the final local CI profile was commit
`7c451c7da22616869816e2513c4404c6b01a0cb9`, tree
`70878e28e0d2a8f65d4cb3569b9ae6144b20f4b6`. The profile later wrote its own
immutable evidence and conformance artifact; its record therefore distinguishes
the clean `candidate_at_start` from the evidence-bearing working tree afterward.

The matching Windows release engine was built from that exact commit with
`REPOPACT_SOURCE_REVISION=7c451c7da22616869816e2513c4404c6b01a0cb9`. Its
conformance process-image evidence reports the same source revision and semantic
identity `repopact-semantic-v1:sha256:b6cdc89b9c4f6f4998d341cf592439e43455b694442735dbacb1c42cb3b1a90b`.
The engine SHA-256 was
`9a5043b8dffbabc8e27b9a77e32c1f917f1c966d8ede8f4d7f1a2440402fba5a`.

The earlier focused run began from expected PR29 source commit
`daa94e8ddc264212a7e1c372b5c935c95c522ab5`, tree
`eba436807784d81052e376a4555ca0d96e451810`. The only changes from that source
in the exact final CI commit before execution were reviewed canonical manifest,
identity, documentation-snapshot, and dashboard outputs; the full suite then
re-executed the repaired implementation at the exact final source commit.

## Validation results

| Gate | Result |
| --- | --- |
| New net-diff reversal regression | 1/1 passed |
| New unhashable finding-status regression | 1/1 passed |
| WI079 self-audit focused suite | 35/35 passed |
| Opt-in verification-boundary suite | 7/7 passed |
| WI079 reference corpus | 3/3 passed |
| Full Python suite in exact-source local CI | 522 tests, OK; 4 skipped |
| Canonical Rust conformance | 43/43 passed |
| WI050 admission corpus | 8/8 passed |
| WI079 reference conformance vectors | 10/10 passed |
| Locked Rust workspace tests | passed |
| SPEC freshness | passed |
| Frozen-surface report against `origin/main` | passed; no changes detected |
| Complete local `verify ci` profile | exit 0; all 5 required steps and optional frozen report passed |

The full local profile ran on Windows for 2,315.453 seconds. At process start,
all inherited `GIT_*` variables, `CARGO_TARGET_DIR`, and any hosted CI/CD flags
were absent; `REPOPACT_GITHUB_CI` and `REPOPACT_GITHUB_CD` remained unset. The
Python and Rust steps were not narrowed or weakened.

## Preserved setup failures and retries

The first editable install stopped because the reviewed semantic-input manifest
was stale; the explicit canonical generator update fixed that generated state,
and the subsequent editable install succeeded. The first focused 35-test run
had 34 passes and one import error because `jsonschema` was absent from that
isolated environment. Installing the declared dependency was followed by a
35/35 successful retry. Every initial failure and successful retry log, with
exit-code sidecars, is preserved under
`evidence/runs/20261125-079-pr29-preflight-attempts/` and indexed by
`20261125-079-pr29-preflight-attempts`. These setup failures are not relabeled
as passing runs.

## Documentation and generated artifacts

The adopter guide and root-contract rollout links remain documented in the WI079
README and Decision 0079. The guidance accurately limits the self-audit to an
opt-in, instruction-level workflow and does not claim protected Git, merge, or
same-principal enforcement. The prospective 2026-11-22 draft rollout remains
linked; it is not represented as evidence of executable enforcement.

Canonical source-to-documentation snapshots were refreshed and checked:

- `experimental-self-audit`: source digest
  `ac5bb0651f9390f787295a57ccb15930a838b9712a830162150d23dd65c0a59f`,
  documentation digest
  `e37a66b4ce054f44f258a5070e84a35d0fa4aa93361c42d561fe6c6e67b32741`.
- `engine-semantic-compatibility`: source digest
  `555b50c8b77c83bc02779a955c7babc31b46aa092342cecb00c3bd1808418bbc`,
  documentation digest
  `e5eb7f75354a7ddfed2369c3cb19c992c8f780912f6531f3611130767edfb3a5`.

The semantic manifest, semantic identity, documentation mappings, and dashboard
were generated with canonical RepoPact tools. The final exact-source CI evidence
is `20261125-079-pr29-final-local-ci`; complete conformance provenance is in
`20261125T233224935458Z-wi076-ac6-engine-conformance`. Historical failed
attempts remain preserved and distinguishable.

## Lifecycle and limits

AC-10 is satisfied by this criterion-linked documentation and validation record.
AC-2 and AC-9 remain waived under the recorded operator disposition. The
comparative protocol remains NOT RUN and is transferred to proposed WI082; WI082
stays proposed with all five criteria pending. No research execution or spending
occurred. WI080 was not changed. No hosted CI/CD, release, publication, merge,
or protected-surface change occurred. The exact local profile demonstrates only
the claimed opt-in local verification path; it does not establish an external
merge gate or independent approval.

WI079 is complete and the entire work-item directory has been moved to `work/completed/`.

## Post-lifecycle canonical checks

After the move and evidence linkage, canonical semantic identity, both documentation snapshots, SPEC regeneration, dashboard generation, `repopact validate`, frozen-surface verification against `origin/main`, Rust formatting, and staged/unstaged diff checks passed with the exact-source release engine explicitly selected and hosted flags unset. Results and exit codes are preserved under `evidence/runs/20261125-079-pr29-final-checks/`. The final staged whitespace check excludes only the byte-preserved initial editable-install failure log, whose captured Cargo output contains trailing blank lines; all source, governance, documentation, manifests, and other evidence files are checked. The first post-move validation correctly rejected a raw stdout transcript that had a `.json` extension; it was renamed to `.log`, and the corrected validation passed. A later attempt without `REPOPACT_ENGINE` timed out at engine handshake; after setting the matching engine, the checks passed. An accidentally initiated duplicate local profile was interrupted before a terminal result and is recorded separately; it is not counted as a gate. No second full CI profile was run.
