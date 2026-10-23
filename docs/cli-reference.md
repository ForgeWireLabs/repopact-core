# RepoPact CLI reference

*Diataxis mode: reference (searchable command summary).*

The stable Core package installs the `repopact` command and a matching native
`repopact-engine`. The canonical engine owns migrated semantic validation and
typed operations. Python provides the CLI dispatcher, compatibility surfaces,
and retained workflows; it does not silently replace a missing or incompatible
engine. See [Install and build the engine](guides/packaging-and-engine.md).

## Find exact command syntax

The command surface evolves with releases. To see exact options for the package
you installed, run:

```console
repopact --version
repopact --help
repopact COMMAND --help
```

The table below is the stable 3.1.3 top-level surface, checked against an
isolated Python 3.12.4 environment installed from the published `repopact`
package: `repopact --version` reported `3.1.3`, and `repopact --help` listed
the commands shown here. Grouped commands may have their own subcommands; use
their group help and the linked guide for the installed version rather than
copying development-only commands from `main`.

| Command | Purpose | Main options or next step |
| --- | --- | --- |
| `init` | Create a governed repository | `--target PATH`; optional `--admission --key-file PATH` |
| `adopt` | Plan or apply adoption to an existing repository | `--target PATH`, `--dry-run`; optional admission setup |
| `import-plan` | Import existing plan items into `work/` | `--root PATH`, `--dry-run`, optionally `--issues` |
| `doctor` | Diagnose drift and optionally apply safe repairs | `--root PATH`, `--fix` |
| `takeover` | Retire fully imported legacy planning sources | `--root PATH`, `--dry-run`; `--delete` is Git-guarded and can fall back to archive |
| `validate` | Run canonical repository validation | `--root PATH` |
| `dashboard` | Regenerate the derived dashboard | `--root PATH` |
| `spec` | Regenerate derived `SPEC.md` blocks | `--root PATH` |
| `new` | Stamp a work item, decision, or policy | `KIND TITLE`, `--root PATH`; work-item `--status` |
| `check-frozen` | Report frozen-surface changes in a diff range | `--root PATH`, `--base REF`, and `--ack` only after explicit human approval |
| `fleet-verify` | Verify declared public adopters | `--root PATH`, `--manifest PATH`, repeated `--discover-root PATH`, `--json` |
| `release-closeout` | Check release publication and adopter rollout evidence | `--root PATH`, `--manifest PATH`, `--discover-root PATH`, `--package-evidence PATH`, `--json` |
| `release-build` | Build and structurally verify local release artifacts | Required `--outdir PATH`; optional `--root PATH`, `--revision REF` |
| `verify` | Run a repository-defined local profile | `PROFILE` from `governance/verification.json` (new repos default to `governance`); see [verification and release](guides/local-ci-cd.md) |
| `release` | Verify, build, inspect, publish, and aggregate readiness | See [verification and release](guides/local-ci-cd.md); publishing is a separate explicit operation |
| `graph` | Inspect/build/verify the optional Repository Orientation Graph | See [the graph guide](repository-orientation-graph.md) |
| `admission` | Set up and inspect opt-in pre-execution admission | See [pre-execution admission](guides/pre-execution-admission.md) |
| `guard` | Install or inspect a protected host guard | Requires the platform-specific operator prerequisites and approval described in the admission guide |
| `work` | Create or amend a proposed work record | `propose`, `amend-proposal`; proposals do not grant implementation authority |
| `assurance` | Print a read-only assurance mapping snapshot | `snapshot`; see the assurance and governance records |
| `approval` | Create, inspect, approve, or deny operator requests | Interactive signing is an explicit operator action; see the admission guide |

## Common options

- `--root PATH` points repository-scoped commands at a repository other than
  the current directory.
- `--target PATH` is required by `init` and `adopt` and names the repository
  being created or adopted.
- `--dry-run` previews supported adoption/import/takeover operations where
  offered; it does not create evidence that the plan was applied.
- `--json` requests machine-readable output only where the command exposes
  that option.
- `--version` prints the installed package version. Compare it with the
  engine/version handshake if an engine mismatch is reported.

## First-use sequence

For a new repository:

```console
python -m pip install repopact
repopact init --target ./my-project
cd ./my-project
repopact validate
```

For an existing repository, preview adoption first:

```console
repopact adopt --target ./existing-project --dry-run
```

Review the proposed paths before running the same command without `--dry-run`.
Continue with the [adoption tutorial](adopt-repopact.md), then use
[author-records](guides/author-records.md) and
[local verification](guides/local-ci-cd.md).

## Safety notes

- `validate`, `--help`, and ordinary `verify` runs do not grant approval or
  complete work by themselves.
- `dashboard` and generated `SPEC.md` sections are projections; regenerate them
  from authoritative records rather than editing derived sections manually.
- `check-frozen --ack` is not a substitute for human approval. Use it only
  after approval is recorded as required by the repository contract.
- `release publish`, admission signing, guard installation, and destructive
  takeover operations cross explicit operator boundaries. Read the linked
  workflow before invoking them.
