# Troubleshoot RepoPact

*Diataxis mode: how-to (diagnose a problem).*

Start with the command's exact diagnostic and the authoritative record or
configuration it names. The dashboard and Repository Orientation Graph are
derived views; when they disagree with their source, regenerate or reconcile
the view instead of editing it as an independent source.

## The command is missing after installation

Activate the Python environment where RepoPact was installed, then check:

```console
python -m pip show repopact
python -m repopact.cli --help
```

On Windows, invoke `.venv\Scripts\repopact.exe`; on macOS/Linux use
`.venv/bin/repopact`. A package installed in one environment is not available
to another environment just because both use the same checkout.

## Core reports an engine compatibility error

Check the installed package version and ensure its matching native
`repopact-engine` is available in the same environment. The compatibility
client deliberately fails closed; it does not fall back to Python semantic
validation when the engine is missing or mismatched. Reinstall a compatible
wheel or rebuild the engine for the source revision you are using. Do not copy
an unrelated engine executable into the environment. See
[Install and build the engine](guides/packaging-and-engine.md).

## Validation reports malformed or missing records

Read the diagnostic path and code. Check that the file is under the expected
repository directory and conforms to its schema in `repopact/schemas/`. For
cross-record failures, follow the referenced work-item, decision, or evidence
IDs and verify the lifecycle and links. Do not change a satisfied criterion to
hide missing evidence; either produce concrete evidence or keep the criterion
pending/waived with the required rationale.

Common reasons include a malformed JSON value, an unknown owner scope, a
missing evidence ID, a pending criterion in a completed item, or an item whose
documentation impact is unresolved at its completion transition. See
[Author records](guides/author-records.md) and the normative
[`SPEC.md`](../SPEC.md).

## The dashboard or generated specification is stale

The validator uses the `dashboard.stale` diagnostic when generated dashboard
content no longer matches its source records. Regenerate from those records:

```console
repopact dashboard
repopact spec
repopact validate
```

If validation still reports stale generated content, compare the generated
file with its generator inputs and include the exact diagnostic in the work
item/evidence. Do not hand-edit generated blocks to force a pass.

## A frozen-surface check reports a change

Use `repopact check-frozen --base <ref>` to identify protected paths or
symbols in the diff. Read `governance/frozen-surface.json` and follow its
approval procedure. A clean check means no protected changes were detected in
that comparison; it does not grant permission for unrelated changes.

## Verification is incomplete or unavailable

Read the named profile and each step's state. `incomplete` can mean the host
lacks a required capability or a required platform has not supplied evidence.
It is not equivalent to a pass. Use the profile matching your task, install
only the documented development prerequisites, and preserve the actual host,
commands, exit codes, and limitations in evidence. See
[Local-first verification and release](guides/local-ci-cd.md).

## Admission or release operation is blocked

Admission, operator approval, protected guard installation, and release
publication have separate trust and authorization boundaries. Read the
[pre-execution admission guide](guides/pre-execution-admission.md) or
[release workflow](guides/local-ci-cd.md), inspect the current repository
records, and resolve the reported prerequisite. Do not bypass the guard by
switching to a Python-only validation path.

## Need to resume later

Read `AGENTS.md`, the active Work Item, linked decisions, and its latest
evidence. Source records in the repository remain authoritative after a shell,
machine, or contributor changes; the dashboard and graph can be regenerated
when stale.
