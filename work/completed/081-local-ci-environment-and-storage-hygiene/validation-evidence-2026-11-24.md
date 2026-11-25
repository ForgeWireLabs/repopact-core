# WI081 validation evidence — 2026-11-24

## Exact source and disposition

Full local CI was run from clean exact-source worktrees at commit
`c78f0159a13effe84623d4be4296c079af19a95b`, tree
`e42775414a41db90b69000813a31aa8edd4b04b4`. Both profiles recorded a clean
candidate at start. Their final candidate status is dirty because the profiles
write their own run records into the checkout; the source commit and tree did
not change. The default profile left Cargo output under `rust/target`; the
controlled profile used the registered external target directory shown in its
run record. No hosted CI or CD was enabled.

## CARGO_TARGET_DIR failures and repair

The preserved WI080 PR21 full-profile record
`evidence/runs/20261124-080-pr21-local-ci.json` is `failed` at source commit
`1c7f1d0bcdbca8fb99b1ee3df8c13165e2a1e771`, tree
`b18f7c654d63bd048c218ff04ebfbcd909ff7054`. Its Python step exited 1. A
controlled reproduction at that same revision names all nine affected tests
in `evidence/runs/20261124-081-pr21-cargo-target-sanitized-repro.log`: the
synthetic package fixture supplied Cargo metadata with a temporary local
`target_directory`, while the process inherited a different external
`CARGO_TARGET_DIR`. The source verifier correctly rejected that inconsistent
identity with `ArchiveIdentityError: Cargo metadata target directory differs
from the active CARGO_TARGET_DIR`. The reproduction removed inherited `GIT_*`
variables and retained only the controlled Cargo target setting. The matching
sanitized control removed both inherited Git and Cargo settings; all nine
tests passed. The original full-profile record remains failed and unchanged.

The test fixture now scopes `CARGO_TARGET_DIR` to the synthetic fixture's
target for ordinary package tests and restores the caller environment. Two
regressions separately prove that a matching external target is accepted and a
mismatched external target is rejected. All eleven package identity tests pass
with default and external targets, including sanitized environments; the
assertions remain intact. Evidence: `tests/test_engine_package_identity.py`,
`evidence/runs/20261124-081-cargo-target-tests-sanitized-default.log`, and
`evidence/runs/20261124-081-cargo-target-tests-sanitized-external.log`.

## Evidence capture and provenance self-contamination

The first WI081 full profile is preserved as failed at commit
`8d1bc9771847a00feabd41bba7138606abaa4044`, tree
`1872b961fbefd3bfc8aa84ea958ef1c53a90dd7d`. Its Python fixture copied the
profile's own `.in-progress` journal and raised `FileExistsError`; conformance
also treated that journal and an outer diagnostic log placed inside the
checkout as source changes. This was a WI081 self-contamination defect, separate
from the CARGO_TARGET_DIR fixture mismatch. The original status remains
`failed`; see `evidence/runs/20261124-081-final-ci-default-e.json`, its log,
partial capture, and the matching failed conformance record.

The repaired runner passes its active capture ID only to its child processes.
Fixture materialization and conformance exclude only the validated current
invocation marker pair; unrelated, stale, malformed, or orphan files remain
visible as source changes. The complete Python suite and conformance pass in
both exact-head profiles. `tests/test_verify_capture.py` also injects
interruption, `OSError(28)` during output capture, short writes, and evidence
write failure. It asserts exit failure/incomplete status, retains the bounded
partial journal, and verifies that no completed PASS record is emitted. The
historical PR21 raw output-capture interruption was not available as a
complete log and is not reconstructed or represented as a passing run.

During final reconciliation, the first canonical attempts exposed two
evidence-authoring problems: raw inventory payloads in `evidence/runs` lacked
the required evidence-run envelope, and acceptance criteria referenced file
paths where RepoPact requires evidence IDs. Those failed validation logs remain
preserved. The inventory payloads are now embedded in schema-valid evidence
records, and criteria refer to registered IDs. An initial PowerShell focused
test log serializer also emitted `System.Object[]` despite test exit 0; those
incomplete capture attempts are preserved with `capture-serialization-failure`
in their names. The same tests were rerun and their complete output is in the
`*-verified.log` files referenced by their evidence records.

## Storage inventory and cleanup

The bounded inventory and preflight were captured at the WI081 source
worktree; a second inventory explicitly set the controlled external
`CARGO_TARGET_DIR` so that the external tree was measured as well. Results:

