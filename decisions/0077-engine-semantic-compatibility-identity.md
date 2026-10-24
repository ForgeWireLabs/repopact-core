---
id: 0077
title: Engine semantic compatibility identity
status: proposed
date: 2026-10-24
supersedes: []
---

# 0077: Engine semantic compatibility identity

## Context

The Python gateway in `repopact/engine_client.py` checks the engine protocol
name/major and exact product version, but it does not identify the validation
semantics implemented by the executable. Its current `_handshake()` starts a
process, and each later `call()` starts another process. The Rust engine reads
stdin to EOF as one request. A response-only identity check would discover a
stale engine after that request had already run; for `dashboard.write`,
`work.create`, graph updates, and other mutating operations, that is too late.

The recorded pre-merge `3.1.3.dev1` executable passed repository validation
but accepted two mapping conformance cases (41/43). Integrated-source
conformance was separately recorded at 43/43, but its evidence has no
executable digest. The 41/43 run used `REPOPACT_ENGINE`; these records show a
same-product semantic-skew risk, but do not prove identical binaries or a
default-discovery reproduction.

## Proposed decision

Keep four independent claims in protocol and conformance evidence:

1. **Protocol identity**: request/response wire compatibility (protocol name
   and major version).
2. **Product identity**: the claimed package/engine release version.
3. **Semantic-contract identity**: the versioned validation contract expected
   by the Python client and implemented by the Rust engine.
4. **Executable authenticity**: whether exact executable bytes came from an
   independently trusted source. The semantic identity does not establish
   this fourth claim.

The semantic identifier format is
`repopact-semantic-v1:sha256:<lowercase-hex>`. A new format requires an
explicit versioned migration; unknown formats fail closed at the strict
enforcement boundary.

### Authoritative input closure

The identity covers the following reviewed input set. The generator discovers
the actual set from the source tree and dependency metadata; it does not trust
a hand-maintained per-file list as the source of truth.

- **Python contract and packaged data:** every tracked runtime `.py` file under
  `repopact/` (currently 41), all `repopact/schemas/*.json`, and all packaged
  templates. This includes the engine resolver, Python-side request/response
  validation and operation wrappers, the conformance runner and its reference
  oracle. The exact Python runtime dependency closure is represented by a
  hash-pinned lock/export; an unconstrained semantic runtime dependency makes
  the source/wheel identity check fail rather than being silently omitted.
- **Rust implementation:** the transitive local Cargo dependency closure of
  `repopact-engine`, including normal, build, and target-specific dependencies.
  The current closure is `repopact-engine`, `repopact-analysis`,
  `repopact-core`, `repopact-graph`, `repopact-mutation`,
  `repopact-protocol`, `repopact-repository`, `repopact-schema`,
  `repopact-types`, and `repopact-validation`. Include each closure member's
  `Cargo.toml`, all Rust source and build scripts, and every file referenced
  by `include_str!`, `include_bytes!`, or equivalent compile-time inclusion.
  Include the workspace `rust/Cargo.toml` and `rust/Cargo.lock`, whose locked
  registry dependencies carry their package checksums.
- **Protocol declarations:** include `repopact-protocol` DTOs and
  `capabilities()` declarations, including its advertised operations and
  `semantic_surfaces`. These declarations are part of the client/engine
  semantic contract even though the wire protocol name and major remain a
  separate identity.
- **Embedded schemas:** include all packaged schema files and verify the
  compile-time embedded set. The current Rust `repopact-schema` embeds
  `evidence-run.schema.json`, `documentation-mappings.schema.json`,
  `frozen-surface.schema.json`, `invariants.schema.json`,
  `work-item.schema.json`, `adopter-fleet.schema.json`,
  `verification-profile.schema.json`, and `assurance-mapping.schema.json`.
- **Conformance contract:** include the complete tracked `conformance/` tree,
  root `SPEC.md`, and every fixture byte. This includes the manifest, valid and
  invalid cases, overlays, and their metadata; it prevents a binary from
  advertising the expected identity while omitting a governed conformance
  input. Adopter repository records and repository-local schemas are
  per-operation data, not executable inputs; conformance fixture bytes are
  covered here, while adopter runs record the repository revision or tree
  digest separately.
- **Identity/build rules:** include the reviewed input-policy file, identity
  generator, engine `build.rs` files, `.gitattributes`, and the semantic
  projection of `pyproject.toml` (`[build-system]`, runtime dependency
  declarations, `requires-python`, and `[tool.maturin]` inclusion settings).
  The hash-pinned Python runtime lock/export is also an input. That lock does
  not currently exist and must be added before package identity can be
  accepted. Product-version values (`VERSION`, `RELEASE_LABEL`, and
  `project.version`) are recorded separately and excluded from the semantic
  digest; build scripts' logic remains an input.

