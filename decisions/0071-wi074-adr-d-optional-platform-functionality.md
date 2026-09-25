---
id: 0071
title: WI074 ADR-D optional and platform-specific functionality
status: accepted
date: 2026-09-24
supersedes: []
---

# 0071: WI074 ADR-D — optional and platform-specific functionality

## Context

The integrated source contains headless security capabilities alongside
desktop, mobile, and remote-provider integrations. A file move must not imply
that security or platform behavior is preserved or proven.

## Decision

Retain Core-owned CLI, engine, sandbox, and confinement behavior in Core.
Workbench owns the Tauri desktop shell/API and the mobile acquisition, SAF,
credential, remote-provider, and GitHub-provider integrations identified by
the WI074 source manifest. Remote GitHub functionality remains optional.
Desktop and Android credentials remain in native credential facilities; no
credential or token may enter frontend bundles, logs, or build artifacts.
Preserve the distinction between `pre-action` and
`sandbox/process-enforced`; moving code alone never upgrades assurance.

S3 reports only actual platform evidence. Windows Tauri results do not prove
Linux, macOS, Android, or iOS behavior. Android validation is attempted only
when the local SDK and device/emulator are available; otherwise record the
missing prerequisite. Linux/macOS/native-mobile gaps remain explicit.

## Alternatives considered

- Removing optional or platform-specific components without consumer proof:
  rejected because it risks feature loss.
- Making GitHub authentication mandatory: rejected because offline/local use is
  a supported Workbench behavior.
- Treating source relocation or a desktop build as security/mobile proof:
  rejected because neither demonstrates the required OS boundary or platform
  integration.

## Consequences

S3 inventories actual consumers and tests each supported path available on the
host. Unavailable platforms remain unverified rather than silently downgraded
or inferred from another target.
