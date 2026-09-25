# WI074 next bounded slice: RPS-007 and RPS-012

## Authorization boundary

This is a prepared implementation directive only. It does not authorize
execution, publication, Workbench or consumer changes, Actions, PyPI activity,
or modification of historical evidence. Begin only under a subsequent
operator instruction. Do not repeat the completed architecture review.

## Baseline and scope

Start from the then-current canonical `ForgeWireLabs/repopact-core` main and
record its exact SHA before creating a clean isolated worktree. Use WI074's
canonical Core ledger. Preserve all existing criterion states and evidence
except RPS-007 and RPS-012. Do not start RPS-008 through RPS-013 work that is
not necessary to prove these two criteria.

Lead: tooling owner. Affected scopes: tooling, governance, evidence, and only
the documentation paths required by accepted scope contracts.

## RPS-007 — semantic authority

Prove from executable behavior and dependency/source boundaries that Core is
the sole authority for canonical validation, governance interpretation,
repository graph semantics, and governed mutation. Inventory duplicate or
shadow semantics in shipped interfaces, test Python/Rust parity against the
canonical engine, exercise fail-closed mismatch cases, and document any
explicitly non-authoritative compatibility adapters. Do not redesign APIs or
claim Workbench parity in this slice.

Acceptance evidence must identify tested entry points, authoritative source
for each semantic, negative cases against duplicate/fallback behavior, exact
commands, source revision, results, and unresolved exceptions. Any discovered
duplicate authority is a failure to resolve or an explicit blocker—not a
reason to weaken parity checks.

## RPS-012 — admission and security correctness

Verify shipped headless admission and guard behavior, including denial before
side effects, protected-scope enforcement, operator authorization, revocation,
session/lease binding, tamper resistance, platform feature gates, and
conservative assurance classification. Use isolated disposable repositories
and credentials only; never use production credentials. Run applicable Rust,
Python, conformance, and security regression suites. Do not broaden claims
beyond platforms actually exercised.

Record exact threat cases and results, test environment, cleanup, and limits.
Any security failure, missing mandatory platform proof, or unexplained
classification mismatch blocks satisfaction of RPS-012.

## Closeout gate

Add separate immutable evidence records, link only RPS-007/RPS-012 when every
criterion clause is proven, regenerate the dashboard, and run canonical
validation, evidence/schema/reference tests, frozen-surface checks, and a
redacted secret scan against the approved base. Review the exact outgoing diff.
Push only if a future operator authorization covers publication and all
release/security gates pass. Otherwise preserve the local work and report the
precise blocker.
