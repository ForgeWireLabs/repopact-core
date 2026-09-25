# Publication gate blocker: inherited Core validation failure

During the WI074 transfer, the project-local RepoPact 3.1.3.dev1 CLI reported
19 canonical validation errors in the transfer worktree. The exact same
diagnostics were reproduced in a detached clean worktree at the published
Core baseline `e15fbcfa4a29e1e6df7339bfed56c15c19185f9a`; they are pre-existing
and not caused by the WI074 transfer.

The root diagnostic is:

```text
ERROR [evidence.json-invalid] evidence/runs/20260910-057-process-amplification-remediation.json: expected `,` or `}` at line 15 column 90
```

The other 18 diagnostics are dependent `work.unknown-evidence` errors in the
completed WI057 item, whose criteria reference that evidence ID. The malformed
run is byte-identical to the published baseline blob. Its line 15 contains a
machine-specific executable path with invalid JSON escaping. Repairing it
would change unrelated completed evidence and is outside this transfer's
authorized WI074 scope; no repair was attempted.

The focused `tests/test_validate_repo.py` run also fails against this same
baseline condition (156 tests: 13 failures, 62 errors, 2 skips). Representative
tracebacks show fixture setup fails while loading the malformed inherited JSON,
which cascades into repository-validation assertions. The separate takeover
reference suite passed 7/7. Frozen-surface check passed.

Because canonical validation is a required publication gate, this transfer
must not be pushed until an operator/work coordinator authorizes a separately
scoped repair of the WI057 evidence record and the completed item references,
or otherwise resolves the baseline defect under the repository's governance
rules. Do not weaken validation or edit completed history merely to make this
transfer publishable.
