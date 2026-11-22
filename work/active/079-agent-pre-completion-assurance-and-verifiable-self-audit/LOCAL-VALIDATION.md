# WI079 — Host-local validation handoff (draft PR #18)

This is the first boundary requiring an actual RepoPact Core checkout, canonical Rust engine/toolchain, and, for cross-platform claims, host-specific execution. Work is authorized on **WI079 only**. Keep the PR draft and preserve all release/merge/frozen-surface gates.

## Start and stop conditions

Use an isolated worktree based on the current remote head of `work/079-pre-completion-assurance-20261122`. Do not reuse a dirty checkout or overwrite user work. Record the **full 40-character** HEAD, merge-base and tree, Git status, Git/Python/Cargo versions, available disk space, and actual test environment. Ensure the branch contains only the intended WI079 diff. Inspect applicable root/nested `AGENTS.md`, current governance, and the work item.

Do not auto-merge, tag, publish, change production hosts, or weaken any tests/acceptance criteria. A frozen-surface modification or significant design expansion requires the existing approval path. Keep generated files generated, never manually patched to force green.

## Ordered local execution

1. **Focused syntax and regression check** (start here):
   - `python -m py_compile repopact/self_audit.py tests/test_self_audit.py`
   - `python -m unittest discover -s tests -p test_self_audit.py -v`
   - There are **24 test methods** in this draft. Earlier isolated evidence ran **23/23** except `test_top_level_cli_exposes_experimental_capture`. Record the full local result and repair confirmed failures without weakening assertions.
2. **Candidate and report smoke**: in a disposable Git repository/worktree, use `repopact self-audit capture --base <exact-commit>` and `check --report <outside-the-repository-json>`. Confirm current self-report returns structural result only; changing staged, unstaged, untracked, or HEAD state causes a nonzero status; open blockers return exit 1, stale/incomplete return exit 2. Confirm missing Git and unsupported paths fail closed.
3. **Canonical identity and derived outputs**: run `python scripts/generate_engine_semantic_identity.py --check` to observe current divergence, then, after reviewing the actual changed source and template scope, use **`--update`**, not hand edits, to regenerate the package/build semantic identity. Run `repopact dashboard` to regenerate `audits/reports/dashboard.md`. Only run `repopact spec` if the documented generator indicates it is required; inspect the generated diff and keep unrelated projections untouched.
4. **Canonical project verification**: run `repopact validate`, `repopact verify ci` and relevant Rust tests, with exact command/output counts and exit statuses. Use the environment documented by RepoPact; do not substitute the legacy Python comparator for the canonical Rust authority. Record any unavailable capability as `INCOMPLETE`, not PASS.
5. **Cross-platform checks**: perform a small, bounded Windows worktree/CLI run on an actual Windows host before claiming Windows support. Do not infer Windows/macOS behavior from isolated Linux success. No publication or release is authorized.
6. **Governance closeout hygiene**: verify ownership/scope, optional verification profile behavior, evidence references, documentation-impact status, `git diff --check`, no broken relative links, and no unexplained changes. Commit only scoped fixes and generated canonical artifacts. Update this draft PR with the exact head, tests, limitations, and evidence IDs.

## Results required back

Return a table showing `focused Python`, `top-level CLI`, `semantic identity`, `dashboard`, `canonical validation`, `verify ci`, `Rust tests`, `Windows target`, `review freshness negative cases`, and `release/approval boundary` with statuses `PASS / FAIL / INCOMPLETE / NOT RUN`, exact run logs, and a full head SHA.

**Stop** on inconsistent semantic identity, unresolved failing tests, unrelated dirty user files, pending protected-surface authorization, or attempts to claim independent review/protected completion authority from a self-report. Continue only safe and independent verification lanes. Leave the item active until all ten acceptance criteria have linked, relevant concrete evidence.

## Research still outstanding

The [preregistered study](../../../research/wi079-self-audit-benchmark-protocol.md) requires live matched-agent tasks and recorded cost/churn/escaped-defect outcomes. It is **NOT RUN** and cannot be simulated with source tests. It may be deferred with an honest work-item scope decision rather than invented or retroactive evidence. Do not require paid external services.
