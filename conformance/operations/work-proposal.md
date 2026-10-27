# `work.propose` operation contract

This focused operation suite supplements the repository-wide fixture corpus.
It is implemented by `repopact-mutation` and the Rust engine, and is not a
claim that the full `repopact validate` contract has changed.

| Case | Expected result |
| --- | --- |
| Stale dashboard or unrelated expired review | Proposal is applicable; diagnostic is a warning; successful commit regenerates the dashboard. |
| Unrelated historical completion finding | Warning; does not veto a proposed candidate. |
| Candidate owner scope not declared | Blocking authorization diagnostic; no commit. |
| Unknown dependency/evidence, broken historical evidence reference, broken assurance reference, sensitive-evidence violation, or dependency cycle | Blocking integrity diagnostic; no commit. |
| Engine product/protocol identity mismatch or runtime failure | Fail closed before mutation; no accepted-work claim. |
| Failure while writing the staging directory | No work item or receipt is visible; retry with the same key can commit. |
| Concurrent proposal and non-proposal RepoPact mutation | Shared repository lock serializes commits; the later operation rechecks its read set and rejects a stale ID or authorization assumption. |
| Timeout or invalid/lost operation response | Outcome is unknown. A valid receipt resolves the committed item across lifecycle moves. `absent` means no visible receipt; it cannot prove that a previously committed receipt was not removed. Inspect the work ledger before retrying. |
| Matching receipt is malformed, duplicated, or disagrees with its item | Status and same-key replay fail closed. |
| Historical receipt is malformed JSON or has a missing/invalid key | Lookup fails closed because key uniqueness cannot be established. A valid receipt with a different key is ignored. |
| Receipt is missing while its historical item remains | Status reports absent; inspect and reconcile the item, and do not retry blindly. |
| Key reused with a different payload | Deterministic conflict; no second item. |
| Item moves through a lifecycle transition | Receipt remains discoverable and status returns the current item path; same-key replay returns the same item. |
| Parallel proposals | Unique IDs and independently resolvable receipts. |
| Canonical full validation | Retains the repository-wide authoritative rule set and diagnostics. |

The transaction stages the item, README, and receipt on the same volume, calls
`sync_all()` on each staged file, and publishes them together with a directory
rename. This is an atomic-visibility boundary while the system is running. The
containing directories are not synced after rename and Windows write-through is
not requested, so the implementation does not promise power-loss persistence;
no power-loss test is claimed.

Run the focused Rust suite with `cargo test --manifest-path rust/Cargo.toml
-p repopact-mutation` and Python protocol/client regressions with
`python -m unittest tests.test_engine_client -v`. Run the full conformance
instructions in [`../README.md`](../README.md) before claiming compatibility.
