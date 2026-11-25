# RepoPact Conformance Suite

This directory is the versioned conformance corpus for RepoPact. It promotes the
fixture set from internal tests into a public suite that alternative
implementations can run.

The suite is defined by [`manifest.json`](manifest.json):

- `valid/` is a minimal conformant RepoPact repository.
- `invalid/*` are overlays on `valid/`.
- `rules` is the normative machine-enforced rule inventory.
- each case names the rule or invariant it exercises and the expected validator
  outcome.

The runner validates coverage in both directions before executing cases: every
inventoried rule must be referenced, every case rule must exist, and the repository
tests require every fixture directory to appear in the manifest. Reject fixtures
must yield exactly one reference diagnostic containing `expected_message`; expected
plus unexpected violations are a failure, not a partial pass.

Run the reference implementation:

```powershell
python -m repopact.run_conformance
```

Run another implementation by passing a command template. The runner replaces
`{repo}` with a temporary fixture repository path:

```powershell
python -m repopact.run_conformance --command "my-repopact-validator --root {repo}"
```

An implementation claiming conformance must accept every `accept` case and reject
every `reject` case with the declared diagnostic text after the fixture coverage
and isolation gates pass.

## Separate reference implementation corpora

The normal reference command also reports WI050 admission vectors and WI079
self-audit vectors **separately** from the Rust repository-validation manifest.
The WI079 corpus is defined in
[`repopact/self_audit_conformance.py`](../repopact/self_audit_conformance.py);
its ten fixed vectors exercise the real Python helper against fresh disposable
Git repositories. They cover current, absent, incomplete, stale and omitted-path
reports, extra approval assertions at the root/review/finding levels, an
unresolved blocker, and a structurally reported fixed finding. It requires Git;
missing Git or fixture errors are failures, not successful skips.

Each WI079 vector has an explicit expected helper exit code, result and, where
applicable, rejection diagnostic. Successful reports must carry
`assurance=instruction-only`. The default reference conformance command fails
if any vector fails and retains a separate `wi079_self_audit_corpus` summary
in its evidence. Rust process-image provenance still refers to the canonical
engine; it does not attest to independent self-review or a protected completion
policy. The WI079 reference corpus is not executed by the custom
`--command` comparator path and is not a new requirement for alternative Rust
governance validators. No lifecycle schema or default enforcement policy changes.