This is deliberately fail-closed and may change the identifier for a
behavior-preserving refactor. The generator must compare its discovered path
set with the stored manifest in both directions. New governed files, removed
files, missing files, changed contents, an unresolved local dependency, or an
unlocked or out-of-bound semantic Python dependency make `--check` fail. The
source closure must also be reviewed against actual Cargo metadata and Maturin
package contents during implementation; this Decision records the current
closure, not a claim that package verification has already run.

### Python runtime dependency consistency — owner choice required

The current mandatory dependency is `jsonschema>=4.20` in both `pyproject.toml`
and `requirements.txt`; this is a lower bound, not a deterministic runtime
closure, and no lock/export currently records its transitive packages. The
optional `cryptography>=43` dependency is used by retained Python-side
signing/admission workflows. Its behavior must not be silently described as
covered by the mandatory runtime dependency declaration.

Before implementation, the tooling owner must choose and approve one explicit
consistency boundary for each dependency that can affect the claimed semantic
surface:

1. **Enforce an exact closure (recommended):** add a canonical lock/export for
   the complete runtime dependency closure, including versions and artifact
   hashes; include it in the semantic-input manifest and sdist; and verify the
   installed distribution name/version set before any dependent operation is
   admitted. A mismatch fails before the operation or any dependent repository
   write. Build/install hashes constrain the selected package inputs; they do
   not authenticate installed bytes or replace independent executable/package
   authenticity checks.
2. **Explicitly bound supported versions:** record the complete supported
   version ranges for the direct and transitive dependencies, include those
   bounds in the semantic identity, enforce them against the loaded runtime,
   and provide focused semantic tests for the declared support boundary. A
   version floor alone, or a range without runtime enforcement and evidence,
   is not an accepted bound.

For optional dependencies such as `cryptography`, either apply one of those
boundaries to the Python-authoritative admission/signing surface or explicitly
separate that surface from the Rust engine semantic claim and document its own
compatibility boundary. Do not silently omit an optional package that can
change retained Python-side behavior.

### Deterministic manifest and build behavior

The proposed canonical generator emits a checked-in semantic-input manifest at
`build/semantic-input-policy.json`,
`scripts/generate_engine_semantic_identity.py`, and the generated
`repopact/_semantic_identity.json`. The policy describes governed roots and
exclusions; the manifest stores the discovered file set and computed ID.
The source/wheel identity is read from that manifest, while product version
continues to come from the existing product-version channel.

The input list is sorted by normalized repository-relative UTF-8 path, uses
`/` on every host, rejects paths escaping the repository and names containing
NUL or newline, and hashes raw file bytes. Governed text inputs use
`.gitattributes` with LF checkout bytes; the generator does not normalize
bytes. Absolute paths, timestamps, product-version values, protocol name/major,
repository worktree metadata, and target outputs are excluded. The identity is
SHA-256 over the bytes `repopact-engine-semantic-inputs\0v1\0`, an unsigned
64-bit big-endian entry count, then each sorted entry as an unsigned 32-bit
big-endian UTF-8 path length, path bytes, and the 32 raw bytes of the file
SHA-256. The manifest stores readable lowercase hex digests and this computed
identifier.

The generator source and policy are inputs. The generated manifest and the
digest embedded in it are outputs and are excluded from the hashed input set,
so there is no self-reference. A known-answer test fixes the canonical
encoding. `--check` only recomputes and compares; it never writes. A separate
explicit `--update` operation may write the manifest, but no build, test,
wheel, or source-distribution command invokes it. A baseline change therefore
appears as a reviewed source diff and cannot be silently advanced by CI or a
local build.

Cargo's build check and the Maturin source-build path must both fail on a stale
manifest and detect an input-set difference. The source distribution includes
the generator, policy, lock data, all files needed to rediscover the closure,
and the identity manifest. The wheel includes the same identity manifest with
the Python package and engine. Tests inspect both archives and prove that a
source build and wheel expose the same identity for the same inputs. Maturin
include patterns in `pyproject.toml` are part of this package audit. No build
path updates the manifest implicitly.

The conformance identity inputs are the normative specification, manifest,
runner/reference implementation, and complete fixture corpus. Test source such
as `tests/test_engine_client.py` is acceptance evidence rather than a runtime
identity input; it is still mandatory validation and cannot be used to advance
the identity baseline.

### Side-effect admission: same process, same image

The selected design is a stateful, line-delimited session owned by one engine
process. The client may reuse that session for operations in its lifetime, but
an admission result is bound to that process instance only:

1. Start one engine process and send only a non-mutating `handshake` request.
   The response reports protocol identity, product identity, semantic
   identity, and capabilities.
