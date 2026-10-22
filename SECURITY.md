# Security Policy

RepoPact Core is the repository-native governance and verification package. The
stable version is recorded in [`VERSION`](VERSION). Core includes a canonical
Rust semantic engine and Python command, compatibility, and packaging
components. See [Install and build the engine](docs/guides/packaging-and-engine.md)
for the engine boundary. RepoPact Workbench is a separate application with its
own build and integration surfaces; this policy covers Core and this repository.

## Reporting a vulnerability

If you believe you have found a vulnerability, report it privately through
GitHub's **Report a vulnerability** feature when available for this repository.
If private reporting is unavailable, contact the maintainers through the
ForgeWire Labs organization profile. Do not include credentials, tokens, or
private repository data in a public issue. This project does not promise a
response or remediation time.

Include the affected release or commit, the relevant input and command, the
security property you expected, and reproducible impact when it is safe to
share. Do not publish exploit details before coordinating a fix.

## Scope and trust boundaries

Reports may concern the Rust semantic engine, Python command and compatibility
surfaces, record and archive processing, package/build configuration, schemas,
or repository-provided scripts and workflows. RepoPact processes files supplied
by repositories; treat records, archives, configuration, and command
definitions as untrusted unless the operator has independently established
trust.

RepoPact validation reports whether governed records satisfy implemented rules.
It does not authenticate a repository, prove its records truthful, or establish
that arbitrary repository commands, integrations, or host environments are
safe. Optional admission, guard, and operating-system enforcement capabilities
have separate assurance classes and platform-specific limits. Do not infer a
host security boundary from ordinary validation or from documentation alone;
scope each enforcement claim to the implementation and evidence for its
capability and platform.

The Rust engine is canonical for migrated semantic validation. Python remains
the command/compatibility surface and an explicit comparator for those rules;
it is not a silent fallback when the engine is missing or incompatible.
Installation and source builds also rely on the Python and Rust ecosystems and
their transitive dependencies.

## Dependencies and release support

The Python package declares `jsonschema` as a runtime dependency. Build and
development dependencies are declared in `pyproject.toml`; Rust dependencies
are recorded in the Cargo manifests and lockfile. Optional components may add
their own dependencies. Assess the exact release artifact and dependency set
when investigating a report.

The current stable release is **3.1.3**. The project has not published an LTS
schedule or a continuing security-maintenance promise for older releases. Use
the current stable release when practical; development code on `main` can differ
from released artifacts. Security fixes are assessed against the current stable
release, with any broader backport or support commitment stated in the relevant
release notice.

RepoPact is not a general-purpose sandbox, operating-system access-control
system, secrets manager, or remote authorization service. Governance records
and validation support explicit workflows but do not by themselves guarantee a
repository or downstream tool is secure.
