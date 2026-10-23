# RepoPact Engineering Readiness Report — 2026-10-22

> The matrix and follow-up findings below preserve the 2026-10-22 review snapshot. The dated 2026-10-23 update at the end records current PR states, the corrected audit-date interpretation, and the Workbench reproducibility investigation.

This review uses the existing PRs and authoritative work items. It records partial evidence as partial and leaves product and publication gates open. A live GitHub recheck on 2026-10-22 confirms all four documentation PRs are open and mergeable; no PR was merged by this review.

## Readiness matrix

| Gate | Owner | Dependencies | Current state | Required proof | Evidence | Blocker | Next action |
|---|---|---|---|---|---|---|---|
| Core security policy PR #6 | governance/docs | None | PASS; open and mergeable; no Actions run/review | Direct usable private reporting and accurate security boundary | PR `2125b97`; GitHub setting `enabled=true` | None in scope | Owner review and merge |
| Workbench documentation PR #6 | docs/tooling | Core #6 | PASS; open and mergeable; no Actions run/review | Local docs validation, source reconciliation, live security form | PR `74462c6`; 38 frontend tests, 245 Rust tests | Core #6 must precede the cross-link becoming current | Owner review and merge after Core #6 |
| ForgeWire Labs portfolio PR #3 | docs | None | PASS; open and mergeable; no Actions run/review | Verified public links and evidence inventory | Current PR head `314a9f8` | None in scope | Owner review and merge when convenient; this PR is independent |
| Core documentation PR #7 | docs/governance | Core #6 and Workbench #6 | PARTIAL; PR was open and GitHub reported mergeable at the follow-up check, but dependencies are not integrated | Local-link validation, audit evidence, fresh-base recheck | GitHub head at follow-up check `240c19f`; current report/evidence additions validated on the branch | Core #6 and Workbench #6 are not merged; the branch must be refreshed and rechecked after both land | Keep this existing PR; after Core #6 and Workbench #6 merge, refresh against Core `main`, recheck links and validation |
| Audit scopes 026/027 | governance/evidence | WI075 scope-specific review | PASS on Core #7 branch | Current dashboard/read-only behavior; 2.2.0 tag/package identity; full Core gates | `20261022-075-forgelink-core-full-rereview`; 386 tests; conformance | Main remains unchanged until Core #7 merges | Merge only through normal PR review |
| RPS-011 platform acceptance | tooling/evidence | Native platform availability | PARTIAL | Native Windows/Linux and supported Android evidence; bound macOS/iOS claims | Existing Android API 35 x86_64 evidence; Workbench Windows build and startup | No native Linux GUI or macOS/iOS host; Android coverage excludes some ABI and permission cases | Keep pending; run only on supported native hosts |
| RPS-012 admission correctness | tooling/security | Protected-service authorization | PARTIAL; production confinement blocked | Adversarial fail-closed coverage plus native process/path confinement proof | Full suite; `run_admission_platform_conformance` | Protected Windows service absent; no process/path confinement proof | Preserve pending; any privileged service test needs separate operator authorization |
| RPS-015 reproducibility | tooling/evidence | Clean exports and locked builds | PARTIAL; Core source exports reproduce, Workbench release executable does not | Independent Core and Workbench builds with artifact manifests and repeated-build evidence | Two normalized Core clean exports match; three Workbench clean release builds succeeded from byte-identical source archives | All three Workbench executable SHA-256 values differ, including two builds from the same extraction path | Keep pending; investigate and fix the Workbench release nondeterminism before claiming reproducibility |
| RPS-016 release independence | tooling | RPS-015 and compatibility probes | PARTIAL | Stable/dev Core identity, Workbench identity/pin, incompatible-version behavior, offline install, upgrade and rollback | Core 3.1.3 and 3.1.3.dev1 offline validations; Workbench 0.1.0, Core Rust pin `1c1f32a` | Dynamic compatibility matrix and unsupported Workbench release combinations remain unproven; Workbench Releases page has no installer | Keep pending; test exact supported/unsupported combinations when the app ships an installer |
| RPS-018 Proving Ground | evidence/research | Public consumer snapshot | PARTIAL | Preserved historical evidence and pinned benchmark setup; safe harness checks | Public archive `54edc03`; 3.0.2 wheel hash matches PyPI; 96 tests and two mock self-tests pass | Current archive validation reports 15 governance/evidence findings; no empirical model run was authorized | Keep pending; consumer owner reconciles current findings; do not run paid models |
| RPS-019 ForgeWire adopter | governance/tooling | Separately authorized consumer work | PARTIAL; migration blocked | Exact consumer pin/protocol, bounded integration and rollback plan | Core stable/dev handshake tests and Workbench immutable pin | Private ForgeWire systems were not accessed; actual migration is separately gated | Use the bounded test plan below only after owner authorizes the consumer work |
| RPS-020 documentation acceptance | docs/evidence | Core #6, Workbench #6, refreshed Core #7 | BLOCKED on PR sequence | Source consistency, user examples, local validation, live links, dashboard and linked evidence | Local link checks and branch evidence; docs review and research review | Workbench guides and corrected Core security policy are not yet on their base branches | After merges, repeat links/source/examples checks and then decide RPS-020 |
| RPS-022 rollback | tooling/evidence | Verified artifacts | PARTIAL | Rollback from verified artifacts, missing/corrupt/incompatible and interruption cases | Stable → dev → stable passed; repository hash preserved; missing/corrupt rejection passed; an unreachable-index upgrade was rejected and 3.1.3 remained installed | Power-loss interruption and Workbench native-app recovery not exercised | Keep pending; test only in disposable installations |
| WI047 AC-7/8/10/11/12/14 | governance/tooling/docs | Focused implementation branch and evidence | IMPLEMENTED on separate active branch; overall validation partial | Mapped freshness detection, generated-doc staleness, doctor/audit, parity, regression/conformance, adopter proof | Commit `102a7fc`; Rust/Python conformance 43/43 each; WI050 8/8; mapping inspect clean; full Python suite 373 pass, 9 baseline failures, 3 skips | WI047's baseline validation used the then-current Core main before Core #7 integrates the substantive WI075 re-reviews. Core #7 records scopes 026/027 as `last_reviewed=2026-10-22`, `next_review=2027-01-20`; do not advance those dates again without another substantive review. WI075's completed state covers only its bounded ForgeLink adopter re-review; wider ForgeLink governance remains partial, with 16 unrelated expired audit scopes | Review the focused draft PR separately; keep WI047 active and RPS-020 pending |
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

