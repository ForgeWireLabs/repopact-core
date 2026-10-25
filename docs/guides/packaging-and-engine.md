# Guide: Install and build the RepoPact engine

*Diataxis mode: how-to (task-oriented).*

RepoPact wheels contain the Python compatibility package and a platform-native
`repopact-engine` executable. A wheel install is therefore compiler-free, while
an sdist build is a source build that requires Rust and Maturin.

## Install a wheel

```powershell
python -m venv .venv
.venv\Scripts\python -m pip install repopact
.venv\Scripts\repopact --help
```

The compatibility client only trusts `repopact-engine` beside the active
Python environment's scripts directory. `REPOPACT_ENGINE` is an explicit
development override and is still handshake- and version-checked. It selects
an executable path; it does not bypass compatibility checks or fall back to a
Python validator when the engine is missing or incompatible.

## Compatibility identity and migration

The engine reports separate protocol, product-version, and semantic-contract
identities. Conformance evidence also records the exact executable image and
SHA-256. These claims answer different questions; a semantic identity or file
digest alone does not prove that executable bytes came from a trusted publisher.
See [Decision 0077](../../decisions/0077-engine-semantic-compatibility-identity.md)
for the input closure, versioning, and limits.

At the proposed strict boundary, `3.1.4.dev0`, the client starts one engine,
checks its identities and required capabilities, admits that same process,
then sends operations to it. A missing, legacy, stale, malformed, or mismatched
identity stops an operation before dispatch. Setting `REPOPACT_ENGINE` does
not bypass this admission. The stable 3.1.3 package continues to use its
matching engine under the previous compatibility behavior.

Once an operation request write is attempted, a timeout, interruption,
transport failure, malformed response, or mismatched response poisons the
session. The client terminates and reaps its owned process and does not retry
or replay the request. The operation may already have taken effect, so callers
must treat the result as unknown and inspect repository state before deciding
what to do next. A keyboard interrupt remains a cancellation signal and carries
the uncertain-outcome detail; it is not converted into a success or retried.

This session contract uses engine protocol major 2. Major 1 used one JSON
request per process and read stdin through EOF; major 2 uses newline-framed
requests in one persistent process for handshake, admission, and operations.
The framing and process-lifecycle change is incompatible even though both
versions use JSON objects, so the client does not negotiate or downgrade. A
legacy v1 executable may wait for EOF and time out before it can return a
version mismatch; the client sends no operation before successful v2 admission.
Keep stable 3.1.3 and its v1 engine together when rolling back.

The strict client checks the installed Python core dependencies against the
exact lock shipped in the package before engine admission. A mismatch stops
the operation. Protected admission and signing workflows also check the
separately locked enforcement dependencies before they write or sign records.

When moving to the strict boundary, install the matching Core package and its
colocated engine together. For development overrides, use an engine built from
the same source revision and semantic inputs as the Python package. If a
compatibility error appears, reinstall the matching pair; rollback means
restoring the complete stable 3.1.3 package/engine pair, not copying an older
engine into a newer Python environment.

## Build from source

Use a supported Rust toolchain with Cargo and Maturin 1.15 or newer but below
2.0. The repository's current proof was run with Rust/Cargo 1.98.0 and Maturin
1.15.0:

```powershell
python -m pip install "maturin>=1.15,<2"
# Verify the locked workspace before Maturin's source-export build. Keep the
# Cargo lockfile fixed while Maturin builds the wheel from the pruned sdist.
cargo metadata --format-version 1 --locked --manifest-path rust/Cargo.toml
python -m maturin build --sdist --locked --out dist
python -m pip install dist\repopact-*.tar.gz
```

Cargo dependencies must be available in the local cache or fetched from the
network. Source builds are not compiler-free; the resulting wheel can be
installed without Rust.

The release build verifies the complete sdist and wheel inventories against
the committed source, the explicit Maturin include policy, and the resolved
Cargo build dependency closure. It rejects unlisted files, unsafe or
ambiguous paths, links, installation hooks, unexpected executables, and
unexpected wheel metadata. Every accepted file is checked against source or
the deterministic packaging rule; wheel `RECORD` is checked for complete
membership and per-file hashes.

Before the verifier launches the packaged engine for its bounded, non-mutating
protocol handshake, it hashes both wheel executables and compares them with
the corresponding local Cargo release outputs from the same controlled
Maturin build. Product version is derived from the trusted source `VERSION`,
`RELEASE_LABEL`, and `pyproject.toml`; sdist and wheel metadata, protocol
major, semantic identity, and the engine's embedded source revision must
agree. The verifier is for locally built artifacts in a trusted build
environment. These checks establish closed packaging and build-output
agreement, not an external signature, publisher authenticity, or a sandbox
for arbitrary downloaded executables.

## Roll back an executable

Rollback is package-level: reinstall a previously compatible RepoPact wheel or
switch the development environment to the previous committed source revision.
No repository record conversion, Git-history rewrite, or durable transaction
state change is required. WI050 admission/guard operations remain on their
existing protected Python/provider path.
