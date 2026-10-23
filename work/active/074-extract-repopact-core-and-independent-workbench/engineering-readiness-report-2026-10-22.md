# RepoPact Engineering Readiness Report — 2026-10-22

This review uses the existing PRs and authoritative work items. It records partial evidence as partial and leaves product and publication gates open. A live GitHub recheck on 2026-10-22 confirms all four documentation PRs are open and mergeable; no PR was merged by this review.

## Readiness matrix

| Gate | Owner | Dependencies | Current state | Required proof | Evidence | Blocker | Next action |
|---|---|---|---|---|---|---|---|
| Core security policy PR #6 | governance/docs | None | PASS; open and mergeable; no Actions run/review | Direct usable private reporting and accurate security boundary | PR `2125b97`; GitHub setting `enabled=true` | None in scope | Owner review and merge |
| Workbench documentation PR #6 | docs/tooling | Core #6 | PASS; open and mergeable; no Actions run/review | Local docs validation, source reconciliation, live security form | PR `74462c6`; 38 frontend tests, 245 Rust tests | Core #6 must precede the cross-link becoming current | Owner review and merge after Core #6 |
| ForgeWire Labs portfolio PR #3 | docs | None | PASS; open and mergeable; no Actions run/review | Verified public links and evidence inventory | Current PR head `314a9f8` | None in scope | Owner review and merge when convenient; this PR is independent |
| Core documentation PR #7 | docs/governance | Core #6 and Workbench #6 | PASS on updated local branch; integration recheck pending | Local-link validation, audit evidence, fresh-base recheck | Updated local head `8ab81c`; evidence in this branch | Remote PR head still requires update; Workbench guide paths are on its PR branch, not Workbench `main`; Core #6 is not merged | Push the existing branch; after Core #6 and Workbench #6 merge, refresh against Core `main`, recheck live paths and validation |
| Audit scopes 026/027 | governance/evidence | WI075 scope-specific review | PASS on Core #7 branch | Current dashboard/read-only behavior; 2.2.0 tag/package identity; full Core gates | `20261022-075-forgelink-core-full-rereview`; 386 tests; conformance | Main remains unchanged until Core #7 merges | Merge only through normal PR review |
| RPS-011 platform acceptance | tooling/evidence | Native platform availability | PARTIAL | Native Windows/Linux and supported Android evidence; bound macOS/iOS claims | Existing Android API 35 x86_64 evidence; Workbench Windows build and startup | No native Linux GUI or macOS/iOS host; Android coverage excludes some ABI and permission cases | Keep pending; run only on supported native hosts |
| RPS-012 admission correctness | tooling/security | Protected-service authorization | PARTIAL; production confinement blocked | Adversarial fail-closed coverage plus native process/path confinement proof | Full suite; `run_admission_platform_conformance` | Protected Windows service absent; no process/path confinement proof | Preserve pending; any privileged service test needs separate operator authorization |
| RPS-015 reproducibility | tooling/evidence | Clean exports and locked builds | PARTIAL | Independent Core and Workbench builds with artifact manifests and repeated-build evidence | Two normalized Core clean exports reproduce; Workbench locked build SHA recorded | Repeated independent clean Workbench build remains outstanding | Run a second clean Workbench build and compare artifact identity |
| RPS-016 release independence | tooling | RPS-015 and compatibility probes | PARTIAL | Stable/dev Core identity, Workbench identity/pin, incompatible-version behavior, offline install, upgrade and rollback | Core 3.1.3 and 3.1.3.dev1 offline validations; Workbench 0.1.0, Core Rust pin `1c1f32a` | Dynamic compatibility matrix and unsupported Workbench release combinations remain unproven; Workbench Releases page has no installer | Keep pending; test exact supported/unsupported combinations when the app ships an installer |
| RPS-018 Proving Ground | evidence/research | Public consumer snapshot | PARTIAL | Preserved historical evidence and pinned benchmark setup; safe harness checks | Public archive `54edc03`; 3.0.2 wheel hash matches PyPI; 96 tests and two mock self-tests pass | Current archive validation reports 15 governance/evidence findings; no empirical model run was authorized | Keep pending; consumer owner reconciles current findings; do not run paid models |
| RPS-019 ForgeWire adopter | governance/tooling | Separately authorized consumer work | PARTIAL; migration blocked | Exact consumer pin/protocol, bounded integration and rollback plan | Core stable/dev handshake tests and Workbench immutable pin | Private ForgeWire systems were not accessed; actual migration is separately gated | Use the bounded test plan below only after owner authorizes the consumer work |
| RPS-020 documentation acceptance | docs/evidence | Core #6, Workbench #6, refreshed Core #7 | BLOCKED on PR sequence | Source consistency, user examples, local validation, live links, dashboard and linked evidence | Local link checks and branch evidence; docs review and research review | Workbench guides and corrected Core security policy are not yet on their base branches | After merges, repeat links/source/examples checks and then decide RPS-020 |
| RPS-022 rollback | tooling/evidence | Verified artifacts | PARTIAL | Rollback from verified artifacts, missing/corrupt/incompatible and interruption cases | Stable → dev → stable passed; repository hash preserved; missing and corrupt artifact rejection passed | Power-loss interruption and Workbench native-app recovery not exercised | Keep pending; test only in disposable installations |
| WI047 AC-7/8/10/11/12/14 | governance/tooling/docs | Focused implementation branch and evidence | IN PROGRESS; kept independently governed | Mapped freshness detection, generated-doc staleness, doctor/audit, parity, regression/conformance, adopter proof | Existing AC-1..6/9/13 evidence plus the separate implementation branch and its validation | Implementation is not part of these documentation PRs; base validation still carries the known 026/027 findings until Core #7 integrates | Review the focused change separately; do not close WI047 or RPS-020 because documentation PRs merge |
| Research freshness deadline | research | Substantive review by 2026-10-24 | PASS for this review checkpoint | Evidence-based review of 14 registered documents | `20261022-074-research-freshness-review`; reviewed 2026-10-22, next review 2026-11-21 | Historical source commit `6d782116` remains unavailable | Keep the source limitation explicit; do not substitute `v3.1.0` or rerun historic experiments |
| RPS-020 governance separation | governance | PR sequence | PENDING | Final documentation acceptance evidence after branch integration | This report and PR evidence | Documentation PR merge cannot close WI047 or change WI074 by itself | Keep WI047 and RPS-020 independently governed |
| WI021 public launch | governance | Operator approval/publication | PENDING | Full AC-1 review; real arXiv URL for AC-2; authorized Show HN evidence for AC-4 | Existing launch evidence; PyPI AC-3 remains satisfied | AC-2 and AC-4 are operator-gated; AC-1's complete wording is not established by the portfolio PR alone | Keep open; no publication performed |
| WI071 paper/release closeout | governance/research | WI070 and WI063 | PROPOSED | ROG-028/034/038, paper source/package readiness, later operator action | Existing proposed record | WI063 remains active with research/UX closeout criteria pending | Do not activate, submit, or cut 3.1.4 on this evidence |

