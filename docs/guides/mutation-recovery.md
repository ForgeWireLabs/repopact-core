# Guide: Recover an interrupted typed mutation

*Diataxis mode: how-to (task-oriented).*

RepoPact protects its typed ordinary work-item mutations with one repository-
local transaction journal. This covers work-item creation through `repopact`
and the Rust create, edit, and transition mutation API. It does not cover
arbitrary file edits or writers outside the shared engine lock.

## Keep the request key

`repopact new work-item` prints a mutation key before sending the request. Save
it if the process exits or the response is lost. A caller using the Rust API
supplies its own key; `repopact new work-item` accepts that key on retry:

```text
repopact new work-item "Register the work" --mutation-key <printed-key>
```

The same key with the same typed intent returns the recorded terminal result.
The same key with a different intent is rejected. Do not silently retry after
transport loss.

## Inspect status without changing files

```text
repopact work mutation-status
repopact work mutation-status --request-key <printed-key>
```

Status does not create or clean transaction state and never performs rollback.
`pending` means an explicit recovery may be needed. `terminal` means the receipt
is durable while journal cleanup remains; the next keyed recovery or replay can
finish that cleanup. `unknown` means no matching receipt was found and is not
proof that the request never ran. `recovery_required` means a journal, receipt,
path, or storage check was ambiguous or invalid and must be preserved for
inspection.

## Recover a pending request explicitly

```text
repopact work mutation-recover --request-key <printed-key>
```

Recovery runs under the working-root mutation lock. It compares the captured
before-state with the journal's durable phase and exact operation cursor.
`prepared` at cursor zero authorizes only the before-state. `applying` at
cursor *k* authorizes only the states immediately before or after operation
*k*. `rolling_back` records the exact forward operation index being reversed;
each retry checks its adjacent before/after states before acting. If a path
contains an external edit, an incomplete write, a link, a future operation, or
another unreachable state, RepoPact leaves it and the journal untouched and
reports `recovery_required`. If a prepared cursor-zero journal still matches
its before-state, recovery records `not_applied` without rewriting repository
paths. Repeated recovery is idempotent, including after the process stops
during a rollback checkpoint.

Proposal operations and the overlapping dashboard writer share the lock and
are blocked while recovery is pending. Graph output and evidence generation
write disjoint paths. Other processes that edit the affected files do not join
the lock, so pause those writers while recovery is in progress.

The private root-local journal allows at most three typed operations, 10,000
preimage entries, 64 MiB of captured file bytes, a 16 MiB aggregate projected
preimage budget, and a 256 MiB serialized journal. It also stores up to 1,024
terminal receipts. It does not evict history to make space. This is process-
interruption recovery for a trusted local user; it does not promise recovery
from power loss or protection from another process running as that same user.

## If receipt capacity is exhausted

The next mutation is rejected before it writes governed files. RepoPact keeps
all receipts because removing one could make a delayed retry apply twice.
Resolve capacity as a controlled working-copy rotation:

1. Stop all RepoPact mutation clients for the working copy. Use
   `repopact work mutation-status` and explicitly recover any pending request.
   Resolve every request key whose outcome is still unknown before continuing.
2. Make and verify a complete backup of the repository and transaction state.
   Keep the old working copy available as a read-only status archive.
3. Make a second working copy from the complete current tree and version
   control metadata. Preserve `work/.repopact-transactions` proposal state, but
   do not copy its `ordinary-v1` directory into the new working copy. Keep the
   archived `ordinary-v1` receipts with the old copy.
4. Confirm the new root is clean, then resume mutations there with newly minted
   request keys. Keep a mapping from each old key to its archived receipt and
   never reuse an old key in the new working copy.

The native root identity changes during this rotation, so old ordinary
receipts intentionally remain queryable only from the archived old working
copy. Do not remove receipts from an active store to make room.

See [Decision 0080](../../decisions/0080-ordinary-process-interruption-recovery-for-typed-mutations.md)
for the complete boundary and non-goals.
