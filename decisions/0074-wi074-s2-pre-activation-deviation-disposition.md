---
id: 0074
title: WI074 S2 pre-activation deviation disposition
status: accepted
date: 2026-09-25
supersedes: ["0073"]
---

# 0074: WI074 S2 pre-activation deviation disposition

## Context

WI074 was registered as proposed at `2026-09-24T19:12:30Z` with an explicit
note that extraction was not authorized by the proposal. The S2 Core extraction
commit `eaae21319488cd04effe9ff0362bb8f30495a3d6` was committed at
`2026-09-24T19:58:30Z`; its source-map ownership correction
`6aff2c376efb5ddf236bd11cd1d700f873ec6a4f` followed at
`2026-09-24T20:31:40Z`. The operator's architecture/S3 approval was recorded
at `2026-09-24T21:31:54Z` and authorized prospective S3 only.

The operator's S4.1/S5 instruction on 2026-09-25 directed the work coordinator
to record acceptance of the documented deviation and corrective measures.
This disposition was recorded at `2026-09-25T02:39:02Z`; it does not change
the times or scope of any earlier action.

## Decision

Accept and close the documented process deviation as a historical exception.
This acceptance is not retroactive authorization, ratification, or approval of
S2. Preserve the original preflight note, proposal/activation timestamps,
commits, source maps, and evidence without rewriting or backdating them.

Decision 0073's outstanding disposition is superseded by this decision. Its
chronology remains valid as the contemporaneous record of the then-unresolved
question; its status transition does not alter that record's content.

## Corrective measures

1. Treat the preflight marker as evidence that a work item was registered
   before work, never as permission to implement.
2. Before future physical implementation, the work coordinator must verify the
   item is active and that the operator's authorization covers the specific
   slice, then record that authorization before execution begins.
3. Preserve the S2 commits and evidence as-is. Do not rewrite history, amend
   the original preflight marker, or represent S3/S4/S5 approval as retroactive
   S2 authority.
4. Record this disposition and the timestamped evidence in the WI074 ledger;
   retain the remaining acceptance criteria in their truthful states.

## Approval and limits

The operator's current instruction explicitly requested recording acceptance
of this deviation and its corrective measures. This decision records that
approval at the time of disposition; it does not assert an earlier approval.
The separate conditional authorization for S4.1/S5 remains subject to the
security, governance, portability, and release gates in the current operator
instruction.
