# 083 — Reduce governance debt and restore small-team throughput

> **Status**: Active
> **Lead**: tooling-owner
> **Owner scope**: tooling
> **Affected scopes**: tooling, docs, governance, work, evidence
> **Depends on**: none

## Intent

Reduce repeated RepoPact work in the shipped tutorial and verification orchestration while keeping the declared assurance intact. This is one reversible implementation batch: simplify the existing path, measure candidate validation work, and retain the current security, frozen-surface, evidence, and transaction guarantees. No new mode, cache framework, mandatory self-audit, hosted CI, paid experiment, merge, or release is in scope.

## Scope

- Remove avoidable repeated verification instructions and align bootstrap/adoption wording.
- Consolidate per-invocation verification contract loading without a public unchecked-config path or cross-run cache.
- Measure the existing Rust mutation candidate path; change it only if the safety boundaries in AC-4 remain demonstrably intact.
- Capture focused regression and exact local validation evidence, refresh semantic identity and generated dashboard, and prepare one draft PR.

## Registration and allocation

Main was verified at `b9dec03ad59c2ef7d782eda9f2b3220747e8ba3c`. Open draft PR #29 is based on that commit and carries proposed WI082 at head `daa94e8ddc264212a7e1c372b5c935c95c522ab5`. The canonical allocator would choose 082 from main, so WI083 was created through the existing Rust-engine `work.create` path on a detached local worktree of the actual PR #29 candidate. Only the generated WI083 directory was copied here; no PR #29 branch was committed or pushed. Temporary identity and documentation-mapping reconciliation in that scratch worktree was not carried over.

The first CLI create attempt timed out at its 30-second limit after dispatch. The worktree was inspected before retry; no WI083 record or dashboard update existed. A single retry through the same canonical create path with a 180-second client deadline completed in 18.285 seconds. This is a local observation, not a general performance claim.

## Optimization follow-up preflight — 2026-11-26

Before implementation, verified `origin/main` at `559ed9a86a1c399826f9505e5412d11c54d099ab`, tree `8be9c62cda36506623e828a267538e5d62849195`. PR #29 is merged; WI079 and the first WI083 batch are integrated. Added prospective AC-7 to the existing canonical WI083 JSON while leaving AC-4, AC-1/AC-6, and all historical evidence unchanged. The authorized slice is a matched original-versus-single-candidate-snapshot experiment. The runtime copy and independent live post-state validation remain required. The pre-registered adoption gate requires exact equivalence, at least 15% improvement in pooled median candidate-validation time, and no representative case with p95 more than 5% above baseline. No source implementation had started when AC-7 was registered.

## AC-1 corrected-baseline comparison preregistration — 2026-11-26

This prospective amendment was recorded before collecting any new tutorial
timings. The literal replay remains preserved in
`evidence/WI083-AC7-20261126/raw-results.json` and
`evidence/WI083-AC7-20261126/interpretation.md`: the historical instructions
failed `dashboard.stale` in 3/3 runs and the updated instructions completed
3/3. Those observations will not be rerun, replaced, or represented as a
completed historical tutorial.

The new comparison is a **corrected-baseline command-sequence comparison**.
The historical source is tutorial `docs/adopt-repopact.md` at commit
`b9dec03ad59c2ef7d782eda9f2b3220747e8ba3c`, tree
`a01027d89c3be3440067a80a32185d1a0db869f4`, Git blob
`b47c922147d8798d936279d88f0725e04d9a5282`, content SHA-256
`db6c850b8936f5da3b0f7c37dc4fa3014a2ebf43e325daa83d0ab76a7951a9e8`.
The current successful source is that tutorial at integrated main commit
`733e23a8a091727908dbd3c690fb7169f91b7287`, tree
`656543a1472f29b1380227621fe842231179264b`, Git blob
`871065f026d64e96df80ffb417eb181e92c23987`, content SHA-256
`9e5c161bb4082ca15236dfa2bba36ca98a75d684985a607579725412f0e745da`.

