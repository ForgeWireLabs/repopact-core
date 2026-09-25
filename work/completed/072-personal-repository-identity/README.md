# 072 — Keep RepoPact on the personal GitHub repository

> **Status**: Completed
> **Owners**: governance-owner (lead), docs-owner, tooling-owner, evidence-owner.
> **Depends on**: none.

## Intent

RepoPact now belongs at `JeremyShows/repopact`. Git remotes and current product,
package, citation, contributor, and repository-identity references must use that
canonical location directly instead of depending on GitHub's redirect from the
former `ForgeWireLabs/repopact` location.

Historical records that accurately describe releases, publications, or decisions
made while the repository was at its former location remain unchanged. The
separate RepoPact Proving Ground repository is also pointed at its current
personal owner, `JeremyShows/repopact-proving-ground`.

## Reopened scope

This item was initially closed after correcting RepoPact's self-references. A
subsequent direct GitHub API audit showed that the linked Proving Ground had
also moved from the organization namespace to the personal account. This item
was reopened to update those current pointers. Historical evidence and
completed work remain unchanged.

The follow-up was completed with direct owner verification and updated current
links. Repository-wide validation still reports the separately expired research
freshness contract; that historical claim needs its own review.

## Scope and decision

- Lead scope: `governance`; affected scopes: `docs`, `tooling`, `evidence`, and
  `work`.
- Set this checkout's `origin` fetch and push URL to
  `https://github.com/JeremyShows/repopact.git`.
- Update current self-references in package metadata, citation metadata,
  repository guidance, release links, upstream identity, tests, and the GitHub
  API user-agent contact URL.
- Preserve completed work, historical evidence, and historical decisions as
  point-in-time records under INV-4.

## Acceptance criteria

1. The local `origin` fetch and push URLs directly name `JeremyShows/repopact`.
2. Current RepoPact and RepoPact Proving Ground links point directly to their
   current personal repositories.
3. Historical records remain unchanged, and the active work item is closed with
   linked validation evidence.

## Closeout

Record validation in an evidence run, regenerate the dashboard if required by
validation, then move this complete work-item directory to `work/completed/`.
