# Recover from RepoPact failures

*Diataxis mode: how-to (recover safely from a governance-tool failure).*

This guide separates a legitimate governance rejection from defects in the
repository or runtime. Read the diagnostic and the named source record first.
Proposed work is not implementation approval, and an unhealthy engine does not
authorize bypassing protected mutations.

## Classify the failure

1. **Governance rejection.** `work plan` reports a blocking scope, owner,
   reference, identity, or dependency diagnostic. Correct the authoritative
   record or request an operator decision; do not hide the condition in a
   dashboard or retry with a different key.
2. **Invalid authoritative state.** `repopact validate` reports malformed
   invariants, ownership, work records, or broken references. Preserve the
   failing files and repair the source records under their declared owner.
   A successful proposal does not certify that the whole repository validates.
3. **Infrastructure/runtime failure.** The engine is unavailable, incompatible,
   or exits without a valid protocol response. Check that Python and
   `repopact-engine` come from the same package build. Do not use a Python
   validator as a mutation fallback.
4. **Timeout or unknown outcome.** If a proposal request times out or returns
   an invalid/lost operation response, keep the printed idempotency key and run:

   ```console
   repopact work proposal-status <key> --root .
   ```

   If the state is `committed`, use the returned work-item path. If it is
   `absent`, retry the exact request with the same key. A different payload
   under a committed key is rejected. Status lookup does not grant work
   authority.
5. **Derived-state inconsistency.** A stale or missing dashboard is a
   maintenance diagnostic for proposal admission. After the source change,
   run `repopact dashboard` and `repopact validate`; never edit generated
   dashboard content to simulate a pass.

## Inspect before proposing

Use the read-only operation to see candidate blockers and phase timings without
creating a work record:

```console
repopact work plan "Describe the proposed work" --root . --confirm-preflight
```

Then submit with the required preflight confirmation. Keep its printed key if
the response is interrupted:

```console
repopact work propose "Describe the proposed work" --root . --confirm-preflight
```

An unrelated maintenance warning may be recorded while the proposal proceeds.
Authorization violations, broken authoritative references, dependency cycles,
and sensitive-evidence errors remain blocking. Full validation remains the
canonical repository-wide assurance operation.

## Continue unaffected work safely

Engine failure does not prevent ordinary source changes that the repository
owners have already authorized, but it does prevent claiming proposal
admission, activation, completion, evidence acceptance, or release approval.
Record the incident outside RepoPact's registry only when the registration
path itself is defective; label it explicitly as an external incident rather
than fabricating a work-item ID or status.