## Core baseline and audit scopes

On unchanged Core `main` (`9a3e51f`), the exact nine failing validation tests were reproduced. All nine were blocked by expired scopes 026/027; five also saw a stale dashboard (186 committed manifest rows versus 187 authoritative manifests). No runtime source change caused these failures. The revised Core #7 branch re-reviewed both scopes, updated the derived dashboard, passed the exact nine cases, canonical validation, and the full suite (386 run, 383 passed, 0 failed, 3 existing skips). Rust conformance passed 37/37, Python compatibility 37/37, and WI050 8/8. The audit records were advanced to 2026-10-22 only after those reviews and gates; historical completed work items were not edited.

The final Core #7 branch update was pushed to its existing PR as `0f48947449820582bb1e4b0453bc5c0ea1a222b0`; its description now records the corrected test counts, external-link boundary, and merge order. The branch update changes evidence and governance records, not runtime implementation.

The current Core PR branch also built two independent clean exports. Normalized wheel and sdist bytes matched. Wheel SHA-256: `e4ace34784fc2b356420ae1ecb13bfbdf74bcf17bda245421aefa15ebded6efe`. Sdist SHA-256: `5c62f1348f3fc5c8995ca7d405a6bc1df1a78a527a0b906e9caa7487ed496c73`.

## Platform and admission boundaries

Workbench branch checks passed: frontend typecheck, 38 frontend tests, production build, generated-type check, 245 Rust tests (10 ignored), and Windows desktop build (SHA-256 `6fd5822b61b11c90411823343a583c5bfc69953a1e77bac95097bf6ff9d57ac6`). Prior Android API 35 x86_64 launch/flow evidence remains bounded to that emulator profile; it does not establish every ABI or OS permission-denial scenario. No native Linux graphical, macOS, or iOS run is claimed.

