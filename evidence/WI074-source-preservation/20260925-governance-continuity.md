# WI074 RPS-001 / RPS-002 / RPS-017 verification

**Scope:** source preservation, complete source-path reconciliation, and
governance continuity only. Verification is against integrated source
`8f1ce8deb139287655afcc8479dc69dd621d8720`, Core S2
`6aff2c376efb5ddf236bd11cd1d700f873ec6a4f`, Workbench S3
`5678bdb1d04f8582fcf5ddc5b3fc8cb884199342`, published Core baseline
`e15fbcfa4a29e1e6df7339bfed56c15c19185f9a`, current Core baseline
`7d761f6733b1de5078d08bc64ea076cf10cee58e`, and published Workbench
`a1876b08b78f169a79597ec3099d9840ace55e06`.

The machine-readable reports are the authority for path-by-path hashes and
bundle/ref inventory:

- [Preservation verification](20260925-preservation-verification.json)
- [1,048-path source reconciliation](../WI074-source-reconciliation/20260925-source-reconciliation.json)
- [Canonical WI074 transfer manifest](../WI074-transfer/20260925-transfer-manifest.json)

## RPS-001 — backups, refs, and historical boundaries

All three named external bundles were present and passed `git bundle verify`,
fetch into disposable bare repositories, and
`git fsck --full --no-reflogs --connectivity-only`. Their exact byte sizes,
SHA-256 digests, advertised-ref counts, reachable commit counts and redacted
ref-inventory digests are in the preservation manifest. The integrated-source
bundle by itself advertises only its integration branch. Therefore a new,
additive bundle was created in the private external backup store; no prior
bundle was overwritten. This new bundle passed the same integrity checks and
contains all 65 refs from the clean integrated-source checkout with identical
object IDs (plus four worktree HEAD refs and its bundle HEAD). It includes the
WI072 `main` SHA, integration and planning branch refs, both previously
divergent branch refs, and all 18 version/paper tags.

The WI022 checkpoint ref is `33809126e888da8eb96b8a40979a35336e7e5994` and
has four commits unique against WI072 `92251565459dd28e3f74d44782ec14ad41758508`.
The WI032 decision ref is `8966ba060b5fe4a7dedd5aa65879db9dec6be2b0` and has
one unique commit against that same WI072 head. Those commits and refs remain
in the private Git history backup; they were not copied into this publication
or changed.

The original dirty WI073 checkout remained separate and unchanged at its
recorded `main` head `92251565459dd28e3f74d44782ec14ad41758508`; two tracked
paths and three untracked files remained present. Git bundles preserve commits
and refs, not uncommitted working-tree data. No WI073 contents were copied into
these reports. The restricted former personal-repository remote was not contacted.

Git bundles do not contain GitHub issues/comments, pull requests/reviews,
discussions, release records/assets, branch-protection/ruleset settings, or
Actions settings, workflow runs and hosted artifacts. Current read-only API
checks for Core and Workbench reported Actions disabled and zero workflow runs
for both. This is not a claim that Git bundles preserve that hosted metadata.
The former personal repository is represented only by the historical S4.1
snapshot; its present hosted state was not checked in this task.

## RPS-002 — source inventory reconciliation

The immutable S2 manifest has 1,048 unique source paths: 853 Core, 194
Workbench and one historical archive entry. Every original source SHA-256
matches the raw Git blob at the integrated-source commit. The independent
reconciliation includes source and destination path/hash values, ownership,
crate manifests, command targets, publication-baseline hashes, later Core
transfer paths and all publication-specific Workbench files.

All 853 Core-owned extraction destinations exist. The five content changes
are confined to the expected standalone-boundary files: `README.md` removes
integrated Workbench claims; `docs/assets/README.md` identifies the screenshot
as archived; `pyproject.toml` identifies the Core project and canonical
ForgeWireLabs URL while retaining package/version identity; `rust/Cargo.toml`
removes Workbench workspace members and dependencies; and `rust/Cargo.lock`
prunes the resulting unused dependency graph. The remaining Core destinations
match their source bytes.

All 194 Workbench source rows match the S3 manifest and destination Git blobs.
The four documented changes are `docs/guides/github-app-setup.md`,
`scripts/generate-types.mjs`, `src/lib/mobile-types.ts` and
`src/lib/remote-types.ts`; 190 source entries are byte-identical. The S3
manifest's change notes and source/destination SHA-256 values are retained per
row.

The published Core baseline provenance list (850 files) matches the raw
published tree exactly. Three original Core files were withheld: the two
private pre-launch documents and the operator-specific WI050 signer harness.
The historical Workbench screenshot is separately excluded from Core
publication. The generated S2 manifest itself is withheld because it contains
machine-specific paths. These are the five explicit publication exclusions;
none is an unexplained source omission. The published Workbench provenance
list (206 files) also matches its Git tree exactly: all 194 original Workbench
paths plus 12 standalone publication files. Its local-only S3 manifest and
manifest-generation helper remain withheld.

Across the two published path lists, nine root/support paths share a pathname;
only `LICENSE` has identical bytes. The other same-name files are independent
repository scaffolding/governance material, not duplicate ownership of an S2
source entry. The full mapping confirms unique S2 ownership and records the
Workbench's independent ledger boundary.

Four Core paths changed after the initial Core publication baseline and are
reconciled explicitly: the dashboard was regenerated after the WI057 repair
evidence link; the malformed WI057 evidence received the authorized narrow
serialization repair; and the two original proposed-WI074 files were moved
into the active canonical Core ledger. The transfer manifest and current
active-ledger hashes provide the follow-on provenance. No other published
Core baseline hash discrepancy remains.

## RPS-017 — governance continuity

Decisions 0067–0074 retain their source provenance and original identifiers;
Decision 0075 establishes ForgeWireLabs Core as the sole ongoing WI074
authority. Source and destination hashes were checked against the transfer
manifest. The current WI074 record is active, retains all 24 criteria, and
preserves the original preflight (`2026-09-24T19:12:30Z`), architecture
approval (`2026-09-24T21:31:54Z`), S2-before-approval chronology, and Decision
0074's actual deviation disposition (`2026-09-25T02:39:02Z`). No historical
timestamp or evidence record was backdated or rewritten. The WI057 repair run
is linked as additional evidence without changing historical WI057 result
fields.

The source map's historical governance/evidence/audit entries are reconciled
in place rather than re-imported as a duplicate monorepo ledger. The Apache
`LICENSE` blob is byte-identical from integrated source through Core and
Workbench publication; the license decisions remain in Core. The source
ownership map's schemas, policies, owners, invariants and frozen-surface
record are retained and hash-mapped. The Workbench repository has no WI074
work-item, evidence-run or governance authority; its published `work/README.md`
directs WI074 ownership to Core. Private pre-launch content, the signer
harness, private bundles and raw scanner output remain outside the public
repository.

No release, Android-feature, platform-parity or consumer-migration claim is
made by this slice. ForgeWire, Proving Ground, Workbench and PyPI were not
modified. Any acceptance-state change is limited to RPS-001, RPS-002 and
RPS-017 and is governed by the linked immutable run evidence.
