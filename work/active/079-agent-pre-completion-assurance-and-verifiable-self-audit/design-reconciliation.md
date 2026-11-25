# WI079 — Architecture and assurance boundary reconciliation

Date: 2026-11-22. This is the design baseline for the WI079 prototype, **not** a ratified change to the canonical RepoPact semantic contract. The operator has authorized engineering work; the item remains active and is not ready for completion.

## Existing machinery and what it actually proves

- `AGENTS.md` is a durable instruction surface. It may ask an agent to review its work but cannot force arbitrary model behavior or prevent external Git writes.
- `governance/verification.json` supplies ordered local checks with argv-only execution, timeouts, statuses, and optional evidence recording. `repopact verify --evidence-work-item` records *observed command invocations* and a best-effort Git commit/tree/dirty indicator, but not an exact dirty-tree fingerprint or proof that a self-review occurred.
- The canonical Rust validator checks authoritative work-item lifecycle/relationships, criterion evidence links, governance invariants, and relevant opt-in contracts. A linked evidence record need not demonstrate the semantic sufficiency of a human-written review; changing that would intersect WI044.
- H14 covers the **coverage, invocation and effectiveness of an enforcement checkpoint**. An optional step existing on disk does not prove a checkpoint was invoked before merge, promotion, or an agent response. An agent with the same Git write principal can generally edit its own review and bypass instruction-only guards.
- WI044 separately investigates **typed capability/product-completion and replacement-cutover claims**, not whether a generic agent reviewed a particular diff. Reuse evidence references where possible; do not subsume or duplicate WI044.

## Options assessed

| Placement | Benefit | Limitation | Disposition |
| --- | --- | --- | --- |
| AGENTS.md only | Immediate, provider-neutral, no new code or migration | Unverifiable self-report; can be ignored | Already drafted for 20 active repos; keep as guidance only |
| Optional local helper + existing verification profiles | Deterministic candidate freshness and review-path coverage; no new canonical schema; executable by `repopact verify` on hosts that opt in | Self-review still self-reported; external report artifact and host checkpoint require operator wiring; no remote coverage guarantee | **Chosen exploratory pilot** |
| New canonical Rust evidence/closeout policy | Could reject transitions lacking a current typed review record | Needs candidate identity, immutable attestation semantics, dirty-tree treatment, trust/approval policy, migration and conformance changes; prematurely duplicates WI044/H14 | Defer pending pilot/benchmark evidence and explicit design approval |
| Mandatory independent model reviewer | Potentially stronger defect discovery | Cost, availability, shared failure modes; not provider-neutral and no proof of quality | Optional risk policy, not a universal requirement |

## Pilot contract (implemented for experimental use only)

`repopact self-audit capture --base <ref> --root <repo>` emits a JSON template containing a candidate fingerprint. It covers the resolved base and HEAD, Git index, porcelain status, committed/staged/unstaged path changes relative to the base, nonignored untracked paths, and content bytes or symlink targets of these paths. Deletions are represented explicitly. The worktree identity is hashed rather than leaked as an absolute host path.

The agent reviews the changed paths and explicitly fills `review`: actor, outcome, exact reviewed-path set, bounded concrete findings and residual limits. A report starts **incomplete**, not passing. `repopact self-audit check --report <external-path>` recomputes the candidate and checks for equality, matching reviewed paths, and no declared blocking findings. Reports must reside **outside** the candidate working tree to avoid a self-referential report changing the reviewed candidate. The command is read-only and does not grant any authority.

The pilot returns `self-report-current` and `assurance=instruction-only` on success; it deliberately does **not** return a status implying that code is correct, that GitHub branch protection ran, or that a model truly examined each line. Its exit status can be included as one **required** step in an opted-in, host-defined RepoPact verification profile, alongside the real build/test/security checks. Such a local verification result shows invocation on that host and can be recorded by existing evidence APIs. It does not prove remote checkpoint coverage.

The report intentionally does not cover ignored files or arbitrarily large/inaccessible changed paths; those are outside coverage or result in an incomplete/error outcome, never a silent success. Tests must exercise Git renames/deletions, stage/unstage drift, symlinks, large files, invalid reports, and path traversal.

## Boundaries and security

- An agent capable of writing both the report and repository can forge the report, spoof its `actor`, alter findings, or avoid calling the helper. This pilot detects drift, **not adversarial tampering by a malicious same-principal process**.
- A passing `check` result is insufficient to accept a work item; the real profile, criterion-level evidence, operator authority, and independent review remain independent requirements.
- RepoPact Core is not an arbitrary-process filesystem sandbox. The pilot does not upgrade instruction-only to `pre-action` or `sandbox/process-enforced`.
- The external artifact is not durable automatically; before closeout, its provenance, digest, and retained evidence should be linked through the existing RepoPact evidence scheme. Avoid pretending an external report path will survive a different host.
- Any attempt to enforce review before a covered Git promotion, work completion, or merge must first establish checkpoint **coverage + invocation + effectiveness** (H14), including negative bypass tests. It cannot be claimed by appending a verification step or `AGENTS.md` alone.
- The pilot is additive and opt-in; no historical completion criteria, release flags, or existing adopters change merely because the source helper exists.

## Prospective benchmark, before any effectiveness claim

Preregister a fixed set of small and risk-bearing tasks with locked model, prompt, code baseline, and test harness. Compare:
1. ordinary tests/verification without structured audit;
2. bounded self-audit with candidate freshness verification;
3. separate independent reviewer with matched scope.

Record escaped functional/security defects, false positives, post-review regressions, churn (unnecessary modified lines and diff volume), end-to-end latency, token/compute cost, and successful completion. Repeat across multiple seeds/tasks and report negative results. Do not select tasks or adjust thresholds after seeing comparative results.

**Decision gate:** after the pilot's executable validation and benchmark, explicitly choose whether a canonical versioned completion/evidence policy adds value. If it does, write a durable decision and implement in the Rust engine with compatibility/conformance. If it does not, keep the helper optional and close WI079 only after documenting that justified disposition.

## Initial 2026-11-22 no-claim inventory (historical)

- At initial drafting the helper and isolated regressions existed, but **no local execution was yet evidenced at that checkpoint**. Subsequent focused and full Windows local validation is preserved in PR #25, including clean exact-source run `20261124-079-pr25-clean-exact-head-ci`; those later results do not retroactively change the initial record.
- Current limitations remain: real agent review quality, independent-review effectiveness, hosted invocation, and a protected pre-completion gate are **NOT PROVEN**. See the separate [opt-in profile boundary](optional-profile-boundary-2026-11-25.md) for pending local interception tests and explicit non-interception of plain Git commits.
- WI079 work-item criteria remain pending. No release or adopter rollout is approved.
