# WI076 — Engine Semantic Compatibility Identity

> **Status**: Active — the compatibility defect is governed here; implementation and acceptance evidence remain outstanding.
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
operation response is too late for writes. The proposed correction is a
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
Maturin source/wheel checks, and focused no-mutation tests. It remains
**proposed** pending the tooling owner's explicit approval of the admission
boundary, input closure, migration boundary, and archive/provenance plan. No
implementation has started. Stable 3.1.3 remains unchanged, WI076 remains
active, and WI074 RPS-016 remains pending.

## Implementation clarifications for tooling-owner approval (2026-10-24)

The mandatory Python runtime currently declares `jsonschema>=4.20`, with no
complete transitive lock; optional `cryptography>=43` supports retained
Python-side signing/admission behavior. Before implementation, the tooling
owner must choose either an enforced exact runtime dependency closure (with
installed versions checked before dependent operations) or explicitly bounded
supported versions (with bounds in the identity, runtime enforcement, and
semantic evidence). Optional dependencies that affect a Python-authoritative
surface need the same treatment or a separately documented compatibility
boundary. Hashes used to lock dependency artifacts do not authenticate the
installed engine or package.

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

These clarifications are design questions for the tooling owner, not approval
and not implementation evidence. RepoPact governance now assigns the
WI076/Decision 0077 architectural approval role to GitHub account
`JeremyShows`; that scoped assignment is not approval. Because this account
authored PR #12, it cannot approve its own GitHub review. The four architecture
choices must instead be explicitly accepted in an operator-authored PR
conversation comment and linked from governed evidence. Independent GitHub
implementation review remains separate. Decision 0077 remains **proposed**;
WI076 remains active; AC-2 through AC-6 remain pending; WI074 RPS-016 remains
pending.

The scoped role assignment and its distinction from approval are recorded in
[20261024-076-tooling-owner-assignment](../../../evidence/runs/20261024-076-tooling-owner-assignment.json).

AC-1 is satisfied as a recorded source-level architecture comparison, not as
owner approval or implementation. The added dependency and Python mutation
clarifications remain approval gates, not completed design approval. AC-2
through AC-6 remain pending. AC-7 is
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

## Build and storage plan

No build or full suite was run for the registration or this architecture review. The 15:22 UTC snapshot found 2,242,760,704 bytes free and an unrelated `fabric-qualification` Cargo test process tree. A fresh 15:48 UTC read-only snapshot found C: healthy NTFS with 16,077,275,136 bytes free (14.97 GiB); the process query found no Cargo or rustc build process. Current free space is still below the recorded 18 GiB minimum, so compilation remains blocked by storage. No cleanup was performed. The denied temporary repository and unrelated ForgeWire tree remain untouched.

For a cold engine/workspace test build, reserve a conservative planning ceiling of 12 GiB for target output, 2 GiB for temporary/wheel output, and 4 GiB of headroom; require at least 18 GiB free and a fresh process check with no competing build. The selected task-owned paths are `C:\Projects\repopact-core-wi076-build\target` for `CARGO_TARGET_DIR` and `C:\Projects\repopact-core-wi076-build\tmp` for `TMP`/`TEMP`; the build root does not exist yet. Do not compile below that capacity or while unrelated Cargo work is active. If capacity recovers, reuse that target across focused runs. If cleanup is later needed, remove only that task-owned target after outputs are recorded and all of its processes have exited; retain Cargo caches and every unrelated target.
