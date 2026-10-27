"""Run repeatable proposal benchmarks against disposable repository copies."""

from __future__ import annotations

import argparse
import json
import os
import platform
import shutil
import sys
import tempfile
import time
from datetime import date
from pathlib import Path
from typing import Any

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPOSITORY_ROOT))

from repopact.engine_client import EngineClient  # noqa: E402


def percentile(values: list[float], fraction: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    index = max(0, min(len(ordered) - 1, int((len(ordered) * fraction + 0.999999)) - 1))
    return round(ordered[index], 3)


def summarize(values: list[float]) -> dict[str, float | None]:
    return {
        "p50_ms": percentile(values, 0.50),
        "p95_ms": percentile(values, 0.95),
        "worst_ms": round(max(values), 3) if values else None,
    }


def summarize_counts(values: list[float]) -> dict[str, float | None]:
    return {
        "p50": percentile(values, 0.50),
        "p95": percentile(values, 0.95),
        "worst": round(max(values), 3) if values else None,
    }


def record_counts(root: Path) -> dict[str, int]:
    def count(pattern: str) -> int:
        return sum(1 for _ in root.glob(pattern))

    return {
        "work_items": count("work/*/*/work-item.json"),
        "evidence_records": count("evidence/runs/*.json"),
        "audit_findings": count("audits/findings/*.json"),
        "source_files": sum(
            1
            for path in root.rglob("*")
            if path.is_file()
            and not {
                ".git",
                "target",
                ".venv",
                "__pycache__",
                ".pytest_cache",
                "node_modules",
                "build",
                "dist",
            }.intersection(path.relative_to(root).parts)
        ),
    }


def ignore_copy(_directory: str, names: list[str]) -> set[str]:
    ignored = {
        ".git", "target", ".venv", "__pycache__", ".pytest_cache",
        "node_modules", "build", "dist", ".repopact-transactions",
    }
    return {name for name in names if name in ignored}


def run_benchmark(label: str, source: Path, repetitions: int, engine: Path | None) -> dict[str, Any]:
    with tempfile.TemporaryDirectory(prefix=f"repopact-bench-{label}-") as temporary:
        root = Path(temporary) / "repository"
        shutil.copytree(source, root, ignore=ignore_copy, symlinks=True)
        warmup_client = EngineClient(engine=engine, timeout=10.0) if engine else EngineClient(timeout=10.0)
        warmup_started = time.perf_counter()
        warmup_response = warmup_client.call(
            "work.proposal-plan",
            root=root,
            params={
                "title": f"Benchmark warmup {label}",
                "date": date.today().isoformat(),
                "preflight_confirmed_before_work_started": True,
                "idempotency_key": f"bench-{label}-{os.getpid()}-warmup",
            },
        )
        warmup_result = warmup_response.get("result") or {}
        if not warmup_result.get("applicable"):
            raise RuntimeError(
                f"{label} warmup plan was rejected: "
                + json.dumps(
                    warmup_result.get("diagnostics", warmup_response.get("diagnostics", [])),
                    sort_keys=True,
                )
            )
        warmup = {
            "wall_ms": round((time.perf_counter() - warmup_started) * 1000, 3),
            "client_metrics": warmup_response.get("client_metrics", {}),
            "engine_metrics": warmup_result.get("metrics", {}),
        }

        samples: list[dict[str, Any]] = []
        for index in range(repetitions):
            key = f"bench-{label}-{os.getpid()}-{index}"
            params = {
                "title": f"Benchmark proposal {label} {index}",
                "date": date.today().isoformat(),
                "preflight_confirmed_before_work_started": True,
                "idempotency_key": key,
            }
            client = EngineClient(engine=engine, timeout=10.0) if engine else EngineClient(timeout=10.0)
            started = time.perf_counter()
            response = client.call("work.propose", root=root, params=params)
            wall_ms = (time.perf_counter() - started) * 1000
            result = response.get("result") or {}
            if not result.get("success"):
                raise RuntimeError(
                    f"{label} proposal {index} was rejected: "
                    + json.dumps(result.get("diagnostics", response.get("diagnostics", [])), sort_keys=True)
                )
            samples.append({
                "wall_ms": round(wall_ms, 3),
                "client_metrics": response.get("client_metrics", {}),
                "engine_metrics": result.get("metrics", {}),
                "record_counts": record_counts(root),
            })

        phase_names = (
            "snapshot_ms", "planning_ms", "proposal_lock_wait_ms", "idempotency_lookup_ms",
            "stale_check_ms", "critical_validation_ms", "authority_checks_ms", "work_schema_ms",
            "dependency_graph_ms", "maintenance_diagnostics_ms", "filesystem_staging_ms",
            "atomic_commit_ms", "dashboard_regeneration_ms", "engine_execution_ms",
            "external_integrations_ms",
        )
        count_names = ("records_examined", "source_files_read")
        phases: dict[str, list[float]] = {name: [] for name in phase_names}
        counts: dict[str, list[float]] = {name: [] for name in count_names}
        phases["wall_ms"] = []
        phases["python_client_total_ms"] = []
        peak_working_set: list[float] = []
        for sample in samples:
            phases["wall_ms"].append(sample["wall_ms"])
            client_metrics = sample["client_metrics"]
            if isinstance(client_metrics.get("python_client_total_ms"), (int, float)):
                phases["python_client_total_ms"].append(float(client_metrics["python_client_total_ms"]))
            peak = client_metrics.get("operation_peak_working_set_bytes")
            if isinstance(peak, (int, float)):
                peak_working_set.append(float(peak))
            for name in phase_names:
                value = sample["engine_metrics"].get(name)
                if isinstance(value, (int, float)):
                    phases[name].append(float(value))
            for name in count_names:
                value = sample["engine_metrics"].get(name)
                if isinstance(value, (int, float)):
                    counts[name].append(float(value))
        return {
            "label": label,
            "source_root": str(source),
            "repetitions": repetitions,
            "initial_record_counts": record_counts(source),
            "warmup_sample_excluded_from_statistics": warmup,
            "phase_statistics": {name: summarize(values) for name, values in phases.items()},
            "record_count_statistics": {name: summarize_counts(values) for name, values in counts.items()},
            "peak_operation_working_set_bytes": {
                "p50": percentile(peak_working_set, 0.50),
                "p95": percentile(peak_working_set, 0.95),
                "worst": max(peak_working_set) if peak_working_set else None,
            },
            "memory_measurement": "OS high-water mark when supported; null means unavailable",
            "samples": samples,
            "target_p95_below_5000ms_met_for_this_fixture": (percentile(phases["wall_ms"], 0.95) or 0) < 5000,
        }


def parse_roots(values: list[str]) -> list[tuple[str, Path]]:
    roots: list[tuple[str, Path]] = []
    for value in values:
        label, separator, raw_path = value.partition("=")
        path = Path(raw_path).expanduser().resolve()
        if not separator or not label.strip() or not path.is_dir():
            raise argparse.ArgumentTypeError("each --root must be LABEL=PATH to an existing directory")
        roots.append((label.strip(), path))
    return roots


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", action="append", required=True, metavar="LABEL=PATH")
    parser.add_argument("--repetitions", type=int, default=5)
    parser.add_argument("--engine", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.repetitions < 1 or args.repetitions > 100:
        parser.error("--repetitions must be between 1 and 100")
    try:
        roots = parse_roots(args.root)
        report = {
            "benchmark": "repopact-work-propose",
            "timestamp_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "host": {
                "platform": platform.platform(),
                "processor": platform.processor(),
                "python": sys.version.split()[0],
            },
            "engine": str(args.engine.resolve()) if args.engine else "environment-discovered",
            "target_ms_p95": 5000,
            "operation_deadline_ms": 10000,
            "runs": [run_benchmark(label, root, args.repetitions, args.engine) for label, root in roots],
        }
    except (argparse.ArgumentTypeError, RuntimeError, OSError) as exc:
        print(f"Benchmark failed: {exc}", file=sys.stderr)
        return 1
    encoded = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(encoded, encoding="utf-8")
    print(encoded, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