The October 24 research-freshness deadline received a substantive review on October 22 across 14 registered documents; the next review is November 21. No experiments or historical results were rerun. Manuscript source commit `6d782116fc9762da00439cf079d0f50585fea52` remains unavailable; public `v3.1.0` is a different commit and was not substituted. An earlier report statement that Moto commit `0adb522ab4b3e4c33b0beb101ae637ad737c5e91` had been recovered locally was incorrect. Current authorized RepoPact checkouts and backups contain no such object; GitHub exposed private-repository metadata only, and no source tree was fetched or substituted.

WI047 remains independently active. WI021 AC-2 and AC-4 remain operator-gated, WI071 stays proposed, and WI063 remains active. No arXiv submission, Show HN post, new release, paid benchmark, private migration, hosted workflow, or automatic merge was performed.

## PR integration order

Core #6 → Workbench #6 → refresh/revalidate Core #7 against the updated Core base. ForgeWire Labs #3 is open and independent; it can be reviewed separately. The Workbench guide targets already resolve on `main`; Workbench #6 updates their content. Core #7 still needs a post-dependency-merge content and base recheck. No merge is performed by this report. A prior draft incorrectly said Portfolio #3 had merged and that the Workbench guide paths were branch-only; GitHub API checks corrected both statuses. The Core and Workbench Releases collections currently return no entries.


## Live destination checks

Targeted live checks opened the Core PyPI installation page, Core contribution guide, Workbench GitHub integration guides, and cited arXiv records. The GitHub Releases API returned no entries for either Core or Workbench; Core installs from PyPI 3.1.3, and Workbench has no installers, consistent with its README. The Workbench guide paths resolve on `main`; Workbench #6 updates their content, so Core #7 retains a post-merge content/base recheck. Security form settings were verified through GitHub's private-vulnerability-reporting API (`enabled=true` for both repositories); the offline Markdown checker still reports external GitHub links as unchecked.

## Follow-up evidence — 2026-10-22

This continuation is recorded in [`20261022-074-readiness-followup`](../../../evidence/runs/20261022-074-readiness-followup.json). A live GitHub API recheck confirms Core #6 (`2125b97`), Core #7 (`240c19f`), Workbench #6 (`74462c6`), and ForgeWire Labs #3 (`314a9f8`) are open and GitHub marks each mergeable. Their three repositories report no Actions runs; all four head commits have zero commit statuses and none has a submitted review. Mergeability is not a substitute for the Core #7 dependency refresh. The recommended sequence remains Core #6, Workbench #6, then refresh and revalidate Core #7; ForgeWire Labs #3 is independent.

