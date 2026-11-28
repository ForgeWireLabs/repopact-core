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
before-state and every ordered projected operation state with live files. It
reverses only complete operations whose resulting content still matches the
journal. If a path contains an external edit, an incomplete write, a link, or
another unrecognized state, RepoPact leaves it and the journal untouched and
reports `recovery_required`. Repeated recovery is idempotent.

Proposal operations and the overlapping dashboard writer share the lock and
are blocked while recovery is pending. Graph output and evidence generation
write disjoint paths. Other processes that edit the affected files do not join
the lock, so pause those writers while recovery is in progress.

The private root-local journal has bounded preimages, journal size, and 1,024
terminal receipts. It does not evict history to make space. This is process-
interruption recovery for a trusted local user; it does not promise recovery
from power loss or protection from another process running as that same user.

See [Decision 0080](../../decisions/0080-ordinary-process-interruption-recovery-for-typed-mutations.md)
for the complete boundary and non-goals.
