#!/usr/bin/env python3
"""Read-only retrospective reconstruction of WI083 AC-1's sole mismatch.

The preregistered result remains failed. This is a distinct post-result
diagnostic, not a replacement for the original benchmark or normalization.
No fixtures are executed and no evidence files are modified.
"""
import datetime as dt
import hashlib
import json
import subprocess
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCE = "d6b65737027e189690511a2c0726cf00b5b18686"
RAW_PATH = "evidence/WI083-AC1-20261126/raw-results.json"
RAW_SHA = "da59922853927c5f8cc12ffe1b68324e19c127cb225049acc4f6d16eddb17d74"
ITEM = "work/completed/001-wire-up-the-health-check/work-item.json"
EXPECTED = "bfb91cf012a80f853ac6bcf3c63a1f01ee452b39efc4ec90c53f82fb03cfd688"


def check(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def serialize(doc):
    return (json.dumps(doc, ensure_ascii=False, indent=2) + "\n").replace("\n", "\r\n").encode("utf-8")


def reconstruct(run, stamp):
    # Derived from create_manifest and CreateWorkItem::new in the pinned
    # Rust mutation source, followed by fill_item, link_evidence and
    # complete_item in the preregistered harness. Dict order and CRLF
    # serialization are verified against the original full-file hashes.
    return {
        "$schema": "../../../schemas/work-item.schema.json",
        "id": "001",
        "title": "Wire up the health check",
        "status": "completed",
        "owner_scope": "governance",
        "affected_scopes": [],
        "depends_on": [],
        "provenance": "concrete",
        "preflight": {
            "created_before_work_started": True,
            "created_at": stamp,
            "note": "The caller recorded the operator's preflight confirmation through the RepoPact work-item registration flow.",
        },
        "acceptance_criteria": [{
            "id": "AC-1",
            "text": "The default governance profile passes on the health-check candidate and is linked as immutable evidence.",
            "state": "satisfied",
            "evidence": [run["evidence_id"]],
        }],
        "created": "2026-11-26",
        "updated": "2026-11-26",
        "documentation_impact": {
            "state": "none",
            "rationale": "This local fixture adds no durable documentation surface; it demonstrates a health-check scaffold and evidence workflow only.",
        },
    }


def main():
    blob = subprocess.check_output(
        ["git", "-C", str(ROOT), "cat-file", "blob", f"{SOURCE}:{RAW_PATH}"]
    )
    check(sha(blob) == RAW_SHA, "original measurement blob SHA-256 mismatch")
    raw = json.loads(blob)
    check(raw["ac1_supported"] is False, "original result must remain failed")
    check(len(raw["runs"]) == 24, "unexpected run count")
    check(len(raw["pairwise_state_equivalence"]) == 12, "unexpected pair count")
    check(all(not p["state_equivalence"]["equivalent"] and
              p["state_equivalence"]["mismatched_paths"] == [ITEM]
              for p in raw["pairwise_state_equivalence"]), "original comparison differs")
    groups = defaultdict(list)
    output = []
    count = 0
    for run in raw["runs"]:
        check(run["success"] and all(c["returncode"] == 0 for c in run["commands"]),
              "unsuccessful original run")
        count += len(run["commands"])
        command = next(c for c in run["commands"] if c["label"] == "create-work-item")
        start = dt.datetime.fromisoformat(command["started_at_utc"]).timestamp()
        end = start + command["elapsed_ms"] / 1000
        records = {f["path"]: f for f in run["final_state"]["files"]}
        original = records[ITEM]
        matches = []
        for second in range(int(start), int(end) + 1):
            stamp = dt.datetime.fromtimestamp(second, dt.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
            candidate = reconstruct(run, stamp)
            content = serialize(candidate)
            if sha(content) != original["sha256_raw"]:
                continue
            normalized = content.replace(run["evidence_id"].encode(), b"<generated-evidence-id>")
            check(sha(normalized) == original["sha256_normalized"], "original normalization mismatch")
            candidate["preflight"]["created_at"] = "<generated-preflight-time>"
            candidate["acceptance_criteria"][0]["evidence"] = ["<generated-evidence-id>"]
            secondary = sha(serialize(candidate))
            check(secondary == EXPECTED, "unexpected secondary normalized hash")
            matches.append((stamp, secondary))
        check(len(matches) == 1, f"{run['run_id']}: not exactly one hash match")
        stamp, secondary = matches[0]
        files = {p: f["sha256_normalized"] for p, f in records.items()}
        files[ITEM] = secondary
        groups[run["run_id"].rsplit("-", 1)[0]].append(
            (run["side"], files, run["final_state"]["directories"])
        )
        output.append({"run": run["run_id"], "preflight_created_at": stamp,
                       "original_raw_hash_verified": True,
                       "original_normalized_hash_verified": True,
                       "secondary_normalized_sha256": secondary})
    check(count == 240 and len(groups) == 12, "unexpected command or pair count")
    for group, pair in groups.items():
        check(len(pair) == 2 and {x[0] for x in pair} == {"corrected_baseline", "current"},
              f"{group}: invalid pair")
        check(pair[0][1:] == pair[1][1:], f"{group}: residual state mismatch")
    print(json.dumps({
        "analysis": "retrospective hash reconstruction, not a preregistered pass",
        "source_commit": SOURCE, "raw_sha256": RAW_SHA,
        "raw_matches": len(output), "original_normalized_matches": len(output),
        "retrospective_equivalent_pairs": len(groups), "command_count": count,
        "secondary_normalized_sha256": EXPECTED, "records": output,
    }, indent=2))


if __name__ == "__main__":
    main()
