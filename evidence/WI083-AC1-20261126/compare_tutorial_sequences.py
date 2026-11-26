#!/usr/bin/env python3
"""Preregistered WI083 AC-1 corrected-baseline tutorial comparison."""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import importlib.metadata
import json
import math
import os
from pathlib import Path
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().with_name("raw-results.json")
BASE = "733e23a8a091727908dbd3c690fb7169f91b7287"
OLD = "b9dec03ad59c2ef7d782eda9f2b3220747e8ba3c"
ENGINE_SOURCE = "805f86ccf46d2311bbe19121cad22904b5993c9d"
ENGINE_HASH = "63a78d027472020dd6d1aa3a6e2a7d49ed092f529dcf1efa77283a8e1d0ca81b"
SEMANTIC = "repopact-semantic-v1:sha256:a11c42f9a8129a186a44a0e7822d9f2edb1512a7bc13fbe945ede32d8acfb255"
OLD_BLOB, OLD_SHA = "b47c922147d8798d936279d88f0725e04d9a5282", "db6c850b8936f5da3b0f7c37dc4fa3014a2ebf43e325daa83d0ab76a7951a9e8"
NEW_BLOB, NEW_SHA = "871065f026d64e96df80ffb417eb181e92c23987", "9e5c161bb4082ca15236dfa2bba36ca98a75d684985a607579725412f0e745da"
CLI = Path(r"C:\Projects\repopact-core-wi083-optimization-venv-20261126\Scripts\repopact.exe")
ENGINE = Path(r"C:\Projects\repopact-core-wi083-optimization-20261126\rust\target\debug\repopact-engine.exe")
FIXTURES = Path(tempfile.gettempdir()) / "WI083-AC1-corrected-baseline-20261126"
WARMUPS, PAIRS, TIMEOUT = 2, 10, 180
MIN_FREE = 12 * 1024**3
ID_TOKEN, TIME_TOKEN, DURATION_TOKEN = "<generated-evidence-id>", "<generated-time>", "<generated-duration>"
TIME_KEYS = {"timestamp", "started_at", "finished_at", "started_at_utc", "finished_at_utc", "created_at", "updated_at", "completed_at", "recorded_at", "start_time", "end_time"}
DURATION_KEYS = {"duration", "duration_ms", "duration_seconds", "elapsed_ms", "elapsed_seconds", "wall_time_ms", "wall_time_seconds"}


class ExperimentError(RuntimeError):
    pass


def git(*args: str, binary: bool = False) -> str | bytes:
    return subprocess.check_output(["git", "-C", str(ROOT), *args], text=not binary)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def verify_inputs() -> dict[str, Any]:
    source_info = {}
    for ref, blob_expected, sha_expected in ((OLD, OLD_BLOB, OLD_SHA), (BASE, NEW_BLOB, NEW_SHA)):
        spec = f"{ref}:docs/adopt-repopact.md"
        content = git("cat-file", "blob", spec, binary=True)
        blob = str(git("rev-parse", spec)).strip()
        sha = digest(content)  # type: ignore[arg-type]
        if blob != blob_expected or sha != sha_expected:
            raise ExperimentError(f"tutorial identity mismatch at {ref}")
        source_info[ref] = {
            "commit": ref,
            "tree": str(git("rev-parse", f"{ref}^{{tree}}")).strip(),
            "path": "docs/adopt-repopact.md",
            "git_blob": blob,
            "sha256": sha,
            "bytes": len(content),  # type: ignore[arg-type]
        }
    return source_info


def python_cli_info() -> dict[str, Any]:
    site = Path(sys.prefix) / "Lib" / "site-packages"
    pointer = site / "repopact.pth"
    if not pointer.is_file():
        raise ExperimentError(f"missing editable package pointer {pointer}")
    package_root = Path(pointer.read_text(encoding="utf-8").strip()).resolve()
    package_rev = str(subprocess.check_output(["git", "-C", str(package_root), "rev-parse", "HEAD"], text=True)).strip()
    subprocess.check_call(["git", "-C", str(package_root), "diff", "--quiet", BASE, package_rev, "--", "repopact"])
    dirty = subprocess.check_output(["git", "-C", str(package_root), "status", "--porcelain", "--", "repopact"], text=True)
    if dirty:
        raise ExperimentError("editable Python CLI has uncommitted repopact source changes")
    return {
        "python": str(Path(sys.executable).resolve()),
        "venv": str(Path(sys.prefix).resolve()),
        "package_version": importlib.metadata.version("repopact"),
        "package_checkout": str(package_root),
        "package_source_revision": package_rev,
        "package_matches_comparison_candidate": True,
        "jsonschema_version": importlib.metadata.version("jsonschema"),
    }