The corrected baseline retains every historical step and inserts exactly
three `repopact dashboard` commands immediately before the validations made
stale by the preceding edits: after the invariant edit, after the new work
item and acceptance-criterion/README edits, and after linking the verification
evidence. Each refresh makes the derived dashboard current before the existing
validation; no historical command is removed. Thus original tutorial counts
remain init 1, new 1, validate 5, verify 2, dashboard 1 (10 explicit RepoPact
invocations; 7 explicit verification invocations). Corrected-baseline counts
are init 1, new 1, validate 5, verify 2, dashboard 4 (13 total; 7
verification invocations). Current tutorial counts are init 1, new 1, validate
3, verify 1, dashboard 1 (7 total; 4 verification invocations). Counts treat
each CLI invocation once and exclude any internal validation performed by
`init`; verification counts mean explicit `validate` plus `verify` commands.

Both sides use the identical scripted governance edit, work-item creation and
acceptance-criterion text, health-check source edit, evidence link, and
completion transition. The primary comparison uses the same installed Python
CLI, one already-built compatible canonical engine, toolchain, host, and
repository fixture path for both sides within each pair. Engine build source is
`805f86ccf46d2311bbe19121cad22904b5993c9d`; executable SHA-256 is
`63a78d027472020dd6d1aa3a6e2a7d49ed092f529dcf1efa77283a8e1d0ca81b`; recorded
semantic identity is
`repopact-semantic-v1:sha256:a11c42f9a8129a186a44a0e7822d9f2edb1512a7bc13fbe945ede32d8acfb255`.
The runtime-source delta from that build revision to integrated main is empty.
The build profile is debug, target `x86_64-pc-windows-msvc`, Rust/Cargo
1.96.0; Python is 3.12.4, RepoPact is 3.1.4.dev0, and jsonschema is 4.26.0.
This same-engine comparison isolates the documented command-sequence change;
it is not a historical-binary performance comparison.

The fixed sample plan is two warmup runs per side followed by ten matched
pairs, alternating which side runs first within each pair. Preserve every
command's argv, output, error, exit code, and elapsed time; report CLI-command
totals separately from verification-command counts, per-sequence sum median
and p95, per-command median and p95, and paired deltas. No sample exclusions
are planned; any failure is retained and keeps AC-1 pending. Final file-tree
equivalence is checked on each completed pair. Only the generated evidence ID
(in the evidence filename, manifest root `id`, and acceptance-criterion
reference), generated timestamps in that verification manifest, and generated
duration/elapsed fields in that manifest may be normalized. Every other path
and byte must match. The raw record retains actual IDs, timestamps, command
timings, and outputs. No model calls are used. AC-6 remains pending and is not
part of this batch.

The canonical harness and resulting measurements are linked under
`evidence/WI083-AC1-20261126/`; no source snapshot is copied. This is a
mechanism-level command-sequence comparison only. It supports no human-time,
productivity, effectiveness, causal, or historical-binary claim.

### AC-1 result — 2026-11-26

The preregistered run is recorded as
`20261126-wi083-ac1-corrected-baseline-comparison`. Two warmups per side and
ten alternating-order matched pairs completed (24 sequence runs total); all
240 RepoPact CLI invocations exited 0 and no samples were excluded. Each
corrected-baseline run used 13 commands (7 explicit verification commands:
5 `validate`, 2 `verify`); each current run used 7 commands (4 explicit
verification commands: 3 `validate`, 1 `verify`). The original historical
source count remains 10 commands and 7 explicit verification commands.

Across the ten measured runs per side, corrected-baseline CLI elapsed-time
sums had a 10,235.011 ms median and 10,419.941 ms p95. Current-sequence sums
had a 5,835.166 ms median and 5,939.742 ms p95. The paired corrected-minus-
current delta had a 4,400.678 ms median and 4,570.877 ms p95. Percentiles use
the preregistered nearest-rank calculation. These are same-engine command
sequence timings only.

