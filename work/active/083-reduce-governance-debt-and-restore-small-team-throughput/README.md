# 083 — Reduce governance debt and restore small-team throughput

> **Status**: Active
> **Lead**: tooling-owner
> **Owner scope**: tooling
> **Affected scopes**: tooling, docs, governance, work, evidence
> **Depends on**: none

## Intent

Reduce repeated RepoPact work in the shipped tutorial and verification orchestration while keeping the declared assurance intact. This is one reversible implementation batch: simplify the existing path, measure candidate validation work, and retain the current security, frozen-surface, evidence, and transaction guarantees. No new mode, cache framework, mandatory self-audit, hosted CI, paid experiment, merge, or release is in scope.

## Scope

- Remove avoidable repeated verification instructions and align bootstrap/adoption wording.
- Consolidate per-invocation verification contract loading without a public unchecked-config path or cross-run cache.
- Measure the existing Rust mutation candidate path; change it only if the safety boundaries in AC-4 remain demonstrably intact.
- Capture focused regression and exact local validation evidence, refresh semantic identity and generated dashboard, and prepare one draft PR.

## Registration and allocation

Main was verified at `b9dec03ad59c2ef7d782eda9f2b3220747e8ba3c`. Open draft PR #29 is based on that commit and carries proposed WI082 at head `daa94e8ddc264212a7e1c372b5c935c95c522ab5`. The canonical allocator would choose 082 from main, so WI083 was created through the existing Rust-engine `work.create` path on a detached local worktree of the actual PR #29 candidate. Only the generated WI083 directory was copied here; no PR #29 branch was committed or pushed. Temporary identity and documentation-mapping reconciliation in that scratch worktree was not carried over.

The first CLI create attempt timed out at its 30-second limit after dispatch. The worktree was inspected before retry; no WI083 record or dashboard update existed. A single retry through the same canonical create path with a 180-second client deadline completed in 18.285 seconds. This is a local observation, not a general performance claim.

## Decisions

- A successful declared profile run can satisfy the same check for the same relevant candidate if no relevant inputs have changed since it ran. Focused tests during iteration do not replace the final required profile.
- Keep preflight and current-state checks at their existing boundaries. Keep all profile steps and the required legacy comparator distinct where their assurance differs.
- Do not reuse executable evidence based on commit/tree or a doc-only assumption. Do not add a cache unless a complete relevant read set and all execution inputs are proven.
- Leave matched productivity/effectiveness research as an explicit pending outcome, using WI022 and the open WI082 proposal rather than claiming a result from this batch.

## Closeout

Every satisfied criterion needs linked evidence in `work-item.json`. Keep unavailable research pending. Regenerate the dashboard after governed changes. Submit one validated draft PR; do not merge, approve, or release.
