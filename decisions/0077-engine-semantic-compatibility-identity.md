---
id: 0077
title: Engine semantic compatibility identity
status: proposed
date: 2026-10-24
supersedes: []
---

# 0077: Engine semantic compatibility identity

## Context

The Python client in `repopact/engine_client.py` verifies the engine protocol
name/major and exact product version. It does not currently verify the
validation semantics advertised by the engine. The pre-merge 3.1.3.dev1
executable passed repository validation but accepted two mapping conformance
cases (41/43); the integrated-source run passed 43/43 but did not record its
executable digest. These runs demonstrate a same-version semantic-skew risk,
but do not prove the binaries were identical or establish a default-discovery
reproduction.

The client caches handshake capabilities and starts a separate process for
each operation. A handshake-only identity check would leave later calls
unchecked if the executable at the selected path changed. The ordinary Scripts
directory lookup and `REPOPACT_ENGINE` explicit path converge on the same
`EngineClient`; an override currently changes selection, not the protocol or
product-version checks.

## Proposed decision

1. Define a semantic-contract identity independently from the protocol name and
   version, product version, executable digest, and source revision. Use the
   format `repopact-semantic-v1:sha256:<lowercase-hex>`.
2. Generate the digest from a versioned, checked-in input manifest. For the
   current engine, the Rust source closure is `repopact-engine` plus its local
   semantic crates `repopact-analysis`, `repopact-core`, `repopact-graph`,
   `repopact-mutation`, `repopact-repository`, `repopact-schema`,
   `repopact-types`, and `repopact-validation`. The manifest also covers the
   built-in schemas embedded by `repopact-schema`, the conformance manifest and
   fixtures, and the relevant Cargo manifests and lockfile. The protocol DTO
   crate is excluded because transport compatibility has its own protocol
   identity. Exclude timestamps, absolute paths, product version, protocol
   version, and target-specific build outputs. Repository-local schemas remain
   validation inputs supplied by each repository, not part of the engine
   binary identity.
3. Compute the ID using this byte sequence: the UTF-8 domain prefix
   `repopact-engine-semantic-inputs\0v1\0`, then for each input sorted by its
   normalized repository-relative UTF-8 path, append the path, NUL, lowercase
   hexadecimal SHA-256 of the raw file bytes, and LF. Paths use `/` on every
   host and may not escape the repository. A `.gitattributes` rule must keep
   hashed text inputs at LF so Windows and Unix builds hash the same source
   bytes; do not normalize file contents in the identity tool itself. A
   generator updates the manifest; the Rust build script verifies that its
   listed input hashes match the source tree and embeds the manifest's ID.
   Python reads the same packaged manifest. The current Maturin configuration
   explicitly includes selected files in sdist and wheel; implementation must
   add the identity manifest to both and include every source input needed to
   validate it from an sdist. Maturin supports explicit sdist/wheel includes
   ([configuration guide](https://www.maturin.rs/config.html)).
   A stale manifest fails a source build instead of silently producing an
   engine/client pair with different identities.
4. Put the semantic identity in every engine response. The client validates
   protocol, product version, and semantic identity on every response. A
   missing, malformed, unsupported-format, or mismatched identity fails closed
   before the operation result is used. `REPOPACT_ENGINE` selects a path only;
   it never bypasses compatibility checks. No incompatible-engine escape hatch
   is available to canonical operations.
5. Preserve the released stable 3.1.3 Python/engine pair unchanged. Strict
   semantic-identity enforcement is introduced for a later product version;
   do not publish a rebuilt artifact under the stable 3.1.3 identity. Existing
   stable 3.1.3 users keep their current matched pair, while a newer client
   rejects an older engine through the existing exact product-version check or
   the missing-identity check. Unknown identity formats fail closed; migrating
   the identity format requires an explicit versioned transition.
6. For each new conformance run, record the exact source revision, executable
   SHA-256, product version, protocol name/major, semantic identity format and
   value, and focused validation result in the evidence run's `environment`.
   Historical evidence stays immutable. This semantic fingerprint detects
   accidental input-set skew; it does not prove semantic equivalence or
   authenticate executable bytes. Trusted-artifact verification remains a
   separate design.

The input list must be checked against the engine's actual local dependency
closure and package contents before implementation is approved. Any hash over
source and conformance inputs is conservative: a behavior-preserving refactor
of an included file changes the identity and may reject an otherwise compatible
engine. That is an intentional fail-closed trade-off for this first version.

## Required implementation evidence

- Matching engine identity is accepted.
- A stale engine with the same protocol and product version but a different
  semantic identity is rejected through ordinary Scripts-directory discovery
  and through `REPOPACT_ENGINE`.
- Missing, malformed, unsupported-format, and mismatched identities are
  rejected; validation also covers a stale engine placed beside the Python
  package and a path replacement between handshake and operation calls.
- The source tree, source distribution, and wheel expose the same identity for
  identical semantic inputs.
- A conformance run records executable SHA-256 immediately before and after the
  run and is not accepted as provenance-complete if the digest changes.
- Stable 3.1.3 remains unchanged, and the release boundary for strict
  enforcement is explicit.
- New conformance evidence records executable and source provenance without
  presenting the semantic identity as binary authenticity.

## Alternatives considered

- **Protocol and product version only** — rejected because both matched in the
  observed stale-engine run while behavior differed.
- **One manually incremented semantic integer** — rejected as the sole input
  because it can be forgotten when semantics change under an unchanged product
  version.
- **Executable SHA-256 as the semantic identity** — rejected because it varies
  by target/build and conflates semantic compatibility with artifact identity;
  record it separately.
- **Allow `REPOPACT_ENGINE` to bypass the check** — rejected because the
  existing explicit override reproduced stale behavior and canonical
  operations must remain fail-closed.

## Consequences and pre-build gate

No implementation or compilation was performed during this review. At the
latest read-only snapshot, C: was healthy with 2,343,563,264 bytes free
(2.18 GiB); no alternate volume, active Cargo/rustc process, or Core target
directory was present. For a cold engine/workspace test build, use a
conservative planning ceiling of 12 GiB for target output, 2 GiB for
temporary/wheel output, and 4 GiB of headroom; require at least 18 GiB free and
recheck competing processes before any compilation. If that capacity is not
available, do not compile. If recovered, isolate `CARGO_TARGET_DIR` at
`C:\Projects\repopact-core-wi076-build\target` and `TMP`/`TEMP` at
`C:\Projects\repopact-core-wi076-build\tmp`, creating those directories only
after the capacity gate passes. Reuse that task-owned target across focused
runs. Do not clean Cargo caches, existing targets, the previously denied
temporary repository, or unrelated ForgeWire trees. Only the task-owned WI076
target can be considered for cleanup after outputs are recorded and all
associated build processes have exited.

WI074 RPS-016 remains pending after WI076 completes until its separate release,
compatibility matrix, offline installation, upgrade, and rollback criteria are
independently proven.
