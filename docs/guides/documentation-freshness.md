# Guide: Check documentation freshness

*Diataxis mode: how-to (task-oriented).*

RepoPact can detect structural freshness drift for source/documentation pairs an
adopter explicitly maps. It compares file bytes with a reviewed snapshot. It
does not judge prose meaning or prove that a generator produced its output.

## Opt in

Add this setting to `governance/owners.json`:

```json
"documentation_mappings": {
  "enabled": true,
  "path": "governance/documentation-mappings.json"
}
```

Start from [`documentation-mappings.json`](../../repopact/templates/documentation-mappings.json)
and replace its example paths. A mapping names a stable ID, a kind (`manual` or
`generated`), one or more repository-relative source paths, one or more
documentation paths, a short explanation, and the reviewed digest snapshot.
Paths must resolve to regular files inside the repository. The configuration is
opt-in; repositories without it remain compatible.

## Review and refresh a mapping

Run `repopact validate` to make drift a blocking diagnostic, or `repopact doctor`
to see a non-destructive warning. A source-only change is reported as stale. If
source and documentation both changed, RepoPact still asks for review because
matching edits do not prove that the documentation explains the implementation.
For `generated` mappings, regenerate the output first, then review the pair.

After reviewing the relevant changes, request current digests:

```text
repopact documentation snapshot <mapping-id>
```

The command prints `source_digest` and `documentation_digest`; it does not edit
the mapping. Merge those two values into that mapping's `snapshot` object and
rerun validation. `doctor --fix` never refreshes this baseline. Treat every
snapshot refresh as a human review assertion. The hashes prove only that the
mapped bytes match the reviewed baseline; they do not prove semantic accuracy,
generator provenance, or completeness of the mapping.

## Keep the boundary narrow

Map the source files whose change creates a real documentation freshness risk
to the documentation files that explain them. Avoid broad directories and
unrelated files: a mapping is a review trigger, not a code-to-prose inference
engine. Use the ordinary `documentation_impact` work-item closure for evidence
that a specific change's documentation was created, updated, or regenerated.
The mapping check adds prospective drift detection; it does not replace that
closeout record. See [Decision 0076](../../decisions/0076-structural-documentation-freshness-mappings.md)
and the normative [SPEC](../../SPEC.md).