2. Verify all expected identities and required capabilities in Python. No
   repository operation has been sent at this point.
3. Send an explicit `admit` request to that same process, echoing the expected
   semantic identity. The Rust session compares it with its build-time
   identity, then returns an admission acknowledgement. Until admitted, the
   engine refuses every operation before dispatch, including every mutating
   operation.
4. Only after verifying that acknowledgement does the client send an
   operation. The same already-running process that passed both checks
   executes it. Every response still carries the identities and is checked,
   but response checking is a consistency check, not the side-effect barrier.

The client must never treat `check_compatibility()` from one process as
authorization for an operation launched in a later process. A process exit
requires a fresh handshake and admission before any later operation. If a
process exits after an operation was sent and its result is unknown, report an
unknown outcome and do not retry that operation automatically. Replacing the
executable at its path after process start does not replace the image already
executing in that process. Serialize requests so only one is in flight on a
session; a response cannot be matched to the wrong operation.

Legacy behavior fails before mutation. The current one-request engine waits
for stdin EOF; the new client keeps the session pipe open while awaiting the
handshake, so a legacy binary times out and is terminated before any operation
is sent. A legacy binary that does answer a handshake but omits the semantic
identity is rejected before `admit`. A binary that ignores newly added JSON
fields is not trusted to reject itself: the client sends no mutating request
until it has verified the handshake and the same-process admission
acknowledgement. The engine-side unadmitted state is a second guard for new
engines. Ordinary Scripts-directory discovery, an executable beside the
Python package, and `REPOPACT_ENGINE` all use the same admission rule; an
explicit override selects a path but cannot bypass it.

A separate handshake process followed by a second executable launch is
rejected as unsafe: the second launch may resolve to different bytes and may
mutate before the client sees its response. Checking an identity only in an
operation response is also rejected because that check occurs after side
effects.

### Retained Python-side mutation workflows — audit clarification

A read-only source audit of the current `repopact/*.py` tree found that the
repository-writing entry points `adopt_repo.adopt`, `doctor.fix`,
`init_repo.bootstrap`, `new.new_markdown`, `plan_import.import_plan`,
`takeover.takeover`, and `verification.record_evidence` perform Python-side
filesystem writes. Several call `EngineClient.check_compatibility()` before
writing, but that method currently handshakes in a separate process and does
not admit the later engine process; `takeover` also performs validation in a
separate process before Python rewrites or retires files. Those checks are
ordering hints only and do not satisfy this Decision's admission boundary.
Their implementation must retain an admitted session before the first
repository write and reuse that session for any later canonical engine
operation. The inventory also includes `generate_spec.write_spec`, which
currently writes `SPEC.md` directly without a compatibility gate; it must be
classified as a Python-authoritative generator with its own compatibility
contract or establish the same admission before changing repository content.
On mismatch, timeout, or failed admission, each canonical-dependent workflow
must have made no repository changes.

Engine-backed mutations such as `work.create` and graph operations must use
the operation path of that same admitted process. Low-level Python writers
such as `adopt_repo.Report.write/json`, `generate_dashboard.write_dashboard`,
and the canonical-record helper are callable below the CLI layer; their call
sites must be audited so a direct entry point cannot bypass the workflow's
pre-write admission requirement. The direct dashboard generator is not a
substitute for canonical Rust dashboard generation.

Decision 0042 keeps WI050 admission/guard, protected-service, platform-backend,
cryptographic-approval, and operator-authority semantics Python-authoritative
and outside the canonical Rust engine protocol. These paths, including the
CLI admission request/receipt writer, Python admission record/signing writers,
and platform install/remove paths,
must be classified explicitly during implementation: either require the
same compatibility admission before repository writes that depend on the
canonical engine, or remain a separately governed Python authority with its
own pre-write checks and compatibility contract. Release/package output
writers are likewise separate from repository governance mutations. This
classification is not evidence that those independent paths have been
revalidated or accepted.

Focused tests must exercise stale or legacy engines against each retained
canonical-dependent Python mutation entry point and compare the repository
before and after, proving rejection occurred before the first write. Tests
must also prove any explicitly separate Python-authoritative path follows its
declared boundary. The earlier one-shot `check_compatibility()` calls cannot
be counted as admission evidence.

### Migration and compatibility boundary

Do not change or rebuild the released stable 3.1.3 Python/engine pair under
its existing identity. Introduce strict session admission only in a distinct,
governed post-3.1.3 development/product identity and ship the client and
engine together in its wheel. A new strict client paired with a legacy engine
times out or rejects the missing identity before sending an operation. A
future strict engine rejects a legacy one-shot operation before dispatch;
cross-version pairings may fail closed, but the existing matched stable 3.1.3
installation continues to behave as released. There is no `REPOPACT_ENGINE`
compatibility bypass for canonical operations.

