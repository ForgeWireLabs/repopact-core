# RepoPact Core

**Keep the engineering context that matters in the repository, so people and coding agents can pick up work responsibly.**

AI-assisted development can move quickly, while project intent, decision history, ownership, constraints, evidence, and unfinished work remain scattered across chats and tools. RepoPact Core gives teams a shared, version-controlled way to record and validate that context alongside their code.

> **The repository is the pact.**
>
> The project records—not a particular chat or model—are the durable reference for what the work is, who may change it, and how completion is shown.

RepoPact is open source under the [Apache-2.0 license](LICENSE). The stable command-line package is available on [PyPI](https://pypi.org/project/repopact/).

`pip install repopact` · current release **3.1.3**.

## Get started

Install the stable CLI:

```bash
python -m pip install repopact
```

Create governance records in a new repository:

```bash
repopact init --target ../your-repo
cd ../your-repo

repopact new work-item "Add retry policy"
repopact validate
repopact dashboard
```

For an existing repository, first preview the adoption plan:

```bash
repopact adopt --target ../existing-repo --dry-run
```

Review the plan, then run `repopact adopt --target ../existing-repo` to create the initial records. The [adoption guide](docs/adopt-repopact.md) walks through the process.

## What it records

RepoPact keeps project state that should survive a handoff in typed, version-controlled records:

- **Intent and decisions** explain what the team is doing and which choices are settled.
- **Roles and scopes** make ownership and change boundaries visible.
- **Work items and acceptance criteria** show what is planned, active, blocked, or complete.
- **Invariants and protected paths** record guarantees that need explicit review to change.
- **Evidence and provenance** connect completion claims to validation and show when reconstructed information is still uncertain.
- **Generated dashboards and repository maps** make the records easier to navigate while leaving the records themselves authoritative.

The validator checks record structure and relationships, including lifecycle, dependencies, references, and completion evidence. Completion can require linked evidence rather than relying on a status label alone.

## Works with your tools

RepoPact does not require a particular coding agent, model provider, editor, CI service, or cloud account. Keep using the tools you already have; the repository records give human contributors and agents a common source of project context.

The optional [RepoPact Workbench](https://github.com/ForgeWireLabs/repopact-workbench) provides a desktop interface for inspecting and working with governed repositories. The CLI and Workbench are separate products.

## Documentation

- [Adopt RepoPact](docs/adopt-repopact.md) — initialize a repository or bring an existing project under governance.
- [Documentation map](docs/README.md) — guides for using, understanding, integrating, and developing RepoPact.
- [Concepts](docs/concepts.md) — authority, lifecycle, provenance, evidence, and assurance boundaries.
- [Specification](SPEC.md) and [conformance contract](CONFORMANCE.md) — normative behavior and compatibility expectations.
- [Contributing](CONTRIBUTING.md) — development setup and contribution guidance.

RepoPact is a governance and validation layer. It is not an AI agent, issue tracker, hosted CI service, or operating-system sandbox, and it does not replace Git, test frameworks, or external authorization systems.

## License

RepoPact Core is licensed under Apache-2.0. See [LICENSE](LICENSE).
