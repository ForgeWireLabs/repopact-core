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
`ci` profile remains to be run on the reconciled evidence candidate. AC-1 and
AC-6 remain pending.

## Closeout

Every satisfied criterion needs linked evidence in `work-item.json`. Keep unavailable research pending. Regenerate the dashboard after governed changes. Submit one validated draft PR; do not merge, approve, or release.
