# RepoPact Agent Contract

## Binding thesis

The repository is the durable coordination surface. Conversations may initiate
work, but authority, state, decisions, validation, and handoff must survive in
versioned files.

## The pact (binding invariants)

The invariants in [`governance/invariants.json`](governance/invariants.json) are
binding on every role. You may not weaken one without flagging it and obtaining
explicit human operator approval. Where an invariant names an `enforced_by` rule,
that check is the backstop; where it names `null`, escalation is the only
enforcement and the responsibility is yours. Principles (judgment) and invariants
(binding) are kept distinct — see [`governance/charter.md`](governance/charter.md).

## Frozen surface

Paths and symbols listed in
[`governance/frozen-surface.json`](governance/frozen-surface.json) must not be
changed without operator approval (`INV-6`). Run
`repopact check-frozen --base <ref>` before proposing a change
that may touch them; only pass `--ack` after a human has approved.

## Invariants

- Read every `AGENTS.md` from the repository root to the target path.
- The deepest applicable contract refines its parents but cannot weaken them.
- Every change belongs to one explicit owner scope (the non-overlap principle).
- Cross-scope work must name a lead and affected scopes in its work item.
- Work is not complete until its acceptance criteria have linked evidence.
- Never rewrite completed work to make history appear cleaner.
- Audits report drift; they do not silently redefine the intended architecture.
- Machine-readable lifecycle state lives in JSON. Markdown explains why.
- Derived artifacts are generated, not hand-maintained (policy `001`).

## Roles and writable scopes

Roles and the scopes they may write are declared in
[`governance/owners.json`](governance/owners.json). Each role is bound to one or
more path scopes; keep each change within a single role's scope. Adopters redefine
these roles for their own repository — the contract is the structure, not this
particular role set.

- **Governance owner**: `AGENTS.md`, `governance/`, `repopact/schemas/`, `decisions/`, `research/`.
- **Work coordinator**: `work/`, status transitions, dependency records.
- **Evidence owner**: `evidence/`, validation manifests, reproducibility records.
- **Tooling owner**: `repopact/` except `repopact/schemas/`, `rust/`, `scripts/`, `tests/`,
  `conformance/`, packaged templates, packaging, and CI automation.

## Durable records

- **Decisions** (`decisions/`): material, hard-to-reverse choices and rejected
  alternatives. Promote a decision here when its rationale outlives the work item.
- **Policies** (`governance/policies/`): durable operating rules and hard-won
  lessons that are not themselves binding invariants.

## Repository Orientation Graph (optional)

Where enabled, [`repopact graph orient`/`search`/`dependencies`/`impact`/
`tests`/`governance`](docs/repository-orientation-graph.md) give a bounded,
deterministic way to find relevant work/decisions/tests before reading
source — but every fact they surface is derived, never authoritative.
Governance state still comes from `work/`, `governance/`, `decisions/`, and
`evidence/` directly.

## Required checks

Run before proposing completion:

```powershell
python -m pip install -e ".[dev]"
repopact validate
python -m unittest discover -s tests -v
```

Regenerate `audits/reports/dashboard.md` after changing governed structure or
work-item status. Run `repopact check-frozen` when a change may
touch the frozen surface.

## Change protocol

1. Honor the pact: check invariants and the frozen surface first.
2. Capture intent in a work item.
3. Discover applicable contracts and ownership.
4. Record acceptance criteria before implementation; promote durable choices to
   `decisions/`.
5. Implement within scope.
6. Record validation in an evidence manifest.
7. Reconcile affected audit entries; regenerate the dashboard.
8. Move the complete work-item directory to `work/completed/`.

<!-- BEGIN SHARED AGENT SELF-AUDIT v1 -->
## Mandatory pre-completion adversarial self-audit

Before claiming completion or requesting review, inspect the full task-scoped change against an explicit review base: committed branch diff, staged and unstaged changes, and relevant untracked files. Record base/HEAD and preserve unrelated user changes. A bare `git diff` is not the whole candidate.

Look for concrete correctness and edge-case defects, concurrency/races, fail-closed behavior, API/schema compatibility, security and authority boundaries, durability, resource leaks/cleanup, missing positive and negative tests, and unnecessary complexity or out-of-scope edits. Tie each finding to a location and credible failure scenario; do not invent stylistic defects or churn working code.

Repair confirmed defects only within authorized scope, rerun affected checks, and reinspect remediation. Use at most two repair/re-review cycles by default; if blocked, report `INCOMPLETE` or `FAIL` instead of looping or weakening tests/invariants. Escalate changes requiring new authority.

Pre-existing failures are not waived by showing they also occur on the baseline. Separate cause attribution from resolution: repair the confirmed root cause when authorized, or create/link an owned, actionable remediation work item with reproduction, evidence, and a validation gate. Preserve failures and keep project-level health accurately red or incomplete until resolved. Never use permanent skips, false evidence, or weakened assertions to manufacture green results.

Handoff must state `Self-Audit: PASS|INCOMPLETE|FAIL`, reviewed revision and scope, concrete findings/fixes (or no confirmed findings), actual checks/results, unrun checks, and residual risks. PASS means review completed with no known in-scope blockers, not proof of defect freedom. Self-review is neither independent approval nor authorization to merge, deploy, release, publish, or perform destructive operations.
<!-- END SHARED AGENT SELF-AUDIT v1 -->
