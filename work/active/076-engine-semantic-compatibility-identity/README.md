# WI076 — Engine Semantic Compatibility Identity

> **Status**: Active — the compatibility defect is governed here; the compatibility fix and complete acceptance evidence remain outstanding.
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
