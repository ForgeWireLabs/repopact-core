---
id: 0073
title: WI074 S2 implementation before formal activation
status: superseded
date: 2026-09-24
supersedes: []
---

# 0073: WI074 S2 implementation before formal activation

## Context

WI074 was registered as proposed with an explicit note that physical extraction
was not authorized by the proposal. The S2 standalone Core extraction was
committed at `eaae21319488cd04effe9ff0362bb8f30495a3d6` (2026-09-24T14:58:30-05:00)
and its evidence-scope correction at `6aff2c376efb5ddf236bd11cd1d700f873ec6a4f`
(2026-09-24T15:31:40-05:00). The operator's formal architecture/S3 approval
was received later at `2026-09-24T21:31:54Z`.

## Decision

Preserve this chronology as-is. The later approval authorizes prospective S3
only and does not retroactively authorize, ratify, or rewrite the S2 work.
Leave the original preflight marker, registration timestamp, S2 commits, and
evidence unchanged. Do not change the S2 criteria merely to imply that the
authorization existed earlier.

## Required disposition

This record is deferred pending a separate operator/work-coordinator
determination of the governance disposition required for the pre-activation S2
execution, including whether corrective or audit action is required under the
then-applicable workflow. The current approval does not decide that question.
No later S3 acceptance may mark the chronology discrepancy resolved without
that separate disposition. The recorded discrepancy and required follow-up are
also captured in `evidence/runs/20260924-074-s2-chronology.json`.

## Alternatives considered

- Backdate WI074 activation or rewrite the preflight marker: rejected because
  it falsifies chronology.
- Treat S3 approval as retroactive S2 authorization: rejected explicitly by
  the operator.
- Silently ignore the discrepancy: rejected because it would hide a material
  governance fact.