The final-state equivalence check failed for every measured pair and both
warmups. The sole mismatched path was
`work/completed/001-wire-up-the-health-check/work-item.json`; all other paths
matched after the preregistered generated-ID/time normalization. Raw evidence
retains raw and normalized hashes for that path but not its contents, so it
does not establish which field differs. No additional normalization or
post-result sample was applied. AC-1 remains pending because comparable final
governed state was not proven. The already-recorded literal replay was not
repeated or rewritten. AC-6 remains pending.

The storage preflight found 13,335,900,160 free bytes against the 12 GiB
minimum. The exact engine, Python CLI, source identities, commands, errors,
timings, state hashes, and raw command outputs are in
`evidence/WI083-AC1-20261126/raw-results.json`.

## Decisions

## Decisions

- A successful declared profile run can satisfy the same check for the same relevant candidate if no relevant inputs have changed since it ran. Focused tests during iteration do not replace the final required profile.
- Keep preflight and current-state checks at their existing boundaries. Keep all profile steps and the required legacy comparator distinct where their assurance differs.
- Do not reuse executable evidence based on commit/tree or a doc-only assumption. Do not add a cache unless a complete relevant read set and all execution inputs are proven.
- Leave matched productivity/effectiveness research as an explicit pending outcome, using WI022 and the open WI082 proposal rather than claiming a result from this batch.

## Validation repair

The first full `ci` evidence run is preserved as
`20261126-000614-018647-5e45898d-verify-ci` and failed: 207 Python fixture
setups saw malformed inherited indexed Git config, and conformance found an
engine built from `b01896a` while the checkout was `2b1c20d`. Clearing the
runner-only indexed Git config after the self-audit override test reduced the
next failure count, and the focused self-audit/status sequence passed 42/42.

The second full `ci` evidence run is preserved as
`20261126-003030-342440-95556594-verify-ci`; its conformance provenance record
is `20261126T005845093723Z-wi076-ac6-engine-conformance`. It still had 67
fixture setup errors because early tests encounter the host's malformed
`GIT_CONFIG_*` values before that cleanup test runs. Disposable fixture Git
calls now remove inherited Git overrides, with a dedicated hostile-environment
regression; the affected fixture and research-metadata modules passed 30/30.
The same run found Cargo had reused source-revision metadata from `2b1c20d` at
checkout `ec070bf`. The engine build now tracks `HEAD`, the active branch ref,
and packed refs, so a checkout advance rebuilds the embedded source revision.
After commit `e86bb27`, Cargo rebuilt the engine automatically when the branch
ref advanced; the embedded source revision matched that commit. The canonical
and legacy conformance comparison then passed 43/43 each, WI050 passed 8/8,
WI079 passed 10/10, and process-image provenance was complete. This run is
recorded as `20261126-wi083-current-legacy-python-comparator`, with provenance
`20261126T010905966805Z-wi076-ac6-engine-conformance`. Both failed profile
records link under AC-5. The incomplete provenance artifact remains preserved
as diagnostic history, not completion evidence. Exact-candidate full-profile
evidence was still pending at this point.

The third full `ci` run, `20261126-011205-922819-1a876a8c-verify-ci`, passed
repository validation, conformance, the Rust workspace, spec freshness, and
the frozen-surface check. Four Python tests still inherited the malformed
Git environment: source-state evidence returned no status, and fleet/graph
fixtures could not initialize local Git repositories. The passing conformance
provenance is `20261126T014330886187Z-wi076-ac6-engine-conformance`.
`run_conformance` now removes inherited `GIT_*` values when measuring local
source state; the fleet and graph test helpers likewise isolate their local
Git commands. This third negative profile and its successful conformance
record remain linked under AC-5. After the repair, the affected provenance,
fleet, and graph-merge tests passed 10/10 under the injected host environment.
After commit `c14a0b9`, the exact-revision legacy comparator was rerun and
passed 43/43 in both implementations; WI050 passed 8/8, WI079 passed 10/10,
and engine process-image provenance matched the clean source revision. This is
recorded as `20261126-wi083-final-legacy-python-comparator`, with provenance
`20261126T014817231543Z-wi076-ac6-engine-conformance`.

