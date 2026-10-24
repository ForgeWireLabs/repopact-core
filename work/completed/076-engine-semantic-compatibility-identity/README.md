# WI076 — Engine Semantic Compatibility Identity

> **Status**: Completed on the PR branch with implementation evidence; PR #12 remains open for independent review and has not been merged.
> **Owner**: tooling-owner (lead); docs-owner and evidence-owner affected.
> **Related issue**: [Core issue #10](https://github.com/ForgeWireLabs/repopact-core/issues/10).
> **Related readiness gate**: WI074 / RPS-016. This work can provide one compatibility input, but it cannot satisfy RPS-016 by itself.

## Intent

Distinguish four identities that the current Python/Rust boundary conflates:

1. **Protocol identity** establishes that the request and response formats can communicate.
2. **Product identity** establishes the claimed release version.
3. **Semantic-contract identity** establishes that the engine implements the validation contract expected by the Python client.
4. **Artifact authenticity** establishes whether executable bytes came from an independently trusted source.

The recorded pre-merge `3.1.3.dev1` engine returned success for canonical repository validation but accepted two mapping fixtures (41/43). Integrated-source conformance was separately recorded at 43/43, but its evidence does not retain an executable digest. The current client checks protocol and product version, not semantic-contract identity. The exact stale-engine result used an explicit `REPOPACT_ENGINE` override; the ordinary resolver uses the Python environment's Scripts directory. The code would accept a same-version stale engine found there, but that default-path stale pairing has not been independently reproduced.

Maturin places the Python package and Rust executables in one wheel, reducing routine mismatch risk. A deterministic semantic identity can detect accidental same-version skew; it is not an integrity check and cannot stop a malicious executable from lying about that identity.

## Scope and constraints

- Define and test a semantic-contract identity separately from the transport protocol and product version.
- Decide migration behavior for older engines and make development override behavior explicit and fail-closed for canonical operations.
- Keep executable authenticity as a separate guarantee requiring independently trusted package or artifact metadata.
- Update compatibility and contributor documentation when behavior is approved.
- Keep WI074 RPS-016 pending until every part of its independent acceptance criterion is met.
- No package or release publication is part of this work.

## Acceptance

The machine-readable criteria are in `work-item.json`. They require an approved identity design before implementation, focused stale-engine mismatch tests, package wiring and compatibility evidence, explicit limits on authenticity claims, and a recorded executable digest for future conformance runs.

## Architecture review (2026-10-24)

The source review confirmed that the current client performs a handshake in
one process and launches another process for each operation. Checking an
operation response is too late for writes. The approved correction is a
same-process JSON-lines session: handshake, client verification, engine-side
admission, then operation on that same process. The client sends no mutation
before checking both the semantic identity and the admission acknowledgement.
A separate handshake process followed by another executable launch is not
considered safe. Legacy one-request engines either time out before answering
while the session pipe remains open or return no semantic identity; neither
case receives a mutating request.

Decision 0077 now records the current local Rust dependency closure, Python
package contract surface, protocol capability declarations, embedded schemas,
conformance corpus, package/build inputs, deterministic manifest rules,
Maturin source/wheel checks, and focused no-mutation tests. The architecture is
approved for implementation as recorded below. Stable 3.1.3 remains unchanged,
WI076 remains active, and WI074 RPS-016 remains pending.

## Implementation clarifications and pre-approval snapshot (2026-10-24)

At the time of this pre-approval snapshot, the mandatory Python runtime declared
jsonschema>=4.20 without a complete transitive lock. Optional cryptography>=43
supported retained Python-side signing/admission behavior. The exact-lock
consistency choice had not yet been approved. Artifact hashes do not
authenticate the installed engine or package.
A source audit found repository-writing Python workflows in `adopt_repo.adopt`,
`doctor.fix`, `init_repo.bootstrap`, `new.new_markdown`, `plan_import.import_plan`,
`takeover.takeover`, and `verification.record_evidence`. Existing
`check_compatibility()` calls are separate-process preflights and cannot
authorize later Python writes. Each canonical-dependent workflow must hold a
same-process admitted engine session before its first repository write and
reuse it for later canonical operations. `generate_spec.write_spec` is an
additional direct repository writer without an existing compatibility gate;
its role in the semantic boundary needs an explicit owner classification.
The call sites of low-level writers must also be checked so they cannot
bypass that entry boundary. Python-owned admission/signing,
protected-service/platform, and release-output paths remain
explicitly classified against Decision 0042 and their separate compatibility
contracts; no conclusion of revalidation is implied.

At the time this pre-approval snapshot was recorded, these clarifications were
design questions rather than approval or implementation evidence. RepoPact governance assigns the
WI076/Decision 0077 architectural approval role to GitHub account
`JeremyShows`; the scoped assignment alone was not approval. Because this account
authored PR #12, it cannot approve its own GitHub review. The four architecture
choices were to be explicitly accepted in an operator-authored PR conversation
comment and linked from governed evidence. Independent GitHub implementation
review remains separate. The later approval record below supersedes the
pre-approval status stated here. WI076 remains active; AC-2 through AC-6 remain
pending; WI074 RPS-016 remains pending.

The scoped role assignment and its distinction from approval are recorded in
[20261024-076-tooling-owner-assignment](../../../evidence/runs/20261024-076-tooling-owner-assignment.json).

Before owner approval, AC-1 was satisfied as a recorded source-level
architecture comparison, not implementation evidence. AC-2 through AC-6
remain pending. AC-7 is
satisfied by the merged registration linking WI076 to RPS-016 without
satisfying that gate. AC-8 is satisfied as a pre-build plan. Evidence is
recorded in [20261024-076-architecture-review](../../../evidence/runs/20261024-076-architecture-review.json),
[20261024-076-admission-architecture-review](../../../evidence/runs/20261024-076-admission-architecture-review.json),
[20261024-076-rps016-link-registration](../../../evidence/runs/20261024-076-rps016-link-registration.json),
and [20261024-076-prebuild-storage-budget](../../../evidence/runs/20261024-076-prebuild-storage-budget.json).
The source-level dependency and mutation audit is recorded in
[20261024-076-python-mutation-dependency-audit](../../../evidence/runs/20261024-076-python-mutation-dependency-audit.json).
These design and governance results do not claim an implemented compatibility
fix.

## Tooling-owner architectural approval (2026-10-24)

The operator explicitly approved all four Decision 0077 choices, selecting an
exact Python runtime dependency lock. Approval was recorded in [PR #12's
conversation](https://github.com/ForgeWireLabs/repopact-core/pull/12#issuecomment-6386671837)
and governed evidence
[`20261024-076-tooling-owner-architecture-approval`](../../../evidence/runs/20261024-076-tooling-owner-architecture-approval.json).
Decision 0077 is accepted for architecture only. Implementation is authorized
to proceed in bounded increments; no Rust or Maturin build is authorized below
the 18 GiB free-space minimum or while unrelated Cargo work is active.
Independent implementation review remains separate. WI076 is active, AC-2
through AC-6 are pending, and WI074 RPS-016 remains pending.

## Implementation increment 1 — deterministic semantic manifest (2026-10-24)

Added the approved hash-pinned universal Python runtime lock, deterministic
input discovery and explicit --check / --update modes, the generated
manifest and package identity file, Maturin source/wheel inclusion declarations,
and focused pure-Python tests. At the close of this increment, the manifest contained 253 inputs,
including 10 local Rust packages in the engine dependency closure and the
compile-time include files. Its semantic ID is
repopact-semantic-v1:sha256:404a0dd4b1d90555174f254d3f1332d4051f5972f42478681f4483ec4d245696.

The focused test module passed 8 tests at this increment. The explicit manifest update completed,
and a subsequent read-only check passed. No Cargo or Maturin build has been run.
AC-4 remains pending until runtime lock enforcement and source/sdist/wheel
identity agreement are implemented and verified; AC-2, AC-3, AC-5, and AC-6
remain pending.
## Implementation increment 2 — Python admission state model (2026-10-24)

Added a pure Python session admission state model. Its focused tests prove that
application operations remain blocked until a matching handshake and same
session admission acknowledgement; missing, stale, or mismatched identities
and wrong acknowledgements fail closed; legacy identity omission never reaches
admission; and process exit invalidates the session. The combined manifest and
state-model test run passed 14 tests. The updated manifest contains 254 inputs;
its current semantic ID is
repopact-semantic-v1:sha256:509be0fdde93a0fc1fb2d164194c2c8c6e00b389e1544f94b4378ecd1b4a9376.

This is a pure state model only. It is not yet connected to EngineClient or the
Rust engine, does not prove subprocess identity, and does not prove repository
snapshot preservation in retained Python write workflows. AC-2 and AC-3 remain
pending; AC-4 remains pending for runtime dependency enforcement and archive
agreement. No Cargo or Maturin build has been run. The dashboard was refreshed from the Python projection; canonical Rust parity remains unverified.

## Build and storage plan

No build or full suite was run for the registration or this architecture review. The 15:22 UTC snapshot found 2,242,760,704 bytes free and an unrelated `fabric-qualification` Cargo test process tree. A fresh 15:48 UTC read-only snapshot found C: healthy NTFS with 16,077,275,136 bytes free (14.97 GiB); the process query found no Cargo or rustc build process. Current free space is still below the recorded 18 GiB minimum, so compilation remains blocked by storage. No cleanup was performed. The denied temporary repository and unrelated ForgeWire tree remain untouched.

For a cold engine/workspace test build, reserve a conservative planning ceiling of 12 GiB for target output, 2 GiB for temporary/wheel output, and 4 GiB of headroom; require at least 18 GiB free and a fresh process check with no competing build. The selected task-owned paths are `C:\Projects\repopact-core-wi076-build\target` for `CARGO_TARGET_DIR` and `C:\Projects\repopact-core-wi076-build\tmp` for `TMP`/`TEMP`; the build root does not exist yet. Do not compile below that capacity or while unrelated Cargo work is active. If capacity recovers, reuse that target across focused runs. If cleanup is later needed, remove only that task-owned target after outputs are recorded and all of its processes have exited; retain Cargo caches and every unrelated target.

## Implementation increment 3 — admitted engine, Python writer audit, and run provenance (2026-10-24)

The strict enforcement boundary is now `3.1.4.dev0`, authorized for this
existing PR #12. `VERSION`, `RELEASE_LABEL`, `pyproject.toml`, the conformance
suite version, and the generated SPEC version block agree; stable 3.1.3 remains
below the strict boundary. Runtime core packages match the exact lock in this
environment. The optional cryptographic group does not: `cryptography` is
50.0.1 (locked 50.0.2) and `pycparser` is 3.0 (locked 3.12), so strict
cryptographic admission/key workflows fail closed here until a matching
environment is provided.

The Python client and Rust engine now implement the same-process handshake,
identity check, explicit admission, and operation sequence. Focused negative
tests cover same-version stale and legacy engines, malformed/unsupported IDs,
Scripts discovery, an executable beside the package, explicit
`REPOPACT_ENGINE`, pre-admission rejection, path replacement, process exit,
uncertain outcomes without retry, and repository snapshots around retained
canonical-dependent writers. The source audit classifies SPEC generation and
admission/protected-state writes as Python-authoritative; they check the exact
Python dependency lock before writing, while cryptographic operations require
the enforcement group too. SPEC rendering reads VERSION, schemas, and
invariants, and does not claim a Rust validation pass.

The canonical conformance runner now keeps one admitted engine process for all
cases and writes a standard evidence-run record with source revision, product,
protocol, semantic identity, embedded Rust toolchain/target/profile, process
image file ID, and pre/image/post executable SHA-256. The runner marks
provenance incomplete for wrappers, unavailable image queries, file identity or
digest changes, dirty source, or a source revision mismatch. A semantic ID and
file digest still do not authenticate a binary against a trusted publisher.
The existing clean-release pipeline now checks every sdist semantic input and
the wheel's identity, manifest, and runtime lock against source before reporting
archive agreement. Synthetic archive tests exercise both acceptance and drift
rejection; no Maturin archive has yet been built.

Focused validation passed: 39 Python tests across engine admission, runtime
dependencies, semantic-manifest generation, archive identity, and package
shape; explicit Python syntax checks; Rust formatting checks; and
`git diff --check`. The combined
`test_conformance` invocation was stopped before completion after confirming it
starts the full published fixture subprocess suite; no result is claimed for
that broader suite. No Rust/Maturin build, canonical Rust conformance run, or
actual archive identity check has run. The storage gate remains the recorded
18-GiB-minimum plus a fresh no-competing-Cargo-process check. AC-2 and AC-3 have
focused source/test evidence but remain pending integrated Rust execution; AC-4
remains pending actual sdist/wheel agreement; AC-5 and AC-6 remain pending final
conformance and documentation evidence. WI076 remains active and RPS-016
remains pending.

The focused results and exact-lock environment findings are recorded in
[`20261024-076-implementation-increment-3`](../../../evidence/runs/20261024-076-implementation-increment-3.json).

## Implementation increment 4 — package-local runtime and identity resources (2026-10-24)

Moved the runtime lock and dependency-group policy under `repopact/` and
declared them with the semantic manifest and identity for both sdist and wheel
inclusion. This keeps the wheel resources inside the existing `repopact`
import root. The source archive also includes the build manifest, policy,
generator, and archive verifier. The archive verifier now requires these
package-local resources and checks the source archive's full semantic input
closure and the wheel's identity/manifest/runtime lock. The prior increment
evidence remains an immutable record of its earlier layout.

The manifest was recomputed for the intended PR tree with 257 discovered
inputs and identity
`repopact-semantic-v1:sha256:637927d4fed3e96f0c0411a4aef52004e6634210c63eb96b7c8c3434321aeadd`.
The two unrelated unstaged Rust edits in `repopact-mutation` and
`repopact-validation` were deliberately represented by their current `HEAD`
blob contents; those edits remain untouched and unstaged. The three generated
identity outputs matched that recomputation. A normal worktree `--check` is
not claimed because those preserved files differ from the PR tree.

Focused validation passed 43 tests across the state model, engine admission,
runtime lock, semantic manifest, synthetic archive verification, and release
package shape. Python syntax, the installed core runtime lock, Rust formatting,
the frozen-surface check, version alignment, and `git diff --check` also passed.
The broader `repopact validate` command did not complete: the Rust engine
handshake timed out after 30 seconds. The cause is undetermined, and this run
is not treated as a validation pass.
No Cargo or Maturin build, canonical Rust conformance run, or actual sdist/wheel
verification was run. The last recorded C: measurement remains below the
18-GiB minimum; no cleanup or build was performed. AC-2 through AC-6 remain
pending, WI076 remains active, and RPS-016 remains pending. Details are in
[`20261024-076-implementation-increment-4`](../../../evidence/runs/20261024-076-implementation-increment-4.json).

## Implementation increment 5 — operator compatibility guidance (2026-10-24)

Updated the packaging guide with the distinct protocol, product, semantic, and
executable-provenance claims; same-process pre-dispatch admission; stale and
legacy-engine behavior; the non-bypassable `REPOPACT_ENGINE` path override;
and migration between stable 3.1.3 and the proposed strict boundary. The guide
links to Decision 0077 for the detailed contract and limits. The docs audit
registry review date was advanced to 2026-10-24 as required by `docs/AGENTS.md`.

The focused documentation suite passed 6 tests and the repository Markdown
link checker passed for 340 files; 39 external GitHub links remain unchecked
by that offline tool. The broader `repopact validate` handshake timeout remains
unresolved, and the canonical `repopact dashboard` command also timed out at
the engine handshake. The derived dashboard was left untouched; it must be
regenerated after canonical engine operation is restored. AC-5 is not marked
satisfied yet. WI076 remains active and AC-2 through AC-6 remain pending. See
[`20261024-076-implementation-increment-5`](../../../evidence/runs/20261024-076-implementation-increment-5.json).

## Pre-build gate recheck (2026-10-24)

A fresh read-only check at 17:41:26 UTC found C: healthy NTFS with
16,221,507,584 bytes free (15.11 GiB), still below the required 18 GiB. No
Cargo, rustc, or Maturin process was active. No build or cleanup was performed.
The task-owned target/temp plan remains unchanged; check capacity and competing
processes again before any build. Evidence is in
[`20261024-076-prebuild-gate-recheck-1741`](../../../evidence/runs/20261024-076-prebuild-gate-recheck-1741.json).

## Implementation increment 6 — transport diagnosis and protocol v2 (2026-10-24)

Read-only inspection resolved the handshake timeout mechanism. Ordinary
discovery selected the global Python 3.12 Scripts executable with no
`REPOPACT_ENGINE` override. Its SHA-256 is
`b9f4004a486dc17ceb91d8af00e821f94183191e5e0aa98743b360d7b79eab1f`; static
PE strings identify product `3.1.0.dev1` and the legacy one-request diagnostic.
The image has no source revision, Rust toolchain, or semantic identity metadata.
The old source reads stdin through EOF; the WI076 client writes a newline and
keeps stdin open while awaiting the handshake, so this executable cannot reply
until after the client timeout. Earlier failed-run records remain unchanged;
they did not capture an executable digest, so the exact historical process
image is not independently correlated.

After manifest regeneration, a separate uncommitted edit was observed in
`rust/apps/repopact-engine/src/main.rs`. It remains untouched and unstaged;
the regenerated manifest uses the committed file for this PR, just as it does
for the earlier unrelated Rust edits.

The session transport is now protocol major 2. Major 1 remains the stable
3.1.3 one-request/EOF protocol; major 2 is newline-framed and keeps one process
alive through handshake, admission, and operations. The client does not
negotiate or downgrade. Focused tests prove a responding protocol-1 engine is
rejected before admission or mutation and an EOF-framed engine times out at the
handshake without receiving an operation. The semantic identity was regenerated
to `repopact-semantic-v1:sha256:70fdf142928e3f6a447bea861833610f3cb9b6cf9dff50e004662bd8c7accb23`.

The focused Python tests passed (44), the offline documentation checker passed
(340 Markdown files; 39 external links unchecked), and the isolated manifest
check passed. At 18:50:18 UTC, C: was healthy NTFS with 15.10 GiB free, 2.90
GiB below the 18 GiB build minimum; no competing Cargo/rustc/Maturin process
was active. No Rust build, conformance run, actual sdist/wheel check, or cleanup
was performed. These validations remain NOT_RUN or BLOCKED; WI076 remains
active, AC-2 through AC-6 remain pending, and WI074 RPS-016 remains pending.
Details are in
[`20261024-076-transport-version-diagnosis`](../../../evidence/runs/20261024-076-transport-version-diagnosis.json).

## Matching-engine validation gate (2026-10-24)

A clean detached source checkout was established at the requested PR commit
`565930190fa6684ffb07ce4ad3ef170a42b9153f`; the original working tree's three
unstaged Rust changes remain untouched. The committed source declares product
`3.1.4.dev0`, protocol major 2, and semantic identity
`repopact-semantic-v1:sha256:70fdf142928e3f6a447bea861833610f3cb9b6cf9dff50e004662bd8c7accb23`.
These are source declarations, not a built-engine identity. At 19:05:16 UTC,
healthy C: had 16,186,093,568 bytes free, 3,141,259,264 bytes below the approved
18 GiB minimum. No competing build was active, but no task-owned target/temp
outputs existed that could be cleaned. Build and integration validation remain
blocked; no build, cleanup, or acceptance transition occurred. See
[`20261024-076-matching-engine-storage-blocker`](../../../evidence/runs/20261024-076-matching-engine-storage-blocker.json).
WI076 remains active, AC-2 through AC-6 remain pending, and WI074 RPS-016
remains pending.

## Implementation increment 7 — matching-engine integration and acceptance evidence (2026-10-24)

After the operator-authorized storage recovery, C: had 30.09 GiB free. The
matching engine was built from clean PR source revision
`7ad445e8af5f97d861d7d4a3ef3d0d1191833e19` into the task-owned target. It
reports product `3.1.4.dev0`, protocol major 2, and semantic identity
`repopact-semantic-v1:sha256:6698dfbb03e39ba96dcea5b76b832813df0de9be9c42418888d0190492cf8e3f`.
The exact process image was
`C:\Projects\repopact-core-wi076-build\runtime-venv-313\Scripts\repopact-engine.exe`,
SHA-256 `ee6b3a21062413c5dad6979e7bb0f558d104c594e4e473c463f7c26b59a8aeac`;
the selected path, process-image path, pre-launch hash, process-image hash,
and post-run hash agree. Rust 1.96.0 built the Windows x86_64 release target.

The final source conformance run passed 43/43 canonical cases and 8/8 WI050
admission vectors. Its evidence records source revision, product and protocol,
semantic and build identities, process-image file ID, and exact executable
digest. A preceding conformance run at source revision
`565930190fa6684ffb07ce4ad3ef170a42b9153f` also has complete executable
provenance. Both run records are summarized in
[`20261024-076-matching-engine-integrated-validation`](../../../evidence/runs/20261024-076-matching-engine-integrated-validation.json).

Focused Rust tests passed 7/7. The WI076 Python suites passed 55/55 under
locked CPython 3.12.4 and 55/55 under 3.13.15. The complete Python suite passed:
434 tests run, three skipped, zero failures, in 1496.461 seconds. Its log and
output digest are retained in the task-owned build directory. An earlier full
3.13 run remains preserved with 10 failures and three skips; the missing
`cryptography` case was an environment issue, and stale fixture versions,
documentation snapshots, evidence timestamp syntax, and ownership coverage
were corrected before the successful full 3.12 run.

Actual Maturin sdist and wheel archives were built and independently checked
against the current source manifest. The sdist contains all 257 semantic
inputs; the wheel includes the identity, manifest, and exact runtime lock. The
source, sdist, and wheel identities agree. Archive hashes are in the evidence
record. This agreement does not establish executable authenticity.

Same-version stale-engine rejection occurred at handshake before admission or
mutation; a ten-file repository snapshot was unchanged. Legacy protocol v1 is
rejected before operation dispatch. An uncertain mutation response test
demonstrates no automatic replay using a simulated peer; it does not prove
crash rollback or recovery. The full suite, canonical repository validation,
frozen-surface check, semantic manifest check, and offline documentation link
check passed. The documentation checker leaves 39 external GitHub links
unchecked.

AC-2 through AC-6 now have implementation and validation evidence recorded in
the machine-readable item. WI076 is completed on this PR branch after those
criteria were evidenced; PR #12 remains open and unmerged for independent
implementation review. RPS-016 remains pending. Stable 3.1.3 remains preserved,
no global Python installation changed, and no package was published. The final
read-only storage check found healthy NTFS with 31,672,033,280 bytes free
(about 29.50 GiB); no Cargo, rustc, or Maturin process was active. No cleanup
was performed by this run. Linux and macOS, artifact authenticity, and real
interruption/rollback acceptance remain outside this evidence.