GitHub private vulnerability reporting is enabled for both Core and Workbench. Their public Security pages display the **Report a vulnerability** entry point, and each policy links directly to that repository's private advisory form. An unauthenticated visit to the submission endpoint returns GitHub's sign-in page; reporters need a GitHub account to submit. No personal contact information or external account was added.

Targeted live destination checks covered the stable Core installation page, both Releases destinations, vulnerability reporting, contributing and documentation paths, cited research records, and Core/Workbench cross-repository navigation. No demonstrated destination failure required a link correction. The local Markdown checker verifies local links only; its current report lists 35 Core URLs (the original 34 plus the separately live-checked PR #8 reference), 9 Workbench URLs, and 25 portfolio URLs as unchecked by that offline result.

### RPS-015 — 2026-10-22 finding: Workbench release output was not byte-reproducible

Three clean `cargo build --locked --release --package repopact-desktop --features custom-protocol -j 2` executions succeeded at Workbench PR #6 source commit `74462c67021a2ec28a6b46b3e7240d2545113a82`. Two separately archived source exports were byte-identical (each SHA-256 `2B9A0146E362DEA7E48CA40A91D749BC2A5D152F481CFBB97673A321BA020EC0`), and both first ran `npm ci` and `npm run build` successfully. Each executable was 26,998,784 bytes with ProductVersion `0.1.0` and FileDescription `RepoPact Workbench`, but the hashes were:

- First build at extraction path A: `6B8E9FFD8ACE9192B9CF4E07947D74CF8403EFB5E513F24FF5B8CD08F9D3E491`.
- Second build at extraction path B: `71A278754E548A187F742124D4A26E940BBD56278C46C77BFE0F224D6266F99B`.
- Third build at extraction path A after cleaning its target: `601C05584ABBBAB18CE83C13629DB5A26957209FAFAC4C14824D876EF9288838`.

Since the same-path builds also differ, this is not explained solely by extraction-path variation. As of the 2026-10-22 snapshot, the cause was unresolved; no normalization or reproducibility claim was applied. RPS-015 remained partial. The two retained comparison executables are in the disposable `repopact-workbench-repro-20261022` scratch directory; no project source was changed by those builds.

### RPS-022 — offline index failure probe

In a disposable Python 3.12.4 environment, an upgrade attempt against unreachable `http://127.0.0.1:9/simple` failed as expected and the verified stable `repopact==3.1.3` remained installed. Running from a neutral directory avoided source-checkout import shadowing; both `repopact.exe --version` and `python -m repopact.cli --version` reported `3.1.3`. This adds an unavailable-remote result but does not cover power-loss interruption or Workbench native-app recovery, so RPS-022 remains partial.

### Research-source correction and publication metadata

The exact manuscript snapshot object `6d782116fc9762da00439cf079d0f50585fea52` is absent from the inspected RepoPact checkouts and backups, and the public RepoPact GitHub API returns no commit (HTTP 422). Public tag `v3.1.0` resolves to `a87ea20e808ba67c04f625d7cc44c7f8f92487eb`, which is different and is not a substitute. The previously recorded Moto SHA `0adb522ab4b3e4c33b0beb101ae637ad737c5e91` has no local object in those RepoPact repositories; GitHub exposed private-repository and commit metadata only. No Moto source tree was fetched. These findings correct earlier report wording and do not alter the historical snapshots or experiments.

The October 24 research-freshness checkpoint was substantively reviewed on October 22 across 14 registered documents; the next review is November 21. PyPI's public 3.1.3 description still claims that GitHub's Latest Release entry tracks the PyPI stable version, while both GitHub Releases API collections are empty. The Core source wording has been corrected for a later authorized package build; the published PyPI description cannot be corrected without a new release.

### Current closeout state — 2026-10-22 snapshot

Core #6, Workbench #6, and ForgeWire Labs #3 pass their scoped local and live-link checks and are ready for human review. Core #7's reviewed content and existing-branch validation pass, but its integration gate remains **PARTIAL / BLOCKED on the PR sequence** until Core #6 and Workbench #6 land and the existing branch is refreshed against the resulting Core base. No PR was merged. WI047's implementation is tracked in its separate [draft Core PR #8](https://github.com/ForgeWireLabs/repopact-core/pull/8); refresh that branch after Core #7 because it also touches generated governance artifacts. RPS-020 remains pending, WI047 remains active, and WI021, WI071, and WI063 retain their independent governance states.

## Readiness integration and reproducibility addendum — 2026-10-23

This dated addendum supersedes the October 22 snapshot for current status. Earlier evidence and historical observations remain unchanged. In particular, Core #7 did substantively re-review scopes 026 and 027: both registry entries now say `last_reviewed=2026-10-22` and `next_review=2027-01-20`. These updates are part of Core #7's actual diff, not merely planned work. Do not change either date again without another substantive review. WI075 is complete only for its bounded ForgeLink adopter re-review; it does not certify ForgeLink's wider governance as clean. Sixteen unrelated ForgeLink audit scopes remain expired.

### Current integration state

| Change | State at this recheck | Dependency and next action |
|---|---|---|
| Core #6 — security policy | OPEN, GitHub reports mergeable | Review/merge first. GitHub private vulnerability reporting is enabled; Core #6 adds the exact repository advisory form to `SECURITY.md`. |
| Workbench #6 — documentation | OPEN, GitHub reports mergeable | Review/merge after Core #6. Workbench `main` currently lacks `SECURITY.md` and `CONTRIBUTING.md`; those paths resolve at the PR head, so the main-branch 404s are pre-merge gaps. |
| ForgeWire Labs #3 — portfolio | OPEN, GitHub reports mergeable | Independent of the Core dependency sequence; review separately. |
| Core #7 — docs/evidence | OPEN, GitHub reports mergeable | After Core #6 and Workbench #6 are integrated, refresh this existing branch from Core `main`, reconcile overlaps and rerun the required checks before review/merge. |
| Workbench #7 — Windows PE reproducibility | OPEN, DRAFT | Focused correction PR created after binary comparison; it is not a documentation dependency and does not close RPS-015. |
| Core #8 — WI047 mapping implementation | OPEN, DRAFT | Keep draft until Core #7 is integrated; then refresh and perform the separate WI047 review sequence below. |

The intended documentation order remains Core #6 → Workbench #6 → refresh and revalidate Core #7 against the updated Core base. ForgeWire Labs #3 is independent. No PR was merged. GitHub's `mergeable` result is not a substitute for review, passing checks, dependency integration, or acceptance. Core #7 at the start of this recheck was `e68099b`; the evidence records the exact branch tips inspected. RPS-020 remains pending until the full criteria are checked against the integrated default branches.

GitHub currently reports zero Actions runs, zero commit statuses/check-runs, and zero submitted reviews for the four documentation PRs. The new Workbench #7 draft also has no checks or submitted review. These are absent checks, not passing remote CI.

| Validation at the current Core documentation branch | Result | Scope |
|---|---|---|
| Dashboard generation | PASS | Regenerated after adding the new WI074 evidence reference. |
| Canonical governance validation | PASS | Current documentation branch, with the local Core engine explicitly selected. |
| Full Python test suite | PASS | 386 run; 383 passed, 3 skipped, 0 failed. |
| Conformance | PASS | Rust 37/37; Python compatibility 37/37; WI050 8/8. |
| Offline documentation checker | PASS | 332 Markdown sources and local links pass; 37 external GitHub URLs remain unchecked. |
| Frozen-surface check | PASS | No frozen-surface changes detected. |
| Git whitespace check | PASS | No whitespace errors in the reviewed branch diff. |
| Documentation integration gate | BLOCKED | Core #6 and Workbench #6 have not been merged; Core #7 has not yet been refreshed against their integrated base. |

These local passes describe the current pre-merge branch and do not replace the full post-merge validation required after dependencies land.

### Core audit and validation classification

On unchanged Core `main` at `9a3e51f5669fc2bc4d513d1c64929d3c5636d4ac`, all nine reported failures were reproduced. Each was caused by expired audit scopes 026/027; five also encountered the stale derived dashboard. The same nine test cases pass on Core #7 after the substantive reviews and dashboard regeneration. This establishes that the failures pre-existed the documentation changes and are governance-state failures, not regressions introduced by the PRs. It does not erase the baseline findings or justify date renewal without review.

On the reviewed Core #7 branch, canonical validation passed; the complete Python suite ran 386 tests (383 passed, 3 skipped, 0 failed); conformance passed Rust 37/37, Python compatibility 37/37, and WI050 8/8. The branch's frozen-surface check passed with no frozen paths changed, and its offline documentation checker passed local links. These are branch results before dependency integration; they do not satisfy the required post-merge Core #7 refresh and validation.

### Workbench RPS-015 root cause and scoped correction

The three original clean release builds were from byte-identical source exports, each 26,998,784 bytes, with hashes `6B8E9FFD8ACE9192B9CF4E07947D74CF8403EFB5E513F24FF5B8CD08F9D3E491`, `71A278754E548A187F742124D4A26E940BBD56278C46C77BFE0F224D6266F99B`, and `601C05584ABBBAB18CE83C13629DB5A26957209FAFAC4C14824D876EF9288838`. Comparing the two retained files found exactly 24 differing bytes: two bytes of the PE COFF timestamp, two bytes in each of three `.rdata` debug-directory timestamps, and the 16-byte RSDS CodeView/PDB GUID. The timestamps matched link time. Application sections, resources and the stable `repopact_desktop.pdb` basename matched, identifying MSVC link metadata as the cause rather than source-path or resource drift.

Workbench draft PR #7 (`codex/rps015-windows-pe-reproducibility`) adds target-scoped `/Brepro` for only the `repopact-desktop` binary on Windows MSVC. No binary post-processing or normalization is used. Two separate clean release builds from the correction branch produced byte-identical 26,998,784-byte executables with SHA-256 `21211B7050BD12F4403DD6B849E772C5CA0BBFABE108EE8E7A4433874FDF207F`; comparison found zero changed bytes and matching PE section hashes. The evidence records Windows 10 Pro 10.0.19045, Rust/Cargo 1.96.0, MSVC linker 14.44.35225.0, target `x86_64-pc-windows-msvc`, and the exact commands. Frontend install/build and focused formatting checks passed. Workspace-wide `cargo fmt --all -- --check` still fails on unrelated pre-existing formatting in `rust/crates/repopact-mobile-acquisition/src/operation.rs`; that file was not changed. This scoped correction supports the PE reproducibility finding but does not satisfy all of RPS-015's independent source-export, artifact identity, wheel-content, provenance, and platform-normalization criteria. RPS-015 remains pending.

### Core #8 and independent governance

Core #8 stays draft and unrefreshed until Core #7 is integrated. Then refresh it from the current Core default branch, reconcile overlapping audit, documentation, specification and dashboard changes, preserve the substantive WI026/WI027 reviews, regenerate derived records, and recompute documentation snapshots only after reviewing each mapped source/documentation relationship. Recheck Rust/Python semantic parity, conformance and the full Core suite; review the mapping contract's security and backward compatibility. The frozen-schema change has an existing operator-authorization record in `evidence/runs/20261022-047-documentation-mapping-freshness.json`; verify that authorization still covers the exact integrated diff. WI047 remains active until its acceptance criteria and merged implementation receive independent sign-off.

### Remaining readiness and publication gates

RPS-011 remains limited to the tested Windows and Android API 35 x86_64 profiles; it does not claim native Linux/macOS/iOS acceptance. RPS-012 remains pending without protected-service and process/path-confinement evidence. RPS-016 retains unverified compatibility combinations and installer-based upgrades. RPS-018 and RPS-019 retain their scoped consumer, benchmark, and authorization limits. RPS-022 still lacks interruption recovery and Workbench native-app recovery. These gates remain pending and are not implied by documentation or reproducibility PRs.

The October 24 research-freshness deadline remains active. The substantive evidence-based review of all 14 registered documents was completed October 22, with next review November 21; no experiments or historical results were rerun. No package, installer, research paper, or release was published. No native-platform, protected-service, consumer-migration, or interruption-recovery acceptance is claimed.

The Core offline documentation checker checks local paths only; the current run covers 332 Markdown sources and leaves 37 external GitHub URLs unchecked. Separate live requests verified the installation and releases destinations, private security reporting entry points, contribution and documentation pages, cited research references, and Core/Workbench navigation. GitHub private reporting is enabled for both repositories, and submitting an advisory requires a GitHub sign-in. No demonstrated permanent public-link failure required correction. The Workbench `main` 404s for `SECURITY.md` and `CONTRIBUTING.md` are expected to resolve when Workbench #6 merges; the exact paths resolve on that PR head.

RPS-020 remains pending until integrated-branch review satisfies its complete acceptance wording. WI047 remains independently active. Neither item closes when any documentation PR merges.
