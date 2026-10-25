"""Run the published RepoPact conformance suite.

The runner materializes each fixture as an isolated temporary repository, injects
the canonical schemas from this checkout, runs a RepoPact implementation, and
compares the result with conformance/manifest.json.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from dataclasses import dataclass
from pathlib import Path

import jsonschema

from . import engine_client
from . import generate_dashboard
from . import legacy_validate


ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "conformance" / "manifest.json"


@dataclass(frozen=True)
class CaseResult:
    case_id: str
    passed: bool
    detail: str


def load_manifest(path: Path = MANIFEST) -> dict:
    with path.open(encoding="utf-8") as handle:
        data = json.load(handle)
    if not isinstance(data, dict):
        raise ValueError(f"manifest must be a JSON object: {path}")
    schema = json.loads(
        (ROOT / "repopact" / "schemas" / "conformance-manifest.schema.json").read_text(encoding="utf-8")
    )
    jsonschema.Draft202012Validator(schema).validate(data)
    validate_manifest_coverage(data)
    return data


def validate_manifest_coverage(manifest: dict) -> None:
    """Enforce bidirectional rule/case coverage with deterministic diagnostics."""
    errors: list[str] = []
    rule_ids = [str(rule.get("id", "")) for rule in manifest.get("rules", []) if isinstance(rule, dict)]
    case_ids = [str(case.get("id", "")) for case in manifest.get("cases", []) if isinstance(case, dict)]
    if len(rule_ids) != len(set(rule_ids)):
        errors.append("rule ids must be unique")
    if len(case_ids) != len(set(case_ids)):
        errors.append("case ids must be unique")
    known = set(rule_ids)
    referenced = {
        str(rule_id)
        for case in manifest.get("cases", []) if isinstance(case, dict)
        for rule_id in case.get("rules", [])
    }
    uncovered = sorted(known - referenced)
    unknown = sorted(referenced - known)
    if uncovered:
        errors.append(f"rules without conformance cases: {', '.join(uncovered)}")
    if unknown:
        errors.append(f"cases reference unknown rules: {', '.join(unknown)}")
    if errors:
        raise ValueError("; ".join(errors))


def _copy_overlay(src: Path, dst: Path) -> None:
    for path in src.rglob("*"):
        if path.is_dir() or path.name == "meta.json":
            continue
        target = dst / path.relative_to(src)
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, target)


def materialize_case(root: Path, fixtures_root: Path, case: dict) -> Path:
    repo = root / "repo"
    overlay_on = case.get("overlay_on")
    if overlay_on:
        shutil.copytree(fixtures_root / str(overlay_on), repo)
        _copy_overlay(fixtures_root / str(case["path"]), repo)
    else:
        shutil.copytree(fixtures_root / str(case["path"]), repo)
    shutil.copytree(ROOT / "repopact" / "schemas", repo / "schemas")
    # Overlays intentionally mutate source records. Materialize their canonical
    # projection unless the dashboard itself is the isolated signal under test.
    dashboard_mode = case.get("dashboard", "canonical")
    if dashboard_mode == "canonical":
        generate_dashboard.write_dashboard(repo)
    elif dashboard_mode == "remove":
        (repo / "audits" / "reports" / "dashboard.md").unlink(missing_ok=True)
    return repo


def run_command(command: str, repo: Path) -> subprocess.CompletedProcess[str]:
    rendered = command.format(repo=str(repo))
    return subprocess.run(rendered, shell=True, text=True, capture_output=True)


def evaluate_case(case: dict, command: str, fixtures_root: Path) -> CaseResult:
    case_id = str(case["id"])
    with tempfile.TemporaryDirectory(prefix=f"repopact-conformance-{case_id}-") as tmp:
        repo = materialize_case(Path(tmp), fixtures_root, case)
        # The fixture oracle is the explicit Python compatibility surface, not
        # the historical pre-cutover validator alone. This keeps fixture
        # isolation independent of the Rust implementation while allowing new
        # migrated semantic rules (such as WI046 verification contracts) to be
        # represented without pretending the old module is still complete.
        reference_problems = legacy_validate.validate(repo)
        proc = run_command(command, repo)
    output = "\n".join(part for part in (proc.stdout, proc.stderr) if part)
    expect = case.get("expect")
    if expect == "accept":
        blocking = [problem for problem in reference_problems if problem.severity == "error"]
        if blocking:
            observed = "; ".join(problem.message for problem in blocking)
            return CaseResult(case_id, False, f"fixture isolation failed: unexpected violations: {observed}")
        passed = proc.returncode == 0
        detail = "accepted" if passed else f"expected accept, exit={proc.returncode}: {output.strip()}"
        return CaseResult(case_id, passed, detail)
    expected = str(case.get("expected_message", ""))
    matching = [problem for problem in reference_problems if expected in problem.message]
    unexpected = [problem for problem in reference_problems if expected not in problem.message]
    if len(matching) != 1 or unexpected:
        observed = "; ".join(problem.message for problem in reference_problems) or "none"
        extra = "; ".join(problem.message for problem in unexpected) or "none"
        return CaseResult(
            case_id,
            False,
            f"fixture isolation failed: expected one '{expected}' violation; observed: {observed}; "
            f"unexpected violations: {extra}",
        )
    passed = proc.returncode != 0 and expected in output
    detail = (
        f"rejected with '{expected}'"
        if passed
        else f"expected reject containing '{expected}', exit={proc.returncode}: {output.strip()}"
    )
    return CaseResult(case_id, passed, detail)


def run_suite(command: str, manifest_path: Path = MANIFEST) -> list[CaseResult]:
    manifest = load_manifest(manifest_path)
    fixtures_root = manifest_path.parent / str(manifest.get("fixtures_root", "fixtures"))
    return [evaluate_case(case, command, fixtures_root) for case in manifest.get("cases", [])]


def _git_source_state(root: Path) -> dict[str, object]:
    def git(*args: str) -> str | None:
        result = subprocess.run(
            ["git", "-C", str(root), *args], text=True, capture_output=True, check=False
        )
        return result.stdout.strip() if result.returncode == 0 else None

    revision = git("rev-parse", "--verify", "HEAD")
    status = git("status", "--porcelain", "--untracked-files=all")
    return {
        "revision": revision,
        "dirty": status is None or bool(status),
        "status_porcelain": status,
    }


def _check_semantic_manifest() -> None:
    result = subprocess.run(
        [sys.executable, str(ROOT / "scripts" / "generate_engine_semantic_identity.py"), "--check"],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode:
        detail = "\n".join(part for part in (result.stdout, result.stderr) if part).strip()
        raise engine_client.EngineProtocolError(
            "canonical conformance requires a current semantic input manifest"
            + (f":\n{detail}" if detail else "")
        )


def evaluate_canonical_case(case: dict, fixtures_root: Path, client: engine_client.EngineClient) -> CaseResult:
    """Run one fixture through the single admitted process used for this suite."""
    case_id = str(case["id"])
    with tempfile.TemporaryDirectory(prefix=f"repopact-conformance-{case_id}-") as tmp:
        repo = materialize_case(Path(tmp), fixtures_root, case)
        reference_problems = legacy_validate.validate(repo)
        response = client.call("validate", root=repo)
        result = engine_client._validated_result(response)
    diagnostics = result["diagnostics"]
    output = "\n".join(str(item.get("message", "")) for item in diagnostics)
    expect = case.get("expect")
    if expect == "accept":
        blocking = [problem for problem in reference_problems if problem.severity == "error"]
        if blocking:
            observed = "; ".join(problem.message for problem in blocking)
            return CaseResult(case_id, False, f"fixture isolation failed: unexpected violations: {observed}")
        passed = result["valid"]
        detail = "accepted" if passed else f"expected accept: {output.strip()}"
        return CaseResult(case_id, passed, detail)
    expected = str(case.get("expected_message", ""))
    matching = [problem for problem in reference_problems if expected in problem.message]
    unexpected = [problem for problem in reference_problems if expected not in problem.message]
    if len(matching) != 1 or unexpected:
        observed = "; ".join(problem.message for problem in reference_problems) or "none"
        extra = "; ".join(problem.message for problem in unexpected) or "none"
        return CaseResult(
            case_id, False,
            f"fixture isolation failed: expected one '{expected}' violation; observed: {observed}; "
            f"unexpected violations: {extra}",
        )
    passed = not result["valid"] and expected in output
    detail = (
        f"rejected with '{expected}'"
        if passed
        else f"expected reject containing '{expected}': {output.strip()}"
    )
    return CaseResult(case_id, passed, detail)


def run_canonical_suite(manifest_path: Path = MANIFEST) -> tuple[list[CaseResult], dict[str, object]]:
    """Run canonical validation in one admitted process and capture its image."""
    _check_semantic_manifest()
    manifest = load_manifest(manifest_path)
    fixtures_root = manifest_path.parent / str(manifest.get("fixtures_root", "fixtures"))
    executable = engine_client.locate_engine(allow_checkout=True)
    source = _git_source_state(ROOT)
    with engine_client.EngineClient(engine=executable) as client:
        client.check_compatibility("validate")
        results = [
            evaluate_canonical_case(case, fixtures_root, client)
            for case in manifest.get("cases", [])
        ]
        provenance = client.provenance()
    build_info = provenance.get("engine", {}).get("build_info")
    payload = {
        "format": "repopact-engine-conformance-provenance-v1",
        "recorded_at": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "source": source,
        "engine": provenance,
        "build_identity": None,
        "conformance": {
            "manifest": str(manifest_path.resolve()),
            "suite_version": manifest.get("suite_version"),
            "passed": sum(result.passed for result in results),
            "failed": sum(not result.passed for result in results),
            "cases": [
                {"id": result.case_id, "status": "PASS" if result.passed else "FAIL", "detail": result.detail}
                for result in results
            ],
        },
    }
    if isinstance(build_info, dict):
        engine_metadata = provenance.get("engine", {})
        identity_fields = {
            "source_revision": build_info.get("source_revision"),
            "rustc_version": build_info.get("rustc_version"),
            "target": build_info.get("target"),
            "profile": build_info.get("profile"),
            "product_version": engine_metadata.get("product_version"),
            "protocol": engine_metadata.get("protocol"),
            "protocol_version": engine_metadata.get("protocol_version"),
            "semantic_identity": engine_metadata.get("semantic_identity"),
        }
        payload["build_identity"] = "repopact-build-v1:sha256:" + hashlib.sha256(
            json.dumps(identity_fields, sort_keys=True, separators=(",", ":")).encode("utf-8")
        ).hexdigest()
    if source["dirty"]:
        provenance.setdefault("incomplete_reasons", []).append(
            "source checkout is dirty; the source commit alone does not identify its working tree"
        )
        provenance["status"] = "incomplete"
    if build_info and source.get("revision") and build_info.get("source_revision") != source.get("revision"):
        provenance.setdefault("incomplete_reasons", []).append(
            "engine build source revision does not match the conformance checkout"
        )
        provenance["status"] = "incomplete"
    payload["engine"] = provenance
    return results, payload


def _write_provenance(payload: dict[str, object], requested: Path | None) -> Path:
    if requested is None:
        stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%S%fZ")
        destination = ROOT / "evidence" / "runs" / f"{stamp}-wi076-ac6-engine-conformance.json"
    else:
        destination = requested if requested.is_absolute() else Path.cwd() / requested
    timestamp = str(payload["recorded_at"])
    evidence = {
        "$schema": Path(os.path.relpath(
            ROOT / "repopact" / "schemas" / "evidence-run.schema.json",
            destination.parent.resolve(),
        )).as_posix(),
        "id": "wi076-ac6-engine-conformance-" + timestamp.replace(":", "").replace("-", ""),
        "timestamp": timestamp,
        "work_item": "076",
        "result": payload["result"],
        "provenance": "concrete" if payload["engine"]["status"] == "complete" else "provisional",
        "commands": [{
            "command": "python -m repopact.run_conformance (canonical Rust engine; process-image provenance)",
            "exit_code": payload["exit_code"],
            "summary": (
                f"canonical={payload['conformance']['passed']}/{payload['conformance']['passed'] + payload['conformance']['failed']}; "
                f"WI050={payload['conformance']['wi050_admission_corpus']['passed']}/"
                f"{payload['conformance']['wi050_admission_corpus']['passed'] + payload['conformance']['wi050_admission_corpus']['failed']}; "
                f"process-image provenance={payload['engine']['status']}"
            ),
        }],
        "artifacts": ["conformance/manifest.json", "build/semantic-input-manifest.json"],
        "environment": {
            "evidence_kind": payload["format"],
            "source": payload["source"],
            "engine": payload["engine"],
            "build_identity": payload["build_identity"],
            "conformance": payload["conformance"],
        },
    }
    schema = json.loads(
        (ROOT / "repopact" / "schemas" / "evidence-run.schema.json").read_text(encoding="utf-8")
    )
    jsonschema.Draft202012Validator(schema).validate(evidence)
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return destination


def main() -> int:
    parser = argparse.ArgumentParser(description="Run the RepoPact conformance suite")
    parser.add_argument("--manifest", type=Path, default=MANIFEST)
    parser.add_argument(
        "--command",
        default=None,
        help="Implementation command template; {repo} is replaced with the fixture repo path.",
    )
    parser.add_argument(
        "--provenance-out",
        type=Path,
        default=None,
        help="Path for canonical process-image provenance (default: evidence/runs/<UTC>-wi076-ac6-engine-conformance.json).",
    )
    parser.add_argument(
        "--legacy-python",
        action="store_true",
        help="Also run the independent Python compatibility validator comparator.",
    )
    args = parser.parse_args()

    try:
        provenance = None
        if args.command is None:
            results, provenance = run_canonical_suite(args.manifest.resolve())
        else:
            results = run_suite(args.command, args.manifest.resolve())
    except (OSError, ValueError, json.JSONDecodeError, jsonschema.ValidationError, engine_client.EngineError) as exc:
        print(f"CONFORMANCE MANIFEST ERROR: {exc}", file=sys.stderr)
        return 2
    failed = [result for result in results if not result.passed]
    for result in results:
        status = "PASS" if result.passed else "FAIL"
        print(f"{status} {result.case_id}: {result.detail}")
    print(f"\nCanonical Rust conformance: {len(results) - len(failed)}/{len(results)} cases passed.")
    if args.legacy_python:
        legacy_command = f'"{sys.executable}" -m repopact.legacy_validate --root "{{repo}}"'
        legacy_results = run_suite(legacy_command, args.manifest.resolve())
        legacy_failed = [result for result in legacy_results if not result.passed]
        for result in legacy_results:
            status = "PASS" if result.passed else "FAIL"
            print(f"{status} legacy-python {result.case_id}: {result.detail}")
        print(
            f"\nPython compatibility comparator: "
            f"{len(legacy_results) - len(legacy_failed)}/{len(legacy_results)} cases passed."
        )
    from .admission_conformance import run_admission_corpus
    admission_results = run_admission_corpus()
    admission_failed = [row for row in admission_results if not row[1]]
    for case_id, passed, detail in admission_results:
        print(f"{'PASS' if passed else 'FAIL'} {case_id}: {detail}")
    print(f"\nWI050 admission corpus: {len(admission_results) - len(admission_failed)}/{len(admission_results)} vectors passed.")
    if provenance is not None:
        admission_summary = {
            "passed": len(admission_results) - len(admission_failed),
            "failed": len(admission_failed),
            "cases": [
                {"id": case_id, "status": "PASS" if passed else "FAIL", "detail": detail}
                for case_id, passed, detail in admission_results
            ],
        }
        provenance["conformance"]["wi050_admission_corpus"] = admission_summary
        any_test_failures = bool(failed or admission_failed or (args.legacy_python and legacy_failed))
        incomplete = provenance["engine"]["status"] != "complete"
        exit_code = 1 if any_test_failures or incomplete else 0
        provenance["result"] = "failed" if any_test_failures else "partial" if incomplete else "passed"
        provenance["exit_code"] = exit_code
        path = _write_provenance(provenance, args.provenance_out)
        print(f"Conformance provenance: {provenance['engine']['status']} ({path})")
    elif args.command is not None:
        print("Conformance provenance: NOT_RECORDED (custom command does not expose a correlated process image).")
    incomplete = provenance is not None and provenance["engine"]["status"] != "complete"
    return 1 if failed or admission_failed or incomplete or (args.legacy_python and legacy_failed) else 0


if __name__ == "__main__":
    sys.exit(main())
