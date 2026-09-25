# 073 — Reconcile repository owner routing

> **Status**: Completed
> **Owner**: governance-owner (lead)
> **Affected scopes**: governance, work, evidence, and current-facing repository documentation/configuration in the local ecosystem.

## Intent

Ensure repository metadata, Git remotes, and actionable links use each
repository's current canonical GitHub owner after the personal/ForgeWireLabs
separation. Verify ownership through the GitHub API; do not infer it from
redirects. Preserve dated evidence and historical records.

## Scope disposition

Per operator direction, `SCOUT-3` is out of scope. Its local checkout and remote
were left untouched; no replacement URL was guessed.

## Acceptance criteria

1. RepoPact's adopter registry names the current owners returned by GitHub.
2. Local remotes and current-facing documentation/configuration in the audited
   checkouts point to their verified canonical repository URLs.
3. Historical evidence remains intact, and unresolved/deleted repository paths
   are reported rather than guessed.

## Scope

The governance owner leads the registry reconciliation. Documentation and
configuration owners are affected in ForgeLink, SkillForge Academy, the ForgeWire
Labs profile, Moto One Hyper, and RepoPact checkouts. No ownership transfer,
permission, billing, CI, or remote-content changes are authorized by this item.