def preflight(sample: bool) -> dict[str, Any]:
    if not CLI.is_file() or not ENGINE.is_file():
        raise ExperimentError("pinned CLI or engine executable is missing")
    engine_hash = digest(ENGINE.read_bytes())
    if engine_hash != ENGINE_HASH:
        raise ExperimentError(f"engine SHA-256 mismatch: {engine_hash}")
    sources = verify_inputs()
    subprocess.check_call([
        "git", "-C", str(ROOT), "diff", "--quiet", ENGINE_SOURCE, BASE,
        "--", "repopact", "rust", "scripts", "tests", "conformance",
        "pyproject.toml", "Cargo.toml", "Cargo.lock",
    ])
    head = str(git("rev-parse", "HEAD")).strip()
    parent = str(git("rev-parse", "HEAD^")).strip()
    dirty = str(git("status", "--porcelain")).strip()
    if sample and (parent != BASE or dirty):
        raise ExperimentError("sampling requires a clean preregistration commit directly atop integrated main")
    free = shutil.disk_usage(ROOT).free
    if free < MIN_FREE:
        raise ExperimentError(f"storage preflight failed: {free} free bytes; {MIN_FREE} required")
    return {
        "candidate_base": {"commit": BASE, "tree": str(git("rev-parse", f"{BASE}^{{tree}}")).strip()},
        "preregistration_commit": head if sample else None,
        "pre_run_worktree_clean": not bool(dirty),
        "tutorial_sources": sources,
        "python_cli": python_cli_info(),
        "engine": {
            "path": str(ENGINE), "sha256": engine_hash, "source_revision": ENGINE_SOURCE,
            "semantic_identity": SEMANTIC, "profile": "debug",
            "target": "x86_64-pc-windows-msvc", "rustc": "1.96.0",
            "runtime_source_delta_since_build": "none",
        },
        "host": {"platform": platform.platform(), "python_version": sys.version,
                 "cpu": platform.processor(), "machine": platform.machine(), "model_calls": 0},
        "storage_preflight": {"checked_path": str(ROOT), "free_bytes": free,
                              "minimum_free_bytes": MIN_FREE, "passed": True},
        "plan": {"warmups_per_side": WARMUPS, "matched_pairs": PAIRS,
                 "sample_exclusions": [], "timeout_seconds": TIMEOUT,
                 "fixture_parent": str(FIXTURES)},
    }


def record_edit(run: dict[str, Any], repo: Path, label: str, paths: list[Path], changed: bool = True) -> None:
    run["scripted_edits"].append({
        "label": label, "changed": changed,
        "files": [{"path": p.relative_to(repo).as_posix(), "sha256": digest(p.read_bytes())} for p in paths],
    })


def invoke(run: dict[str, Any], cwd: Path, args: list[str], label: str) -> bool:
    argv = [str(CLI), *args]
    start = dt.datetime.now(dt.timezone.utc).isoformat()
    t0 = time.perf_counter_ns()
    try:
        proc = subprocess.run(argv, cwd=cwd, env={**os.environ, "REPOPACT_ENGINE": str(ENGINE)},
                              capture_output=True, text=True, timeout=TIMEOUT, check=False)
        row = {"label": label, "argv": argv, "cwd": str(cwd), "started_at_utc": start,
               "elapsed_ms": round((time.perf_counter_ns()-t0)/1e6, 3),
               "returncode": proc.returncode, "stdout": proc.stdout, "stderr": proc.stderr,
               "failure": None if proc.returncode == 0 else f"command exited {proc.returncode}"}
    except subprocess.TimeoutExpired as exc:
        stdout, stderr = exc.stdout or "", exc.stderr or ""
        if isinstance(stdout, bytes): stdout = stdout.decode("utf-8", "replace")
        if isinstance(stderr, bytes): stderr = stderr.decode("utf-8", "replace")
        row = {"label": label, "argv": argv, "cwd": str(cwd), "started_at_utc": start,
               "elapsed_ms": round((time.perf_counter_ns()-t0)/1e6, 3), "returncode": None,
               "stdout": stdout, "stderr": stderr, "failure": f"timeout after {TIMEOUT}s"}
    run["commands"].append(row)
    if row["failure"]:
        run["failure"] = {"label": label, "reason": row["failure"]}
        return False
    return True


