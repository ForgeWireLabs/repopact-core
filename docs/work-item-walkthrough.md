# Follow a change through RepoPact

*Diataxis mode: tutorial (learning-oriented).* This example uses the real Core
work-item fields, commands, and evidence rules. The title and values below are
illustrative; they are not output from a run in this repository.

## 1. Capture the proposal

A contributor wants to add retry behavior to a network client. They record the
intent before implementation:

```console
repopact work propose "Add bounded retry policy"
```

Core creates a proposed work-item directory and JSON manifest without
implementation authority. The contributor records its owner, affected scopes,
and acceptance criterion before an operator accepts it. A representative
criterion uses the schema's actual `id`, `text`, `state`, and `evidence` fields:

```json
{
  "id": "AC-1",
  "text": "Transient failures retry at most three times; permanent failures do not retry.",
  "state": "pending",
  "evidence": []
}
```

The work-item README records the motivation and tradeoffs; `work-item.json`
holds machine-readable lifecycle and relationship state. The `owner_scope` and
`affected_scopes` must follow this repository's `governance/owners.json`.

The current CLI can create and amend a proposal, but it does not provide a
general command to assign its scopes or transition an existing item. Those
authoritative record edits remain explicit repository changes. Once an operator
accepts the proposal, update `status` to `active` and move the whole directory to
`work/active/` in the same change. Workbench offers typed plans for the
work-item operations it supports; it does not automate every lifecycle step.

## 2. Read the authority that applies

Before changing code, inspect the work item's declared scope and dependencies,
the applicable roles and invariants, and relevant decisions. `AGENTS.md` directs
contributors to the same governance records. Frozen-surface entries require
operator approval before a change; an owner scope is not an OS security boundary.

## 3. Implement and produce evidence

The contributor implements the retry behavior, then runs the repository's
declared verification profile. In this repository, `repopact verify quick` runs
the `quick` profile from `governance/verification.json`. To preserve that actual
run as a concrete evidence record linked to the work item, use:

```console
repopact verify quick --evidence-work-item NNN
```

Replace `NNN` with the generated item ID. The command records what it ran under
`evidence/runs/`; it does not decide that every acceptance criterion is met.
Evidence supports only the claim it actually measures.

## 4. Close the criterion with evidence

After checking the implementation and evidence, add the generated evidence ID to
the criterion and change `state` to `satisfied` only if the record supports the
criterion. Core rejects a completed item with pending criteria and a satisfied
criterion without linked evidence. A generic green test run does not establish
an unrelated behavior.

When all criteria are properly evidence-closed, move the full item directory to
`work/completed/` and set its JSON `status` to `completed` in the same repository
change. This explicit file edit and move are the current Core CLI workflow;
`repopact` has no general-purpose lifecycle-transition command. Then reconcile
derived state:

```console
repopact dashboard
repopact validate
```

The validator evaluates the repository records and relationships. It reports
diagnostics for violations; the exact diagnostics depend on the repository
state, so this guide does not substitute fabricated terminal output.

## 5. Recover the handoff later

A later contributor starts from the repository: the item README explains intent,
the JSON identifies scope and lifecycle, decisions preserve settled rationale,
and evidence links show what was observed. The dashboard and optional Repository
Orientation Graph help locate these records, but the versioned source records
remain authoritative.

For a complete first-use exercise, follow the [adoption tutorial](adopt-repopact.md).