The final full `ci` profile, `20261126-014953-695892-0e9cd0cd-verify-ci`,
passed all five required steps and the optional frozen-surface report in
1894.516 seconds. It started clean at commit
`3c5e098541f6f37e5ea1b61915cbe31c08639f68`; the end snapshot retained that
commit and tree, with run-generated evidence making the working tree dirty.
Conformance provenance
`20261126T022053868434Z-wi076-ac6-engine-conformance` is complete and embeds
the same source revision. AC-2 through AC-5 are now satisfied. AC-1 remains
pending because tutorial elapsed time was not captured; AC-6 remains pending
because this batch did not include a matched comparative efficacy study.

## Coordinator review corrections

Coordinator review of draft PR #31 found that the engine build script inherited
Git overrides, watched a missing `packed-refs` path, and needed to keep refresh
behavior across loose and packed refs. Git discovery now removes inherited
`GIT_*` variables, watches only existing files plus the active ref's nearest
existing parent directory, and keeps the worktree-specific `HEAD` dependency.
The Cargo regression covers hostile overrides, branch advance, pack/unpack
refresh, linked detached worktrees, and a no-op build with no `packed-refs`
file. The measured no-op build was 113.4 ms and did not rerun the build script.

The same review found a self-audit test that deleted the parent process's
`GIT_CONFIG_*` variables after a scoped override. The test now asserts that the
complete parent Git environment, including empty values, is restored and
sanitizes its later fixture Git call. Candidate identity in
`verification._git_value` now ignores inherited Git overrides and leaves
`dirty` unknown if the commit, tree, or status cannot be read. The conformance
provenance regression also verifies the exact source revision with hostile Git
overrides present.

Focused evidence `20261126-wi083-coordinator-review-focused` records the
exact-source regressions, both test orders, editable Rust build, formatting,
semantic identity, and frozen-surface checks on commit
`cf7ed4e25db9ebffefcfc5fde194646574eed246`. The full local `ci` profile and the
required legacy comparator are being reconciled on the corrected candidate.
The legacy comparator passed 43/43 in both implementations, WI050 passed 8/8,
WI079 passed 10/10, and process-image provenance was complete on clean commit
`7b4d1efa0bd856c42f4eec32951193f8a887b3f9`; evidence is
`20261126-wi083-coordinator-review-legacy-python-comparator` with provenance
`20261126T034610580481Z-wi076-ac6-engine-conformance`. The single final local
`ci` profile first ran on clean commit
`c175f19ab2828471727b0aea713e92e4fdeadef7`. It failed only the Python test
step: 528 tests ran, with 14 errors and 8 skips, while validation, conformance,
the Rust workspace, spec freshness, and frozen-surface checks passed. The
failure record is `20261126-wi083-coordinator-review-ci-failed`; the traceback
showed test Git subprocesses inheriting the runner's malformed indexed config.
The remaining repository, admission, and storage fixtures now filter inherited
`GIT_*` values without mutating the parent environment. Focused evidence
`20261126-wi083-coordinator-review-ci-fixture-regressions` records 30/30 passes
in 115.351 seconds on clean commit
`10e42cfb3e5710a76d72e99e64262e68f1d4ac63`. The final local `ci` profile
passed all five required steps and the optional frozen-surface report in
2194.83 seconds on clean commit
`aa0b12115641ce0eb8f224b258c594ee8b2380ee`. Evidence is
`20261126-wi083-coordinator-review-ci-final`; conformance provenance
`20261126T050959411867Z-wi076-ac6-engine-conformance` is complete and embeds
the same source revision. AC-2 through AC-5 remain satisfied. AC-1 and AC-6
remain pending.

## Closeout

Every satisfied criterion needs linked evidence in `work-item.json`. Keep unavailable research pending. Regenerate the dashboard after governed changes. Submit one validated draft PR; do not merge, approve, or release.

## AC-7 optimization gate — 2026-11-26

