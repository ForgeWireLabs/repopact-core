# Preserved raw-log whitespace diagnostic

The unfiltered `git diff --cached --check` reported trailing whitespace in the preserved raw `evidence/runs/20261125-079-pr29-preflight-attempts/editable-dev-install.log` at lines 129, 132, 236, 252, 254, 258, and 266. Those bytes are part of the original failed Cargo/pip build output and were not normalized. The final staged check excludes only that immutable raw log and checks all other staged paths.
