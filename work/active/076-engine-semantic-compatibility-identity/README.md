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

The source review found that `repopact/engine_client.py` checks protocol and
product version but not semantic identity. It caches handshake capabilities,
then starts a new engine process for each operation. Therefore an identity
check must apply to every response, not only to the handshake, to reject an
engine replaced between calls. Default Scripts-directory discovery and
`REPOPACT_ENGINE` both use the same client path and should enforce the same
compatibility rule.

The proposed design is recorded in [Decision 0077](../../../decisions/0077-engine-semantic-compatibility-identity.md).
It remains **proposed** pending owner review; no implementation has started.
The proposal keeps the stable 3.1.3 installation pair unchanged and makes the
strict identity contract effective only with a later product version. WI074
RPS-016 remains pending.

AC-7 is satisfied by the merged registration: WI074 names WI076 as supporting
RPS-016 evidence and explicitly keeps RPS-016 pending. The independent check is
recorded in [20261024-076-rps016-link-registration](../../../evidence/runs/20261024-076-rps016-link-registration.json).
The other seven criteria remain pending; this governance linkage is not an
implementation or compatibility fix.

## Build and storage plan

No build or full suite was run for the registration or this architecture review. The latest read-only snapshot at 15:22 UTC found only C: (healthy NTFS), with 2,242,760,704 bytes free (2.09 GiB). One unrelated `fabric-qualification` Cargo test was active (`cargo test --workspace --locked --features fabric-hub/test-harness`, represented by its parent and child Cargo processes). No Core target exists, and `CARGO_TARGET_DIR` is unset. The denied temporary repository and unrelated ForgeWire tree remain untouched; no cleanup was performed.

For a cold engine/workspace test build, reserve a conservative planning ceiling of 12 GiB for target output, 2 GiB for temporary/wheel output, and 4 GiB of headroom; require at least 18 GiB free and a fresh process check with no competing build. The selected task-owned paths are `C:\Projects\repopact-core-wi076-build\target` for `CARGO_TARGET_DIR` and `C:\Projects\repopact-core-wi076-build\tmp` for `TMP`/`TEMP`; the build root does not exist yet. Do not compile with current capacity or while the unrelated Cargo test is active. If capacity is recovered, reuse that target across focused runs. If cleanup is later needed, remove only that task-owned target after outputs are recorded and all of its processes have exited; retain Cargo caches and every unrelated target.

AC-7 and AC-8 are satisfied with linked evidence. The other six criteria remain pending; neither criterion claims an implementation or compatibility fix.
