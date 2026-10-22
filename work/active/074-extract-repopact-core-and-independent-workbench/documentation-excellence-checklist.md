# Documentation Excellence — Execution Checklist

**Updated:** 2026-10-22

**Canonical work item:** WI074, RPS-020 (Core `work/active/074-.../work-item.json`)

**Related obligation:** WI047 documentation-impact closure
**Lead:** tooling-owner; canonical WI074 affected scopes are tooling, governance,
docs, evidence, and work. The work scope is included for this initiative because
the coverage matrix, checklist, and a small set of active-work documentation
references are maintained under `work/`.

This checklist resumes the operator-authorized three-repository documentation
initiative from the current merged default branches. The coverage matrix is in
[`documentation-coverage-matrix.md`](documentation-coverage-matrix.md). A status
of `IN_PROGRESS` means edits exist but are not complete until the listed
validation and evidence are recorded. `NOT_RUN` is not a pass.

| Requirement | Implementation / current finding | Validation and evidence | PR | Status |
| --- | --- | --- | --- | --- |
| Recover current state and preserve prior work | Three target repositories cloned from merged `main`; prior uncommitted `SECURITY.md` edit in the historical `C:\Projects\repopact` checkout preserved separately. | Baselines: Core `9a3e51f`; Workbench `59e7141`; portfolio `53f5ba6`. Historical checkout remains uncommitted and has unrelated untracked `artifacts/`. | Not created | DONE |
| Core security policy P0 | Rewrote Core `SECURITY.md` for 3.1.3, Rust/Python split, trust boundaries, dependencies, reporting, and support limits. | Cross-checked against source and package metadata; isolated to one file. | [Core security PR #6](https://github.com/ForgeWireLabs/repopact-core/pull/6) | DONE |
| Core integration navigation P1 | Replaced non-existent Core GitHub guide links with Workbench links and documented repository ownership. | Cross-repository link validation passed: 331 Markdown files; 31 external GitHub links were not checked live. | [Core docs PR #7](https://github.com/ForgeWireLabs/repopact-core/pull/7) | DONE |
| Core install, lifecycle, CLI, errors | Added CLI reference and troubleshooting guide; corrected adoption profile behavior. | Stable 3.1.3 CLI help checked for all 21 documented command groups. Disposable init, governance verification, evidence linking, completion, dashboard generation, and final tutorial validation passed. Adoption dry-run planned 38 records and left the target unchanged. | [Core docs PR #7](https://github.com/ForgeWireLabs/repopact-core/pull/7) | DONE |
| Workbench contributor/security guidance | Added Workbench-specific `CONTRIBUTING.md` and `SECURITY.md`, preserving exact Core pin and platform boundaries. | Source and historic platform evidence reviewed; Workbench frontend and Rust suites passed as recorded in the evidence manifest. | [Workbench PR #6](https://github.com/ForgeWireLabs/repopact-workbench/pull/6) | DONE |
| Workbench navigation and user guidance | Added docs index, user guide, troubleshooting; README now points to Workbench contribution/security guidance. | Link validation passed for 14 Markdown files; typecheck, 38 frontend tests, build, type generation check, and 238 Rust tests passed. | [Workbench PR #6](https://github.com/ForgeWireLabs/repopact-workbench/pull/6) | DONE |
| Split-repository references | Replaced wildcard/missing relative Decision 0061/0062 paths with explicit Core links. | Core and Workbench link/anchor checks passed with repository mappings; exact decision pages resolve. | [Core docs PR #7](https://github.com/ForgeWireLabs/repopact-core/pull/7); [Workbench PR #6](https://github.com/ForgeWireLabs/repopact-workbench/pull/6) | DONE |
| GitHub integration status | Guides label registration statements as the 2026-09-16 WI067 checkpoint and distinguish Android credential storage from Android GitHub commands. Official GitHub docs for PKCE, callback URLs, and token expiration are linked. | Compared source and WI067 records. No live app registration or settings were inspected or changed. | [Workbench PR #6](https://github.com/ForgeWireLabs/repopact-workbench/pull/6) | DONE |
| ForgeWire Labs public portfolio | Reconciled product links/maturity and regenerated social asset hashes, sizes, and dimensions without changing images. | Link and asset validation passed for 8 Markdown files; 25 external GitHub links were not checked live. Current product README/release sources reviewed. | [Portfolio PR #3](https://github.com/ForgeWireLabs/ForgeWireLabs/pull/3) | DONE |
| Feature-to-documentation coverage matrix | Matrix spans Core, Workbench, and portfolio maturity, verified examples, sources, and known limits. | Updated with current validation results and outstanding claims; see matrix. | [Core docs PR #7](https://github.com/ForgeWireLabs/repopact-core/pull/7) | DONE |
| Tutorials, CLI, platform and asset validation | Stable package checks, disposable tutorial/adoption preview, Core docs tests, Workbench suites, and social asset validation completed. | Core frozen check passed. Core validator still reports the two expired audit scopes. The full Core suite ran 385 tests: 9 failed and 3 skipped before dashboard regeneration. Six failure cases rerun after regeneration still failed only on those expired audit scopes; the full suite was not rerun. Desktop Core CLI and 390×844 mobile GitHub renders of the Core and Workbench docs indexes and portfolio README were inspected. Portfolio's three-column overview table caused mobile overflow and was replaced with a list; the updated page fits the 390 px viewport. | [Core security PR #6](https://github.com/ForgeWireLabs/repopact-core/pull/6); [Core docs PR #7](https://github.com/ForgeWireLabs/repopact-core/pull/7); [Workbench PR #6](https://github.com/ForgeWireLabs/repopact-workbench/pull/6); [Portfolio PR #3](https://github.com/ForgeWireLabs/ForgeWireLabs/pull/3) | PARTIAL |
| Research freshness at 2026-10-24 horizon | The 14-document research registry still has `verified_on=2026-09-24` and `review_by=2026-10-24`. | The review deadline is two days away. No dates were advanced; a substantive review and dated evidence remain due by 2026-10-24. | WI071 / research owner | PENDING |
| Governance and evidence | Added a concrete partial evidence run and linked it to RPS-020 without claiming the criterion satisfied. WI047 remains unchanged; its open criteria remain separate. | Core validation's two expired audit scopes remain visible. WI047 AC-7/8/10/11/12/14 were not closed by this documentation initiative. | [Core docs PR #7](https://github.com/ForgeWireLabs/repopact-core/pull/7) | PARTIAL |
| Independent reviewable pull requests | Changes are split into a security-only Core PR, Core documentation/evidence PR, Workbench docs PR, and portfolio metadata/docs PR. | All four PRs are open, attached, and their rendered desktop/mobile pages were inspected. No PR was merged. | [Core security #6](https://github.com/ForgeWireLabs/repopact-core/pull/6); [Core docs #7](https://github.com/ForgeWireLabs/repopact-core/pull/7); [Workbench #6](https://github.com/ForgeWireLabs/repopact-workbench/pull/6); [Portfolio #3](https://github.com/ForgeWireLabs/ForgeWireLabs/pull/3) | DONE |

## Preserved state

- The prior correction in the `C:\Projects\repopact` historical checkout is
  unchanged and has not been committed or represented as validated.
- These changes are in fresh clones of `ForgeWireLabs/repopact-core`,
  `ForgeWireLabs/repopact-workbench`, and `ForgeWireLabs/ForgeWireLabs`.
- No credentials, GitHub App registration settings, release configuration,
  package versions, or product behavior were changed.
