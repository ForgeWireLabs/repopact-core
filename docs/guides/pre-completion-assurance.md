# Pre-completion self-audit (WI079 experimental helper)

*Diataxis mode: how-to (prototype; not part of the released 3.1.3 contract).*

RepoPact can store evidence about actual local verification commands, but it cannot determine whether a coding agent thought critically about a diff. The WI079 helper adds a **read-only structural freshness check for a self-reported review**. It is opt-in, Git-based, provider-neutral, and does not alter canonical work-item completion semantics. It is not equivalent to a protected merge gate or a sandbox.

## Shared instruction template and nested contracts

The [versioned opt-in AGENTS.md block](../../repopact/templates/agent-self-audit.md) is available for repositories that want a consistent advisory review protocol. The template is not installed automatically by `repopact init` or `adopt` and does not change a repository's enforcement class by itself. Read the root contract and each applicable nested `AGENTS.md`; deeper rules may specialize or strengthen but must not weaken the parent. Copy the block only once into the appropriate root contract, and reconcile future template changes against local ownership, frozen paths, tests, and approvals rather than overwriting tailored instructions.

## When and why to use it

Use it after implementation and relevant tests, but before claiming a review of the final candidate. It covers a specific comparison base, HEAD, Git index, nonignored untracked files, and working-tree content changed relative to that base. It detects changes after the snapshot, including a newly added untracked file or a staged edit. It does **not** inspect ignored files, unrelated generated assets excluded by Git, or the quality of an agent's reasoning.

For an existing worktree, use an explicit reviewed base; do not silently assume `main` or `origin/main` points to the same commit on every host. Have Git available and capture the report **outside** the repository (for example, in a worktree-specific validation-log directory). This avoids a self-referential review file changing the candidate after capture.

## Example: capture, review, and recheck

Run from a local RepoPact Core development install containing WI079:

```powershell
$root = (Resolve-Path .).Path
$base = 'origin/main'
$report = Join-Path (Split-Path $root -Parent) 'wi079-self-review.json'
repopact self-audit capture --root $root --base $base | Set-Content -LiteralPath $report -Encoding utf8
```

The command emits a JSON template. It starts with `review.outcome = "incomplete"`. Inspect the entire declared changed-path set and the relevant source/tests. Edit only the **review** section to set:

- `outcome`: `pass` only after a completed bounded inspection;
- `actor`: the self-reporting contributor/agent;
- `reviewed_paths`: exactly the candidate's `changed_paths` (no missing entries);
- `findings`: `[]` if none confirmed, or objects with `path`, `scenario`, and `status` (either `fixed` or `nonblocking`);
- `limitations`: unrun checks, residual risks, or unsupported surfaces.

If a confirmed blocking finding remains, leave the review incomplete; do not call it a pass. After a fix, run the appropriate tests again and **capture a new candidate snapshot**. Do not reuse the old report. The default repair-loop budget is two cycles under the shared AGENTS.md guidance; if it does not converge, escalate as blocked/incomplete.

Then check:

```powershell
repopact self-audit check --root $root --report $report
```

Exit codes: `0` means the **self-reported review matches the current candidate structurally**; `1` means a declared unresolved blocking finding; `2` means missing, stale, invalid, unsupported, or incomplete. A relative `--report` path is resolved against `--root`, not the shell’s current directory. A successful check prints `self-report-current` and `assurance: instruction-only`: **it does not prove review thoroughness, test success, or independent approval**.

## Optional local verification integration

A repository can add a *required* argv step to its own `governance/verification.json` profile (illustrative, not installed by default):

```json
{
  "id": "self-review-freshness",
  "argv": [
    "{python}", "-m", "repopact.self_audit",
    "check", "--root", "{root}",
    "--report", "../wi079-self-review.json"
  ],
  "required": true,
  "timeout_seconds": 120
}
```

Configure a distinct report filename per worktree for the executing host and avoid embedding secrets or private absolute paths in shared repository configuration. This is a *profile check on that host*, not universal model compliance. Independently run and record actual builds/tests, for example `repopact verify ci --evidence-work-item NNN` where the existing work-item and profile requirements permit it. The verification evidence records executed step statuses; it does not upgrade the self-review's trust class. Do not make a passing review claim a new source of authorization.

## Status and exclusions

This WI079 implementation is a prototype source change and requires local Python unit tests, canonical validation, semantic-input identity regeneration, and supported-platform verification before integration. It is not in the published package.

The prototype deliberately rejects unsupported changed directory/submodule types, missing Git dependencies, oversized changed files, and unreadable inputs instead of silently skipping them. Ignored files are outside its coverage. A same-principal agent can fabricate its own report or avoid invocation; therefore even a profile-level PASS does not establish H14 admission closure or independent review. Treat release, signing, protected surfaces, experiments, and destructive actions as separately authorized.

For the rationale and comparison with WI044 and H14, see [WI079 design reconciliation](../../work/active/079-agent-pre-completion-assurance-and-verifiable-self-audit/design-reconciliation.md).
