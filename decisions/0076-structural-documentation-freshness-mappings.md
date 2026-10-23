---
id: 0076
title: Structural documentation freshness mappings
status: accepted
date: 2026-10-22
supersedes: []
---

# 0076: Structural documentation freshness mappings

## Context

Decision 0065 deferred mappings until actual `documentation_impact` usage
existed. Completed WI072 and WI073 now provide that usage across operations,
configuration, and contributor workflow. WI047 AC-7, AC-8, and AC-10 can
therefore be addressed with a deliberately structural contract.

## Decision

Adopters may opt in through `governance/owners.json` and declare a versioned
`governance/documentation-mappings.json` file validated by the packaged
`documentation-mappings.schema.json`. Each mapping names exact repository-
relative regular-file paths for its sources and documentation, a `manual` or
`generated` kind, and a reviewed pair of deterministic SHA-256 digests.

Canonical validation fails when mapped source or documentation bytes drift from
the reviewed pair. It distinguishes source-only drift from documentation drift
and generated-output drift. `repopact doctor` reports the same condition as a
non-destructive warning. `repopact documentation snapshot <id>` prints current
digests without writing them; a human must reconcile or regenerate the files,
review the pair, and update the mapping snapshot. There is no automatic
baseline advancement, generator execution, or arbitrary command field.

The rule is opt-in and prospective. An absent or disabled mapping configuration
does not affect existing adopters or historical completed work. The mapping
format is versioned; future incompatible semantics require an explicit schema
version change and compatibility decision.

## Limits

Matching digests prove only that the bytes match a reviewed snapshot. They do
not establish that prose is semantically correct, that every relevant source or
documentation path was mapped, or that a generated file came from its stated
generator. `generated` marks a declared risk class and makes source/output drift
visible; reviewers remain responsible for running and checking the appropriate
generator. Path containment and file existence are checked, while arbitrary
commands and external paths are rejected.

## Alternatives considered

- Inferring documentation meaning from source or Markdown: rejected because
  generic analysis cannot prove semantic correctness.
- Advancing hashes automatically in `doctor --fix`: rejected because it would
  erase the review boundary and allow stale content to self-approve.
- Running adopter-supplied generators: rejected because arbitrary commands
  expand the execution and trust boundary beyond this freshness contract.
- Requiring mappings for every repository: rejected because adopters have
  different layouts and mapping completeness is a deliberate opt-in choice.