| Path or category | Files | Bytes | Notes |
| --- | ---: | ---: | --- |
| WI081 worktree `rust/target` | 2,345 | 1,174,875,641 | default target tree |
| Exact-head default CI worktree `rust/target` | 3,709 | 1,958,051,894 | dominant per-worktree build output |
| Exact-head external Cargo target | 3,708 | 1,958,030,618 | separate build output, matched to external profile |
| Exact-head default CI checkout | 4,846 | 1,976,536,724 | includes default Cargo target |
| Exact-head external-target CI checkout | 1,137 | 18,484,900 | Cargo output remained outside checkout |
| Python `build/` | 2 | 45,490 | no `.venv` or `dist` found |
| Immutable evidence runs | 342 | 3,263,060 | excluded from reclamation |

The inventory is bounded (100,000 files / 32 GiB limits), records registered
and unregistered Git worktrees, checks dirty state, and reports cleanup
eligibility only for explicitly registered detached clean worktrees. The
12-GiB per-volume preflight passed; free space was approximately 40.9 GiB at
the final inventory. Duplicate Cargo target trees dominate validation disk
use; reports, Python build output, and evidence are much smaller. No attempt was
made to infer that this inventory exactly reproduces the earlier host disk
exhaustion.

The full inventory snapshot counted 342 evidence files (3,263,060 bytes).
After final reconciliation added its records and logs, the current evidence
directory measured 356 files (3,310,645 bytes); `audits/reports` contained one
dashboard file totaling 1,641 bytes. These later counts are separately
measured so the earlier inventory remains an accurate time-stamped snapshot.

Cleanup remains explicit and dry-run by default. Target cleanup requires the
registered ownership marker and an unchanged exact file snapshot. Worktree
cleanup requires the registered detached commit, clean status, no ignored or
untracked files, a non-protected path, and bounded size. The preserved cleanup
sample previews and removes only an explicitly owned empty target; its
worktree cleanup attempt is refused because ignored files exist. No user,
unregistered, dirty, or evidence path was removed. The earlier unsealed-target
inventory error is preserved; current `_registered_artifact_summary` treats a
null snapshot as absent, and final inventory of those unsealed records exits 0.

## Final exact-head CI and generated identity

Both canonical `repopact verify ci` profiles passed all six recorded steps
(five required plus the frozen-surface report): `validate`, complete Python
tests, conformance, locked Rust workspace, spec freshness, and frozen-surface
report. Each profile executed 477 Python tests with 5 skips. Conformance used
the engine built from the exact source revision, reported 43/43 canonical
cases and 8/8 WI050 vectors passed, and reported a clean source with the active
journal pair listed separately as runtime artifacts.

| Profile | Evidence | Duration | Result |
| --- | --- | ---: | --- |
| Default Cargo target | `evidence/runs/20261124-081-final-ci-default-c78f015.json` | 1,759.828 s | exit 0, pass |
| External `CARGO_TARGET_DIR` | `evidence/runs/20261124-081-final-ci-external-c78f015.json` | 1,912.843 s | exit 0, pass |

The source semantic identity is
`repopact-semantic-v1:sha256:6e369782c9d7a077b30a508bb6ac3aefb3547caee748d9353dd400a741b8fcd2`.
The documentation mapping snapshot records source digest
`e3ca0ac3d55fbdfe41ee7acd36815a06766996f4c6b7d77dc2f45e0e4cbadd91` and
documentation digest
`1fc2acdea4d2217eb53a8e8c23388bed705be45b26b56ae90a203d4807491ace`.
The dashboard was regenerated after acceptance evidence was reconciled.

SHA-256 values for the primary records and full-output logs:

| Artifact | SHA-256 |
| --- | --- |
| Original PR21 failed CI record | `D62DB2414909C87CE9B23EC5581ED45258FD99A2C081A3D381DE655C56564DC8` |
| Sanitized CARGO target reproduction | `034414DE5BC20B645DA2C00A37BF15CE847C22A2ECE72A02138D83300D3BD1A2` |
| Sanitized CARGO target control | `9FCFC3C5FA03461A69B826B81AE55B0BD2E9A2DDCF3D5CAA84293DA9B4752FE4` |
| First WI081 failed profile record | `CCBBC62139A06BD46D37C75363DACE4755EBA7B9BB1AF2C9CDFAAD7F527FBB52` |
| First WI081 failed profile log | `895D7A181DB6427E8237D69361E6C69A0D94B3A027C6D9637F18A41602FB2C81` |
| First WI081 partial capture journal | `C44D2F3AF52247A93D402CA3128D5E123812F59805D6989F0EFBCA9D9C287948` |
| First WI081 failed conformance record | `E5095A2C71F53B325CF5AB43B0D24603857AE401BED61DF00A9392C462ACF318` |
| Storage inventory with default and external target | `13E33B8EE5FF81C0794359F83841D019C4555C70EE30092BDA79AF9CD233BA8C` |
| Storage inventory with no configured external target | `42159BA0B6D0AE34B6D53B9A68E5ADD456686E63418EE8E3B5C1A8239A13E277` |
| Storage preflight record | `A03650CC523422839F7846CE4F88C36963BE9A8D1D768EB6812A6FE675F09537` |
| Focused cleanup regressions, 11 tests | `9349A21880DE922C9EA7C61EFA028542C8B2D6874FC641F83AB345AF266A15D8` |
| Focused capture regressions, 25 tests | `D8867B18C55EA0B0F522AB1B8A1ABCC6DE55F63A98E87F4A6E63790B147EAC97` |
| Default exact-head CI record | `F604B01679B051DB936B1BE05216F41DF1818FDD8CD5817FFE5C1AED6CEAEB06` |
| Default exact-head CI full log | `05B6405AEB53FB8593EFE7737B890145FC1D46B0971BC5450BE5794E92786AD6` |
| Default exact-head conformance provenance | `6347679C7CCFDE71301159FC418C6E513C19995B5FEF208AFB5F41BAF12A14BA` |
| External-target exact-head CI record | `CC3EC1A3B0EDA7DDE41FCF8E955E4A4896A59AABAF88C01401E084643EA9C1FA` |
| External-target exact-head CI full log | `37ADD7168C31A557F73288B8E11B9436339D21DF00D68AD35BED96F06E899A0D` |
| External-target exact-head conformance provenance | `E46813ABF0197FECD6DBA6136BFC08AA117E3CDB73B28F4A1A717E56DC5E544F` |
| Final canonical checks after acceptance reconciliation | `F30A611C24AF974CE3B786D478F3A4EBDA5EFE79C1D650DA30AE2D8CFC22B675` |
| Canonical checks after the evidence-record update | `EB6AF20E6A2DD189F6E524800F2EEE8C3FD6373D53FB46320DA7E7EFF767B0AD` |
| Canonical evidence-run record | `2D5334CA3AC965F1E806F0CAD8A8D137637CB38223BFA645BDCF8592F84846CF` |

The two earlier canonical validation failures are also preserved:
`20261124-081-canonical-final-prepush-c78f015.log` records the stale-dashboard
check, and `20261124-081-canonical-final-c78f015.log` records evidence envelope
and ID-reference errors. Both were resolved by regeneration and evidence-ID
reconciliation; the final canonical log above and post-record check passed.

## WI079 preparation and remaining gate

The read-only PR #18 conflict and integration inventory is
`wi079-pr18-rebase-inventory-2026-11-24.md`. It names the semantic manifest,
identity, documentation mappings, and dashboard as generated conflict points;
the research metadata change requires review of the additional PR18 research
document before any review date is advanced. The PR18 branch was not rewritten.

WI081 remains active because AC-8 still requires the human operator's explicit
integration decision for Workbench/Fabric ownership. WI080 AC-6 and AC-10 remain
blocked; these WI081 results do not establish the historical Rust failure's
cause. Hosted verification and publication remain disabled through December
1, 2026 inclusive.

## Subsequent independent acceptance and completion transition

On 2026-11-24 the independent GitHub review of exact PR #22 source and evidence approved **RepoPact Core-only** cleanup integration while explicitly denying implicit Workbench or Fabric adoption. PR #22 merged at `5d185faac83b68596715ce99824dd4b717eb104e` (tree `60647f7aa6203e68f582c59cb2dc4071f94d6222`). The source verification and evidence from the previous sections remain unchanged, including their originally pending AC-8 status *at the time of validation*. The later decision is durably recorded as evidence run `20261124-081-ac8-core-only-integration-review`, now linked to AC-8.

This PR #23 candidate moves all five WI081 files without discarding the historical validation records and marks status `completed` only after all eight criteria reached `satisfied`. The final move/dashboard and exact-head verification are separate acceptance gates; until completed and merged, this paragraph is a **candidate transition**, not an assertion of a new successful test run. WI080 remains blocked on AC-6/AC-10 and local-only CI continues through December 1, 2026 inclusive.
