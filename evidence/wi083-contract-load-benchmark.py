"""Reproduce the WI083 verification-contract handling microbenchmark."""

from __future__ import annotations

import argparse
import hashlib
import json
import platform
import statistics
import subprocess
import sys
import time
import types
from pathlib import Path

from repopact import verification


def baseline_module(root: Path, revision: str) -> types.ModuleType:
    source = subprocess.run(
        ["git", "show", f"{revision}:repopact/verification.py"],
        cwd=root,
        check=True,
        capture_output=True,
        text=True,
    ).stdout
    name = "_wi083_baseline_verification"
    module = types.ModuleType(name)
    module.__file__ = str(root / "repopact" / "verification.py")
    sys.modules[name] = module
    exec(compile(source, module.__file__, "exec"), module.__dict__)
    return module


def measure(function, *, warmups: int, samples: int, iterations: int) -> dict[str, object]:
    for _ in range(warmups):
        function()
    per_sample_ms = []
    for _ in range(samples):
        started = time.perf_counter()
        for _ in range(iterations):
            function()
        per_sample_ms.append((time.perf_counter() - started) * 1000 / iterations)
    return {
        "median_ms_per_operation": round(statistics.median(per_sample_ms), 4),
        "sample_ms_per_operation": [round(value, 4) for value in per_sample_ms],
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--baseline", default="b9dec03")
    parser.add_argument("--warmups", type=int, default=10)
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--iterations", type=int, default=250)
    args = parser.parse_args()
    root = args.root.resolve()
    baseline = baseline_module(root, args.baseline)

    def before_recorded_run() -> None:
        baseline.load_contract(root)
        baseline.load_contract(root)

    def after_recorded_run() -> None:
        loaded = verification._load_contract(root)
        verification._assert_contract_unchanged(loaded)

    config = (root / verification.CONFIG_REL).read_bytes()
    schema = verification._schema_bytes(root)
    result = {
        "baseline_revision": args.baseline,
        "candidate_source": "current checkout: repopact/verification.py",
        "operation": "recorded profile selection plus pre-execution contract check",
        "baseline_full_loads_per_operation": 2,
        "candidate_full_loads_per_operation": 1,
        "candidate_digest_rechecks_per_operation": 1,
        "config_sha256": hashlib.sha256(config).hexdigest(),
        "schema_sha256": hashlib.sha256(schema).hexdigest(),
        "python": sys.version.split()[0],
        "platform": platform.platform(),
        "warmups": args.warmups,
        "samples": args.samples,
        "iterations_per_sample": args.iterations,
        "before": measure(
            before_recorded_run,
            warmups=args.warmups,
            samples=args.samples,
            iterations=args.iterations,
        ),
        "after": measure(
            after_recorded_run,
            warmups=args.warmups,
            samples=args.samples,
            iterations=args.iterations,
        ),
    }
    result["median_ms_saved"] = round(
        result["before"]["median_ms_per_operation"]
        - result["after"]["median_ms_per_operation"],
        4,
    )
    result["microbenchmark_reduction_percent"] = round(
        100 * result["median_ms_saved"] / result["before"]["median_ms_per_operation"],
        2,
    )
    print(json.dumps(result, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