def add_governance(run: dict[str, Any], repo: Path) -> None:
    p = repo / "governance" / "invariants.json"
    doc = json.loads(p.read_text(encoding="utf-8"))
    numbers = [int(m.group(1)) for row in doc["invariants"] if (m := re.fullmatch(r"INV-(\d+)", row["id"]))]
    new_id = f"INV-{max(numbers, default=0)+1}"
    doc["invariants"].append({"id": new_id, "statement": "The health check does not reveal credential values.",
        "rationale": "Health responses may be visible to monitoring systems and must not contain secrets.",
        "escalation": "If the check cannot establish this guarantee, stop and revise the interface before enabling it.",
        "enforced_by": None})
    p.write_text(json.dumps(doc, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    record_edit(run, repo, f"add {new_id} to invariants.json", [p])
    frozen = repo / "governance" / "frozen-surface.json"
    json.loads(frozen.read_text(encoding="utf-8"))
    record_edit(run, repo, "inspect frozen surface; identify README.md stable contract", [frozen], False)


def get_item(repo: Path) -> tuple[Path, dict[str, Any]]:
    for p in (repo / "work" / "active").glob("*/work-item.json"):
        doc = json.loads(p.read_text(encoding="utf-8"))
        if doc.get("title") == "Wire up the health check": return p, doc
    raise ExperimentError("health-check work item missing")


def fill_item(run: dict[str, Any], repo: Path, p: Path, item: dict[str, Any]) -> None:
    item["acceptance_criteria"][0]["text"] = "The default governance profile passes on the health-check candidate and is linked as immutable evidence."
    item["updated"] = dt.date.today().isoformat()
    p.write_text(json.dumps(item, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    readme = p.with_name("README.md")
    readme.write_text(readme.read_text(encoding="utf-8") + "\n\n## Intent\n\nAdd a small local health-check scaffold and preserve a passing RepoPact governance verification as evidence.\n", encoding="utf-8")
    record_edit(run, repo, "fill acceptance criterion and add intent context", [p, readme])


def link_evidence(run: dict[str, Any], repo: Path, p: Path, before: set[Path]) -> str:
    added = sorted(set((repo/"evidence"/"runs").glob("*.json"))-before)
    if len(added) != 1: raise ExperimentError(f"expected one evidence record, found {len(added)}")
    evpath = added[0]
    evidence_id = json.loads(evpath.read_text(encoding="utf-8")).get("id", evpath.stem)
    item = json.loads(p.read_text(encoding="utf-8"))
    item["acceptance_criteria"][0].update(state="satisfied", evidence=[evidence_id])
    item["updated"] = dt.date.today().isoformat()
    p.write_text(json.dumps(item, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    run["evidence_id"] = evidence_id
    run["linked_evidence_file"] = evpath.relative_to(repo).as_posix()
    record_edit(run, repo, "link successful verification evidence to AC-1", [p])
    return evidence_id


def complete_item(run: dict[str, Any], repo: Path, p: Path) -> None:
    src = p.parent
    item = json.loads(p.read_text(encoding="utf-8"))
    item.update(status="completed", updated=dt.date.today().isoformat(), documentation_impact={
        "state": "none", "rationale": "This local fixture adds no durable documentation surface; it demonstrates a health-check scaffold and evidence workflow only."})
    p.write_text(json.dumps(item, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")
    dest = repo/"work"/"completed"/src.name
    if dest.exists(): raise ExperimentError("completed work item already exists")
    dest.parent.mkdir(parents=True, exist_ok=True)
    shutil.move(str(src), str(dest))
    readme = dest/"README.md"
    readme.write_text(re.sub(r"(?m)^> \*\*Status\*\*:.*$", "> **Status**: Completed", readme.read_text(encoding="utf-8")), encoding="utf-8")
    record_edit(run, repo, "complete work item and set documentation impact", [dest/"work-item.json", readme])


def state(repo: Path, evidence_id: str) -> dict[str, Any]:
    files, directories = [], []
    for current, dirs, names in os.walk(repo, followlinks=False):
        here = Path(current)
        directories.extend((here/d).relative_to(repo).as_posix() for d in dirs)
        for name in names:
            p = here/name
            rel = p.relative_to(repo).as_posix()
            raw = p.read_bytes()
            normalized = raw.replace(evidence_id.encode(), ID_TOKEN.encode()) if evidence_id else raw
            path = rel.replace(evidence_id, ID_TOKEN) if evidence_id else rel
            if rel.startswith("evidence/runs/") and rel.endswith("-verify-governance.json"):
                doc = json.loads(raw.decode("utf-8"))
                doc = normalize(doc, evidence_id)
                normalized = (json.dumps(doc, ensure_ascii=False, indent=2)+"\n").encode()
            files.append({"path": path, "sha256_raw": digest(raw), "sha256_normalized": digest(normalized)})
    files.sort(key=lambda row: row["path"]); directories.sort()
    tree = {"directories": directories, "files": files}
    return {"sha256_normalized_tree": digest(json.dumps(tree, sort_keys=True, separators=(",", ":")).encode()), **tree}


def normalize(value: Any, evidence_id: str, path: tuple[str, ...] = ()) -> Any:
    if isinstance(value, dict):
        out = {}
        for key, child in value.items():
            if not path and key == "id": out[key] = ID_TOKEN
            elif key in TIME_KEYS: out[key] = TIME_TOKEN
            elif key in DURATION_KEYS: out[key] = DURATION_TOKEN
            else: out[key] = normalize(child, evidence_id, (*path, key))
        return out
    if isinstance(value, list): return [normalize(child, evidence_id, path) for child in value]
    if isinstance(value, str) and evidence_id: return value.replace(evidence_id, ID_TOKEN)
    return value


def finish(run: dict[str, Any], t0: int, repo: Path | None, fixture: Path) -> dict[str, Any]:
    commands = run["commands"]
    run["harness_elapsed_ms"] = round((time.perf_counter_ns()-t0)/1e6, 3)
    run["command_elapsed_ms_sum"] = round(sum(c["elapsed_ms"] for c in commands), 3)
    run["repopact_cli_command_count"] = len(commands)
    run["direct_validate_count"] = sum(c["label"].startswith("validate-") for c in commands)
    run["verify_count"] = sum(c["label"].startswith("verify-") for c in commands)
    run["verification_command_count"] = run["direct_validate_count"]+run["verify_count"]
    run["init_count"] = sum(c["label"] == "bootstrap" for c in commands)
    run["new_count"] = sum(c["label"] == "create-work-item" for c in commands)
    run["dashboard_count"] = sum(c["label"].startswith("dashboard-") or c["label"] == "regenerate-dashboard" for c in commands)
    run["success"] = run.get("failure") is None and len(commands) in (7, 13)
    if run["success"] and repo is not None:
        run["final_state"] = state(repo, run.get("evidence_id", ""))
        run["normalizations"] = {"generated evidence ID: filename/root id/evidence reference/exact string occurrences": ID_TOKEN,
            "timestamps in generated verification manifest": sorted(TIME_KEYS), "duration fields in that manifest": sorted(DURATION_KEYS)}
    else: run["final_state"] = None
    if fixture.resolve().parent != FIXTURES.resolve(): raise ExperimentError(f"refusing unsafe cleanup: {fixture}")
    if fixture.exists(): shutil.rmtree(fixture)
    return run


def execute(run_id: str, side: str, order: int, fixture: Path) -> dict[str, Any]:
    corrected = side == "corrected_baseline"
    fixture.mkdir(parents=True, exist_ok=False)
    repo = fixture/"my-project"
    run = {"run_id": run_id, "side": side, "execution_order_in_pair": order,
           "source_ref": OLD if corrected else BASE, "fixture_container": str(fixture),
           "commands": [], "scripted_edits": [], "failure": None}
    t0 = time.perf_counter_ns()

    def cmd(args: list[str], label: str) -> bool:
        return invoke(run, repo if repo.exists() else fixture, args, label)

    try:
        if not cmd(["init", "--target", "./my-project"], "bootstrap"): return finish(run,t0,None,fixture)
        if corrected and not cmd(["validate"], "validate-after-init"): return finish(run,t0,repo,fixture)
        add_governance(run, repo)
        if corrected:
            if not cmd(["dashboard"], "dashboard-after-governance-edit"): return finish(run,t0,repo,fixture)
            if not cmd(["validate"], "validate-after-governance-edits"): return finish(run,t0,repo,fixture)
        if not cmd(["new","work-item","Wire up the health check"], "create-work-item"): return finish(run,t0,repo,fixture)
        item_path, item = get_item(repo)
        fill_item(run, repo, item_path, item)
        if corrected and not cmd(["dashboard"], "dashboard-after-work-item-edit"): return finish(run,t0,repo,fixture)
        if not cmd(["validate"], "validate-before-implementation"): return finish(run,t0,repo,fixture)
        source = repo/"src"/"health.py"; source.parent.mkdir(parents=True,exist_ok=True)
        source.write_text('def health_check():\n    return "ok"\n',encoding="utf-8")
        record_edit(run,repo,"add deterministic health-check implementation",[source])
        if corrected and not cmd(["verify"],"verify-unrecorded-candidate"): return finish(run,t0,repo,fixture)
        before=set((repo/"evidence"/"runs").glob("*.json"))
        if not cmd(["verify","--evidence-work-item",item["id"]],"verify-recorded-candidate"): return finish(run,t0,repo,fixture)
        link_evidence(run,repo,item_path,before)
        if corrected and not cmd(["dashboard"],"dashboard-after-evidence-link"): return finish(run,t0,repo,fixture)
        if not cmd(["validate"],"validate-after-evidence-link"): return finish(run,t0,repo,fixture)
        complete_item(run,repo,item_path)
        if not cmd(["dashboard"],"regenerate-dashboard"): return finish(run,t0,repo,fixture)
        if not cmd(["validate"],"validate-completed-state"): return finish(run,t0,repo,fixture)
        return finish(run,t0,repo,fixture)
    except Exception as exc:
        run["failure"]={"label":"harness", "reason":f"{type(exc).__name__}: {exc}"}
        return finish(run,t0,repo if repo.exists() else None,fixture)


def p95(values: list[float]) -> float:
    return sorted(values)[max(0, math.ceil(.95*len(values))-1)]


def compare(a: dict[str,Any] | None,b: dict[str,Any] | None) -> dict[str,Any]:
    if a is None or b is None: return {"equivalent":False,"mismatched_paths":["<incomplete run>"]}
    am={f["path"]:f["sha256_normalized"] for f in a["files"]}; bm={f["path"]:f["sha256_normalized"] for f in b["files"]}
    mismatches=sorted(path for path in set(am)|set(bm) if am.get(path)!=bm.get(path))
    if a["directories"]!=b["directories"]: mismatches.append("<directory set>")
    return {"equivalent":not mismatches,"mismatched_paths":mismatches}


def side_summary(runs: list[dict[str,Any]], side: str) -> dict[str,Any]:
    selected=[r for r in runs if r["phase"]=="sample" and r["side"]==side]
    sums=[r["command_elapsed_ms_sum"] for r in selected]
    labels=sorted({c["label"] for r in selected for c in r["commands"]})
    per={}
    for label in labels:
        vals=[c["elapsed_ms"] for r in selected for c in r["commands"] if c["label"]==label]
        per[label]={"count":len(vals),"median_elapsed_ms":round(statistics.median(vals),3),"p95_elapsed_ms":round(p95(vals),3)}
    count_keys=("repopact_cli_command_count","verification_command_count","direct_validate_count","verify_count","init_count","new_count","dashboard_count")
    return {"runs":len(selected),"all_succeeded":all(r["success"] for r in selected),"sample_exclusions":[],
        "command_elapsed_ms_sum_median":round(statistics.median(sums),3),"command_elapsed_ms_sum_p95":round(p95(sums),3),
        "command_elapsed_ms_sum_min":round(min(sums),3),"command_elapsed_ms_sum_max":round(max(sums),3),
        "command_counts":{k:sorted({r[k] for r in selected}) for k in count_keys},"per_command":per}


def run_all(identity: dict[str,Any]) -> dict[str,Any]:
    if FIXTURES.exists(): raise ExperimentError(f"fixture parent exists; refusing overwrite: {FIXTURES}")
    FIXTURES.mkdir(parents=True)
    runs=[]
    schedule=[]
    for n in range(1,WARMUPS+1):
        sides=("corrected_baseline","current") if n==1 else ("current","corrected_baseline")
        schedule += [(f"warmup-{n:02d}",s,i+1,"warmup") for i,s in enumerate(sides)]
    for n in range(1,PAIRS+1):
        sides=("corrected_baseline","current") if n%2 else ("current","corrected_baseline")
        schedule += [(f"pair-{n:02d}",s,i+1,"sample") for i,s in enumerate(sides)]
    try:
        for group,side,order,phase in schedule:
            run=execute(f"{group}-{side}",side,order,FIXTURES/group)
            run["phase"]=phase; runs.append(run)
    finally:
        if FIXTURES.exists():
            if FIXTURES.resolve().parent!=Path(tempfile.gettempdir()).resolve(): raise ExperimentError("unsafe fixture cleanup refused")
            shutil.rmtree(FIXTURES)
    pair_rows=[]
    groups=[f"warmup-{n:02d}" for n in range(1,WARMUPS+1)]+[f"pair-{n:02d}" for n in range(1,PAIRS+1)]
    for group in groups:
        old=next(r for r in runs if r["run_id"]==f"{group}-corrected_baseline")
        new=next(r for r in runs if r["run_id"]==f"{group}-current")
        pair_rows.append({"group":group,"phase":old["phase"],
            "first_side":"corrected_baseline" if old["execution_order_in_pair"]==1 else "current",
            "state_equivalence":compare(old.get("final_state"),new.get("final_state")),
            "corrected_baseline_command_elapsed_ms_sum":old["command_elapsed_ms_sum"],
            "current_command_elapsed_ms_sum":new["command_elapsed_ms_sum"],
            "paired_delta_ms_corrected_minus_current":round(old["command_elapsed_ms_sum"]-new["command_elapsed_ms_sum"],3)})
    sample_pairs=[p for p in pair_rows if p["phase"]=="sample"]
    deltas=[p["paired_delta_ms_corrected_minus_current"] for p in sample_pairs]
    result={"measurement":"same-engine command-sequence mechanism timing; no productivity/effectiveness/causal or historical-binary claim",
        "protocol":{"preregistration_commit":identity["preregistration_commit"],"harness_sha256":digest(Path(__file__).read_bytes()),
            "warmups_per_side":WARMUPS,"matched_pairs":PAIRS,"alternating_order":True,"sample_exclusions":[],"model_calls":0,
            "corrected_baseline_insertions":["dashboard before validate-after-governance-edits","dashboard before validate-before-implementation","dashboard before validate-after-evidence-link"]},
        "sources":identity["tutorial_sources"],
        "environment":{**identity["python_cli"],**identity["engine"],**identity["host"],"storage_preflight":identity["storage_preflight"]},
        "normalization":{"generated_id":ID_TOKEN,"time_keys":sorted(TIME_KEYS),"duration_keys":sorted(DURATION_KEYS),
            "scope":"generated verification manifest only; all other files compared byte-for-byte"},
        "runs":runs,"pairwise_state_equivalence":pair_rows,
        "summary":{"corrected_baseline":side_summary(runs,"corrected_baseline"),"current":side_summary(runs,"current"),
            "sample_pair_deltas_ms_corrected_minus_current":deltas,"sample_pair_delta_median_ms":round(statistics.median(deltas),3),
            "sample_pair_delta_p95_ms":round(p95(deltas),3),"all_runs_succeeded":all(r["success"] for r in runs),
            "all_sample_final_states_equivalent":all(p["state_equivalence"]["equivalent"] for p in sample_pairs)}}
    result["ac1_supported"]=result["summary"]["all_runs_succeeded"] and result["summary"]["all_sample_final_states_equivalent"] and len(sample_pairs)==PAIRS
    OUT.write_text(json.dumps(result,ensure_ascii=False,indent=2)+"\n",encoding="utf-8")
    return result


def main() -> int:
    parser=argparse.ArgumentParser(); group=parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--preflight",action="store_true",help="check pinned inputs without running sequences")
    group.add_argument("--run",action="store_true",help="run the fixed protocol")
    args=parser.parse_args()
    try:
        identity=preflight(args.run)
        if args.preflight: print(json.dumps(identity,indent=2)); return 0
        result=run_all(identity)
        print(json.dumps({"output":str(OUT),"summary":result["summary"],"ac1_supported":result["ac1_supported"]},indent=2))
        return 0 if result["ac1_supported"] else 1
    except (ExperimentError,subprocess.CalledProcessError,OSError,ValueError) as exc:
        print(f"WI083 AC-1 comparison failed closed: {type(exc).__name__}: {exc}",file=sys.stderr); return 2


if __name__ == "__main__":
    raise SystemExit(main())
