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

No build or full suite was run for the registration or this architecture review. The latest read-only storage check found C: healthy with 2,343,563,264 bytes free (2.18 GiB), no alternative volume, no active Cargo/rustc process, and no Core target directory. No cleanup was performed; the previously denied temporary repository and unrelated ForgeWire tree remain untouched. For a cold engine/workspace test build, reserve a conservative planning ceiling of 12 GiB for target output, 2 GiB for temporary/wheel output, and 4 GiB of headroom; require at least 18 GiB free and a fresh process check with no competing build. If that capacity is unavailable, do not compile. If recovered, isolate `CARGO_TARGET_DIR` at `C:\Projects\repopact-core-wi076-build\target` and `TMP`/`TEMP` at `C:\Projects\repopact-core-wi076-build\tmp`; create these directories only after the capacity gate passes. Reuse that task-owned target across focused runs. If cleanup is later needed, remove only that target after outputs are recorded and all of its processes have exited; retain Cargo caches and every unrelated target.