The paired original-versus-single-snapshot experiment completed on the exact
post-WI079 base and identical 1,220-file fixture. Focused prototype equivalence
tests passed, but the pre-registered adoption gate failed: pooled median
candidate-validation improvement was 4.283% against a 15% threshold, and
active-create p95 regressed 41.5% against the 5% maximum. The optimization was
not adopted; both Rust runtime files were restored to the base. Repository copy
and independent live post-state validation remain intact. Full methodology,
per-case results, limitations, source hashes, and retained raw artifacts are in
`evidence/WI083-AC7-20261126/README.md`; evidence IDs are
`20261126-wi083-ac7-paired-overlay` and
`20261126-wi083-ac7-focused-prototype`. AC-7 is satisfied by completing its
measurement and safety decision, including the required negative outcome. AC-1
and AC-6 remain pending.

## Final local validation repair — 2026-11-26

The first `ci` attempt for AC-7, `20261126-wi083-ac7-final-ci`, stopped at
its storage preflight (11,258,843,136 bytes free; 12,884,901,888 required) and
ran no steps. It remains preserved. After reclaiming 4.4 GiB of this batch's
rebuildable Cargo target caches, retry
`20261126-wi083-ac7-final-ci-retry` ran all six declared checks in 2,390.766
seconds. Repository validation, the Python unit suite, Rust workspace tests,
SPEC freshness, and frozen-surface check passed. Canonical conformance vectors
were 43/43, WI050 admission 8/8, and WI079 reference vectors 10/10, but the
profile correctly failed its provenance requirement because the candidate
worktree was dirty while the uncommitted WI083 ledger/evidence changes were
under validation. Provenance artifact
`20261126T181354883155Z-wi076-ac6-engine-conformance` records the precise
reason: the source commit alone did not identify the dirty working tree. No
source or test failure occurred. These failed attempts remain linked under
AC-5; the final clean-revision full profile and legacy comparator are still
required.

The first explicit legacy-comparator attempt is preserved as
`20261126T183035346612Z-wi076-ac6-engine-conformance` with its captured console
output in `evidence/WI083-AC7-20261126/legacy-comparator-first-attempt-incomplete.txt`.
All 43 canonical and 43 compatibility cases, WI050 8/8, and WI079 10/10 passed;
provenance correctly remained incomplete because the executable's embedded
source revision was `559ed9a` while the clean checkout had advanced to
`f9cf6a2`. The engine will be rebuilt against the finalized candidate commit
before the successful comparator and final profile.

The clean-revision legacy comparator passed on commit
`6e79ce512bc5b99cd73ec2adde1136b720fb51ca` after rebuilding the debug engine
with `cargo build --locked --manifest-path rust/Cargo.toml -p repopact-engine`.
The embedded engine source revision matches the checkout and process-image
provenance is complete. Canonical Rust 43/43, legacy Python 43/43, WI050 8/8,
and WI079 reference 10/10 passed. Evidence ID
`20261126T183309135759Z-wi076-ac6-engine-conformance` is linked under AC-5;
its raw output is `evidence/WI083-AC7-20261126/legacy-comparator-final.txt`.

## Final clean-candidate validation — 2026-11-26

After committing the WI083 measurement and comparator evidence, rebuilt the
engine for commit `805f86ccf46d2311bbe19121cad22904b5993c9d` (tree
`20867865b9b3e8e2ca196b887281d11b8f005651`) and ran the full local `ci`
profile. All five required steps passed: repository validation, the full Python
unit suite, canonical conformance, the Rust workspace, and SPEC freshness. The
optional frozen-surface report also passed. Total profile duration was
2,530.828 seconds. Start identity was clean at the commit/tree above; end
identity retained that commit/tree and was dirty only because this run recorded
its own evidence. Canonical conformance process-image provenance is complete
at `20261126T191658572565Z-wi076-ac6-engine-conformance`. Profile evidence
`20261126-wi083-ac7-clean-final-ci` and the conformance provenance are linked
under AC-5. AC-1 and AC-6 remain pending; no productivity or comparative
efficacy claim is made.