The Core admission platform harness passed its testing-only pre-action matrix, including denial before child creation and unchanged sentinel hashes. It also reported the production Windows service absent and `process_confinement=false` / `path_confinement=false`. No service, system policy, or persistent machine configuration was changed.

## Compatibility and rollback

| Component | Verified identity | Result |
|---|---|---|
| Core stable | PyPI `3.1.3` | Offline install and validation passed from a neutral working directory |
| Core development | `3.1.3.dev1`, Core #7 branch | Offline install and validation passed |
| Workbench | app `0.1.0`; Cargo.lock pins Core commit `1c1f32ae619e8135227acfa87393696ff608951c` | Locked build/tests passed; runtime does not install a second Python validator |
| Workbench downloadable release | Separate repository Releases page | No installer listed; historical combined-repository artifacts are not treated as current Workbench releases |

In a disposable copy, Core 3.1.3 → 3.1.3.dev1 → 3.1.3 validated at every step. A governed work-item record retained SHA-256 `df7b14169deab26ba5f6cfabb88f3d874610dffc715cd50095681c52af2e2609`; the clone remained clean. Missing 3.1.4 and malformed-wheel installs were rejected while the stable environment continued to validate. This does not simulate power loss during installation or Workbench native-app recovery.

## Proving Ground and ForgeWire boundaries

Git transport for the public Proving Ground repository returns 403, while GitHub's public API reports it public and supplies immutable archive commit `54edc0305c4f577a00a436a76902da098cffa5d8` (archive SHA-256 `960f9d39d1ed16edbf8786d3eabf0a636c5ac25c3c233be37c16b782e72467a1`). Its current dependency file pins `repopact==3.0.2`; the downloaded wheel SHA-256 matches PyPI. With the declared `tiktoken==0.9.0` test dependency in an isolated environment, 96 tests pass. The deterministic 24-task mock benchmark and S5 drift self-tests pass without model/API calls. The pinned 3.0.2 validator reports 15 current archive findings (expired audit, stale dashboard, and historical work/evidence references). The consumer repository was not changed and no empirical campaign ran.

For RPS-019, the bounded compatibility plan is: (1) after consumer-owner authorization, record its exact current Core package and protocol pin; (2) export a clean shadow copy and install the candidate from a hash-verified wheelhouse; (3) run the consumer's own validator, Core conformance, and acceptance suite without rewriting its records; (4) compare governed-record hashes and reports; (5) roll back by reinstalling the prior verified package and repeat validation. Core handshake mismatches fail closed. This is a plan based on public evidence, not proof of private ForgeWire integration or authorization to migrate it.

## Research and publication boundaries

The October 24 research-freshness deadline received a substantive review on October 22 across 14 registered documents; the next review is November 21. No experiments or historical results were rerun. Manuscript source commit `6d782116fc9762da00439cf079d0f50585fea52` remains unavailable; public `v3.1.0` is a different commit and was not substituted. Moto commit `0adb522ab4b3e4c33b0beb101ae637ad737c5e91` is recovered locally, but no public distribution or migration was performed.

WI047 remains independently active. WI021 AC-2 and AC-4 remain operator-gated, WI071 stays proposed, and WI063 remains active. No arXiv submission, Show HN post, new release, paid benchmark, private migration, hosted workflow, or automatic merge was performed.

## PR integration order

Core #6 → Workbench #6 → refresh/revalidate Core #7 against the updated Core base. ForgeWire Labs #3 is open and independent; it can be reviewed separately. The live Workbench guide targets currently resolve on the Workbench PR branch, not `main`, and become permanent public destinations only after Workbench #6 merges. Core #7 therefore needs a post-dependency-merge link and base recheck. No merge is performed by this report. A prior draft incorrectly said Portfolio #3 had merged; the live GitHub recheck corrected that status.


## Live destination checks

Targeted live checks opened the Core PyPI installation page, current Core and Workbench Releases pages, Core contribution guide, Workbench PR-branch GitHub integration guide, and the cited arXiv records. GitHub currently lists no Workbench installer, consistent with its README. Core #7 points at the Workbench guides on `main`; those URLs remain a merge-order dependency because the new guide files are public on Workbench #6's branch but not yet on `main`. Security form settings were verified through GitHub's private-vulnerability-reporting API (`enabled=true` for both repositories); the offline Markdown checker still reports external GitHub links as unchecked.
