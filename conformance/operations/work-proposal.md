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
| Timeout or invalid/lost operation response | Outcome is unknown; status lookup finds a committed receipt or reports absent; same-key retry cannot duplicate the item. |
| Key reused with a different payload | Deterministic conflict; no second item. |
| Parallel proposals | Unique IDs and independently resolvable receipts. |
| Canonical full validation | Retains the repository-wide authoritative rule set and diagnostics. |

Run the focused Rust suite with `cargo test --manifest-path rust/Cargo.toml
-p repopact-mutation` and Python protocol/client regressions with
`python -m unittest tests.test_engine_client -v`. Run the full conformance
instructions in [`../README.md`](../README.md) before claiming compatibility.
