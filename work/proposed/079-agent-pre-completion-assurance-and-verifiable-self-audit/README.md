# 079 — Agent Pre-Completion Assurance and Verifiable Self-Audit Evidence

> **Status:** proposed; not implementation-authorized.
> **Lead:** governance-owner. **Affected:** tooling, evidence, docs, work.
> **Depends on:** none. **Created:** 2026-11-22.

## Intent and observed gap

A coordinated 2026-11-22 draft-PR rollout adds a bounded adversarial self-audit instruction to the root `AGENTS.md` of 20 active repositories. That immediately improves guidance, but **an agent's `Self-Audit: PASS` statement does not establish that an audit happened, that its candidate is current, or that its review is correct.** Instructions do not provide a process or merge boundary.

RepoPact already has work-item acceptance criteria, evidence runs, local verification profiles, canonical Rust semantics, audits, and optional admission. This item investigates and, following an explicit design decision, implements the *smallest optional generic mechanism* that allows adopter policies to require current, attributable pre-completion review evidence for supported operations. Do not build a second validator, force every adopter to install an agent framework, or confuse review evidence with approval.

This is **not** a rewrite of [WI044](../044-typed-capability-completion-and-replacement-cutover-evidence-contracts/README.md) (typed product-completion and cutover claims) or H14 (admission checkpoint coverage, invocation, effectiveness). These are adjacent models to reconcile, not silently absorb. WI078 is separately reserved for agent workspace/build cleanup on `codex/wi078-govern-agent-workspace-cleanup`.

## Review contract to evaluate

1. **Candidate boundary:** resolved comparison base, repository and worktree identity, HEAD/tree when available, staged/unstaged state, relevant untracked files, renames/deletions and excluded/unsupported surfaces. A review of a dirty tree cannot masquerade as one of an immutable commit. A post-review mutation invalidates or appropriately narrows prior review/test evidence.
2. **Adversarial inspection:** concrete defects in correctness, error handling, concurrency, permissions/security, contracts, persistence, performance/resource leaks, cleanup, and scope. Findings identify location, plausible failure, and evidence. Do not manufacture stylistic defects or change unrelated work.
3. **Bounded remediation:** in-scope fix, focused tests, re-review of changed surfaces; default maximum two repair cycles, with explicit blocked/incomplete handoff instead of infinite churn or relaxed tests.
4. **Truthful reporting:** separate *self-reported review*, *observed command execution*, *human/independent review*, *operator authorization*, and *enforcement boundary proof*. A green test and a review statement prove different things. `PASS` is not defect freedom, release clearance, or independent approval.
5. **Conditional closeout:** only opted-in repositories and supported host/workflow boundaries may treat missing or stale review evidence as a completion blocker. Bare `AGENTS.md` remains instruction-only; without demonstrated invocation and effective rejection, never claim fail-closed enforcement.

## Decision sequence

**Phase A — baseline and architecture.** Audit current RepoPact schema/CLI/verification/evidence behavior; review the root-contract draft rollout; evaluate at least three placements (template only, typed evidence + existing verify, optional completion policy). Record why a candidate does or does not warrant a new schema/semantic check. Compare against WI044 and H14. Make policy versioning and old-adopter compatibility explicit before implementation.

**Phase B — bounded implementation, if selected.** Implement approved behavior once in Core's canonical Rust semantics and exposed supported CLI surface. Prefer existing evidence runs and verification-profile steps over additional state. If introducing a record or policy, make it opt-in, versioned, portable, schema-backed, and honest about dirty worktrees and unavailable environments. Do not widen mutation authorization or require a model vendor.

**Phase C — adversarial tests and proof.** Run positive and negative cases: no-findings review; defect fixed and retested; stale candidate; unknown base; untracked/staged omission; altered file after inspection; fabricated or absent review; missing/failed required checks; out-of-scope or frozen change; attempted self-approval; and release-gate bypass. Explicitly demonstrate where a supported adapter actually intercepts completion and where same-principal bypass remains possible. Compare instruction-only and opt-in behavior.

**Phase D — research and adoption.** Pre-register matched tasks in three conditions: ordinary verification, bounded self-audit, and separate independent reviewer. Measure defect escape, spurious findings, task success, added diff/churn, time and token/compute cost. Preserve negative results. Document opt-in configuration, `AGENTS.md` nesting, candidate invalidation, verification state, and evidence closure. Do not automatically merge any adopter PR or change an existing release gate.

## Existing rollout reference (separate work)

- RepoPact Core root contract: [draft PR #17](https://github.com/ForgeWireLabs/repopact-core/pull/17).
- Fabric standalone: [draft PR #102](https://github.com/ForgeWireLabs/forgewire-fabric/pull/102).
- Bonsai Low-Bit Lab: [draft PR #28](https://github.com/JeremyShows/Bonsai_low_bit_lab/pull/28), whose root `AGENTS.md` is a frozen surface.
- The broader rollout covers 20 active projects; those PRs are instruction-level and do not satisfy this item's executable acceptance criteria.

## Non-goals and hard boundaries

- No mandatory global model-based review, second coding agent, or online/hosted service.
- No unbounded repair loop or edits beyond an authorized work item.
- No fabricated verification records, auto-approval, or self-issued operator receipts.
- No claim of security confinement against a fully privileged same-principal agent.
- No retroactive rewriting of completed work/evidence or required migration of legacy adopters.
- No immediate universal enforcement by merely adding `AGENTS.md` text.

## Closeout requirements

The ten criteria in `work-item.json` remain pending. Before implementation, record architecture and scope approval in the existing governance workflow. Completion requires criterion-linked concrete evidence, documented limits and negative cases, resolved documentation-impact state, canonical RepoPact validation, and generator-produced dashboard/spec updates (never hand-edited). A draft work item alone is not an executable assurance feature.
