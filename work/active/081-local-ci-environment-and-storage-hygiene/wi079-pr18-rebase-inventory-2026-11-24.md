# WI079 PR #18 rebase inventory — 2026-11-24

This is a read-only merge analysis. No PR #18 worktree, branch, or commit was
rewritten. Main was `f5592e371d6bcc082122823ccf04973bd829fb36`; PR #18 was
`0562293d1f4a789055f6035204a60550ff29fd9e`; their merge base was
`0ac343a2bf25f56a47e8f42798eca938844199fa`. `git merge-tree --write-tree`
returned conflicted merge tree `a77c4123a11e9ca1a6cec94c42cdb9fd8cf8ab32`
without changing a checkout.

## Content conflicts

The simulated merge reports content conflicts in these five generated or
source-to-documentation files:

- `audits/reports/dashboard.md` — regenerate from the merged authoritative
  work, evidence, and audit records after reconciliation.
- `build/semantic-input-manifest.json`
- `repopact/_semantic-input-manifest.json`
- `repopact/_semantic_identity.json` — regenerate all semantic outputs from the
  merged source with `scripts/generate_engine_semantic_identity.py --update`;
  do not take either branch's saved output as authoritative.
- `governance/documentation-mappings.json` — review each affected manual
  mapping and its documentation, then compute snapshots from the merged source.
  Do not select a stored digest just to make validation green.

The documentation mapping diffs touch `documentation-freshness-tooling` and
`engine-semantic-compatibility`. The latter also changed on main as WI080
updated the semantic manifest and engine conformance source. Re-review the
mapping descriptions against the merged CLI and packaging guide, then snapshot
the merged inputs. Preserve mapping intent and documentation digests unless
their actual documentation changed.

## Research metadata and documents

PR #18 adds `research/wi079-self-audit-benchmark-protocol.md` and registers it
in `research/metadata.json`. Main contains the WI080 review of the 14 documents
then registered, with `verified_on` 2026-11-23 and `review_by` 2026-12-23,
supported by `evidence/runs/20261123-080-research-freshness-review.json`.
PR #18's metadata side retains the earlier 2026-10-22 / 2026-11-21 dates and
must not roll back main's reviewed state. The three-way merge is textually
clean, but the newly added 15th research document was outside that 14-document
review. Read and review that document and its benchmark/evidence claims before
including it in the reviewed set or advancing any verification date. Also
retain main's 2026-11-23 adopter-commit-visibility review date; do not replace
it with PR #18's older 2026-10-22 value.

PR #18's `audits/registry.json` and dashboard changes relate to its new guide
and should be reconciled with the merged documentation review. The registry
merge is non-conflicting; the dashboard must be regenerated rather than
manually combined.

## Rebase sequence when authorized to proceed

1. Start from the then-current main in a new clean disposable worktree and
   preserve PR #18's current branch and evidence.
2. Reconcile `research/metadata.json` with the main-side WI080 review, review
   the new WI079 protocol document and its evidence, and change dates only for
   claims actually reviewed.
3. Inspect both documentation mappings and their mapped prose; update hashes
   only after that review.
4. Regenerate semantic identity and the dashboard from merged authoritative
   files; validate, inspect the diff and resolve all conflicts explicitly.
5. Run local validation on the resulting exact source head. Do not reuse the
   f5592e3 baseline result as evidence for the rebased candidate.

The changes in PR #22 are not part of this comparison. No full CI was run on
the f5592e3 baseline for this inventory, and no PR #18 ref was changed.

## Workbench/Fabric cleanup boundary

WI057's operator-assisted Workbench procedure samples one launched process tree
and terminates only that exact Workbench tree. It does not authorize deleting
repositories or Cargo targets. Active WI077 is a proposal-reliability and
performance investigation, not a storage-cleanup plan. WI076's historical
review explicitly left its denied temporary repository and unrelated Fabric
Cargo tree untouched. The WI081 inventory and cleanup tooling therefore do not
claim or inspect those paths as owned. No automatic cleanup integration into
Workbench or Fabric is proposed; that product integration remains an explicit
operator decision.