### Packaging, provenance, and limits

For every canonical conformance run, record the source commit, product
version, protocol name/major, semantic identity format/value, resolved engine
path, process ID and OS file identity, and executable SHA-256. Bind the hash to
the actual process image file identity, measure before the first conformance
operation and after the last response, and mark provenance incomplete if the
image identity or digest cannot be correlated or changes. Run the canonical
suite through one admitted `EngineClient` session where practical; otherwise
record each engine process and its digest. Existing evidence lacking an
executable digest remains historical and is not rewritten.

The semantic digest detects accidental source/input skew when the engine
reports it honestly. It is not proof that a malicious executable reports a
truthful identity, that a binary was built from a claimed revision, or that
the bytes came from a trusted publisher. Those claims require separate,
independently trusted artifact signatures/provenance and approval. A source
identity plus an executable SHA-256 is traceability evidence, not
authentication.

## Focused acceptance tests required before implementation can be accepted

- Matching identity and capabilities are admitted; an operation runs only
  after the handshake and admission responses have been checked, in the same
  PID/process session.
- A same-protocol, same-product stale engine with a different semantic ID is
  rejected before any operation request. Snapshot a temporary repository
  before and after and prove it is byte-for-byte unchanged.
- Missing, malformed, unsupported-format, or mismatched identities are
  rejected before operation dispatch. A legacy fake engine that ignores extra
  JSON fields must record that it received no mutating operation.
- Cover normal Scripts discovery, explicit `REPOPACT_ENGINE`, and a stale
  executable physically beside the Python package. All must leave the
  temporary repository unchanged.
- Simulate replacing the selected executable path after handshake. Assert the
  client neither launches a second process nor sends the operation to the new
  path; only the admitted process instance can receive it. Include an engine
  exit between admission and dispatch and prove the client does not resend a
  mutating request automatically.
- Exercise the engine-side state machine: operations before admission and
  mismatched admission IDs are rejected before filesystem mutation.
- Prove deterministic input discovery, exact manifest membership, new and
  omitted file detection, known-answer hashing, no implicit baseline update,
  and agreement among checkout, sdist, and wheel.
- Preserve the released stable 3.1.3 pair and verify the explicit future
  product-version boundary; do not infer compatibility from protocol version
  alone.
- For each conformance run, assert the recorded executable SHA-256 and OS file
  identity identify the image used, alongside protocol, product, semantic ID,
  and source revision.

## Alternatives considered

- **Check protocol/product only** — rejected because the observed stale engine
  matched both while accepting two more conformance violations.
- **Check semantic ID only in each operation response** — rejected because
  the operation may have mutated repository state before the response is
  checked.
- **Handshake in one process, then relaunch for the operation** — rejected
  because path replacement between launches is a TOCTOU race.
- **Manually increment one semantic integer** — rejected as the sole source
  because it can be forgotten when behavior changes.
- **Use executable SHA-256 as semantic identity** — rejected because it
  conflates input compatibility with target-specific artifact bytes.
- **Allow `REPOPACT_ENGINE` to bypass identity/admission** — rejected because
  the override reproduced stale behavior and canonical operations must remain
  fail-closed.

## Owner approval gate and status

This record remains **proposed**. Before implementation, the tooling owner
must explicitly approve (1) the same-process handshake/admission sequence and
engine-side pre-dispatch gate; (2) the complete input closure and the selected
exact-lock or explicitly-bounded Python dependency rule; (3) the retained
Python-side mutation classification and required pre-write session behavior;
(4) the strict post-3.1.3 migration boundary; and (5) the Maturin archive and
process-image provenance plan. Record that approval in the governed
decision/evidence flow; a successful governance check or this review is not
owner approval. Until then, WI076 remains active and implementation criteria
remain pending. WI074 RPS-016 remains independently pending.

## Pre-build gate

The latest read-only snapshot at 15:48 UTC found C: healthy NTFS with
16,077,275,136 bytes free (14.97 GiB); no `cargo.exe` or `rustc.exe` process
was found. The earlier 15:22 UTC snapshot had an unrelated
`fabric-qualification` Cargo test process tree. The recorded conservative
minimum is 18 GiB free (12 GiB target + 2 GiB temporary and wheel output + 4
GiB headroom). Do not compile while below that threshold or while unrelated
Cargo work is active. If later authorized by capacity,
recheck both conditions and use only the task-owned
`C:\Projects\repopact-core-wi076-build\target` and `...\tmp` paths. Do not
clean Cargo caches, existing targets, the previously denied temporary
repository, or unrelated ForgeWire trees. Only that task-owned target may be
considered for cleanup after its evidence is recorded and its build processes
have exited.
