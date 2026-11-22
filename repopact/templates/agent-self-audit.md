<!-- BEGIN SHARED AGENT SELF-AUDIT v1 -->
## Mandatory pre-completion adversarial self-audit

Before claiming completion or requesting review, inspect the full task-scoped change against an explicit review base: committed branch diff, staged and unstaged changes, and relevant untracked files. Record base/HEAD and preserve unrelated user changes. A bare `git diff` is not the whole candidate.

Look for concrete correctness and edge-case defects, concurrency/races, fail-closed behavior, API/schema compatibility, security and authority boundaries, durability, resource leaks/cleanup, missing positive and negative tests, and unnecessary complexity or out-of-scope edits. Tie each finding to a location and credible failure scenario; do not invent stylistic defects or churn working code.

Repair confirmed defects only within authorized scope, rerun affected checks, and reinspect remediation. Use at most two repair/re-review cycles by default; if blocked, report `INCOMPLETE` or `FAIL` instead of looping or weakening tests/invariants. Escalate changes requiring new authority.

Handoff must state `Self-Audit: PASS|INCOMPLETE|FAIL`, reviewed revision and scope, concrete findings/fixes (or no confirmed findings), actual checks/results, unrun checks, and residual risks. PASS means review completed with no known in-scope blockers, not proof of defect freedom. Self-review is neither independent approval nor authorization to merge, deploy, release, publish, or perform destructive operations.
<!-- END SHARED AGENT SELF-AUDIT v1 -->
