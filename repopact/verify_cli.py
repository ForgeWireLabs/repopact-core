"""CLI entry point for WI046 local verification profiles.

This module exists independently of hosted CI. The public ``repopact verify``
command delegates here, while hosted adapters call the same local contract instead
of maintaining a second semantic command list.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from .verification import (
    VerificationConfigError,
    _candidate_identity,
    _work_item_exists,
    load_contract,
    new_evidence_id,
    record_evidence,
    render_human,
    render_json,
    run_profile,
)
from .verification_capture import CaptureJournal, CaptureStateError


def main(argv: list[str] | None = None) -> int:
    argv = sys.argv[1:] if argv is None else list(argv)
    if argv[:1] == ["status"]:
        from . import verify_status

        return verify_status.main(argv[1:])
    parser = argparse.ArgumentParser(
        description="Run a repository-defined RepoPact verification profile locally"
    )
    parser.add_argument(
        "profile",
        nargs="?",
        help="Profile name; defaults to governance/verification.json default_profile",
    )
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON")
    parser.add_argument(
        "--evidence-work-item",
        metavar="ID",
        help="Record this actual invocation as immutable evidence for the named RepoPact work item",
    )
    parser.add_argument(
        "--evidence-id",
        help="Optional explicit evidence id; never overwrites an existing record",
    )
    args = parser.parse_args(argv)
    journal: CaptureJournal | None = None
    try:
        root = args.root.resolve()
        evidence_id = args.evidence_id
        candidate_at_start = None
        profile_name = args.profile
        if args.evidence_work_item:
            contract = load_contract(root)
            profile_name = profile_name or contract.get("default_profile")
            if not profile_name:
                raise VerificationConfigError("no verification profile supplied and no default_profile is declared")
            if not _work_item_exists(root, args.evidence_work_item):
                raise VerificationConfigError(
                    f"cannot record verification evidence for missing work item {args.evidence_work_item!r}"
                )
            evidence_id = evidence_id or new_evidence_id(profile_name)
            candidate_at_start = _candidate_identity(root)
            journal = CaptureJournal.begin(
                root,
                evidence_id=evidence_id,
                work_item=args.evidence_work_item,
                profile=profile_name,
                candidate_at_start=candidate_at_start,
            )
        elif args.evidence_id:
            raise VerificationConfigError("--evidence-id requires --evidence-work-item")

        report = run_profile(
            root,
            profile_name,
            progress_callback=journal.record_step if journal else None,
            candidate_at_start=candidate_at_start,
        )
        output_report = report
        expected_evidence_path = None
        if journal:
            expected_evidence_path = f"evidence/runs/{evidence_id}.json"
            output_report = report.with_evidence(expected_evidence_path)

            path = record_evidence(
                root,
                report,
                args.evidence_work_item,
                evidence_id=evidence_id,
                artifacts=journal.preserve_failed_log(report.status),
            )
            if path != expected_evidence_path:
                journal.mark_incomplete(f"recorded evidence path differed from expected: {path}")
                return 2

        rendered = render_json(output_report) if args.json else render_human(output_report)
        try:
            written = sys.stdout.write(rendered)
            if written != len(rendered):
                raise OSError(f"short verification output write: {written!r} of {len(rendered)} characters")
            sys.stdout.flush()
        except OSError as exc:
            if journal:
                journal.mark_incomplete(f"verification output capture failed: {exc}")
            print(
                f"RepoPact verification output capture failed; result is INCOMPLETE: {exc}",
                file=sys.stderr,
            )
            return 2

        if journal:
            journal.complete(passing=report.status == "pass")
    except VerificationConfigError as exc:
        if journal:
            try:
                journal.mark_incomplete(str(exc), phase="recording-failed")
            except CaptureStateError:
                pass
        print(f"RepoPact verification configuration error: {exc}", file=sys.stderr)
        return 2
    except CaptureStateError as exc:
        if journal:
            try:
                journal.mark_incomplete(str(exc), phase="capture-failed")
            except CaptureStateError:
                pass
        print(f"RepoPact verification capture error; result is INCOMPLETE: {exc}", file=sys.stderr)
        return 2
    except KeyboardInterrupt:
        if journal:
            try:
                journal.mark_incomplete("local verification was interrupted", phase="interrupted")
            except CaptureStateError:
                pass
        print("RepoPact verification was interrupted; result is INCOMPLETE", file=sys.stderr)
        return 2
    except OSError as exc:
        if journal:
            try:
                journal.mark_incomplete(f"local verification storage failure: {exc}", phase="storage-failed")
            except CaptureStateError:
                pass
        print(f"RepoPact verification storage failure; result is INCOMPLETE: {exc}", file=sys.stderr)
        return 2
    except Exception as exc:
        if journal:
            try:
                journal.mark_incomplete(f"local verification failed before durable evidence: {exc}", phase="runner-failed")
            except CaptureStateError:
                pass
        print(f"RepoPact verification failed before durable evidence; result is INCOMPLETE: {exc}", file=sys.stderr)
        return 2
    return report.exit_code


if __name__ == "__main__":
    raise SystemExit(main())
