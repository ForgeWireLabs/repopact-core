# External engineering incident: governance throughput correction

- **Record kind:** external incident, not a RepoPact work item or evidence-run manifest
- **Date:** 2026-10-27
- **Status:** ready for review; ForgeWire-scale validation pending
- **Lead:** tooling owner
- **Affected scopes:** tooling, governance, documentation, evidence
- **Primary repository:** `ForgeWireLabs/repopact-core`
- **Integration target:** ForgeWire monorepo proposal path

## Why this is external

The required RepoPact registration attempt did not produce a valid work item:
the baseline work.propose CLI omitted mandatory preflight confirmation, and
the expected engine rejection was surfaced as an invalid mutation response
because the Python client compared different but semantically equivalent
diagnostic shapes. No work-item ID or lifecycle state is claimed. This incident
record is deliberately outside the work directory; it preserves the
registration blocker without fabricating registry state. One failed
registration attempt was observed in this task. No fleet telemetry or usage
denominator is available, so an organization-wide defect rate cannot be
estimated.
## Initial incident and baseline

The reported Fabric timeout could not be replayed because the private Fabric
repository, governance dataset, invocation trace, and host telemetry were not
present in the accessible workspace. The available evidence is from the clean
ForgeWireLabs Core baseline `e5581a3`, not the Fabric integration. No claim is
made that this is a Fabric-scale reproduction.

The baseline checks used a source-built engine matching Core's source identity
(`3.1.3.dev1`); the installed PyPI engine advertised `3.1.3` and was rejected by
the fail-closed identity check. On the clean baseline:

- `repopact validate` passed in 6,011 ms for one observed run.
- `repopact work propose "Baseline proposal-path measurement"` returned a
  generic invalid-mutation-result error in 1,374 ms without changing the tree.
- The raw response showed the real deterministic rejection:
  `mutation.preflight-unconfirmed`. Top-level protocol severity and nested
  mutation `blocking` state represented the same error but used different
  shapes, and the Python client rejected that valid representation.
- A disposable copy with explicit preflight confirmation completed ten direct
  source-engine proposal attempts in 9,771–13,370 ms: p50 10,041 ms, p95 and
  worst 13,370 ms. These were synthetic proposals in a disposable Core copy.
- Source tracing found initial snapshot work, candidate validation against a
  copied repository, full post-mutation validation against another copied
  repository, dashboard generation, and additional repository traversals.
  Baseline code had no phase-level timings, so attribution to any one phase is
  not available. Peak memory was not measured.

## Change and measurements

The implementation adds operation-scoped proposal validation, explicit
structured diagnostics, phase counters, a read-only plan/status path, a
two-second repository-scoped lock bound, same-key retry receipts, atomic
directory commit, post-commit dashboard refresh, a ten-second proposal client
deadline, and OS-reported engine working-set sampling where available. Full
`repopact validate` retains its existing repository-wide boundary.

The benchmark harness copies the Core source to a temporary directory and
mutates only that disposable copy. The checked-in machine-readable report is
2026-10-27-work-propose-benchmark.json. On Windows 10, Intel64 Family 6 Model
94, Python 3.12.4, the Core fixture began with 77 work items, 201 evidence
records, and 3 audit findings. Across ten proposals after an explicit
read-only warm-up plan, client wall time was p50 870.883 ms, p95/worst
1,014.683 ms. The separate cold warm-up plan took 4,154 ms. Rust engine
execution was p50 786 ms; snapshot p50 600 ms, planning p50 133 ms, critical
validation p50 131 ms, and dashboard regeneration p50 31 ms. Atomic commit was
p50 1 ms. The proposal path examined p50 457 / p95 467 indexed records and read
p50 560 / p95 575 source files. The OS-reported proposal process working set
was p50 47.5 MB and p95 47.8 MB. The ten-second deadline was not reached. These
are Core fixture measurements, not ForgeWire-scale results.

For context, ten explicit-preflight proposals on the clean baseline source
engine took p50 10,041 ms and p95/worst 13,370 ms. The observed candidate
reduction is about 91% at p50 and 92% at p95, but the baseline direct-engine and
candidate Python-client measurements do not use an identical runner, so this
comparison is indicative rather than a controlled before/after benchmark. That
baseline did not expose phase metrics.
Baseline source tracing showed candidate planning through copied repositories,
full post-mutation validation, dashboard work, and repeated repository
traversals. The candidate removes those copies and full post-mutation
validation from proposal admission. It retains global work-ledger checks,
dependency cycles and scope conflicts, repository-wide evidence and assurance
reference checks, sensitive-evidence guardrails, owner/invariant/frozen checks,
and atomic dashboard regeneration after commit. Release, verification, graph,
adopter, research, and documentation-mapping audits remain in canonical
repopact validate.

Validation results: repopact validate passed; cargo test -p
repopact-mutation passed (26 Rust tests across unit and integration targets);
cargo test -p repopact-validation passed (79 tests); python -m unittest
tests.test_engine_client -v passed (17 tests); python -m unittest discover -s
tests -v passed (396 tests, 3 skipped); conformance passed 43/43 compatibility
cases and 8/8 WI050 admission vectors; check-frozen reported no changes.

## Safety and unresolved evidence

Work identity, owner scopes, binding invariants, frozen paths, broken work and
evidence references, dependency cycles, assurance references, and sensitive
evidence remain blocking. A proposed item remains non-authorizing. Full
validation remains strict and authoritative. Failure after staging but before
the directory rename leaves no proposal receipt or work item; a timeout or
invalid/lost operation response after commit is resolved through the durable
receipt and same-key retry.

The actual ForgeWire governance dataset is not available in this workspace, so
the p95-under-five-seconds target on the ForgeWire-scale fixture remains
unproven. The Fabric trace and memory baseline are also unavailable. Synthetic
and Core measurements will be reported as such; they cannot close those
limitations.
