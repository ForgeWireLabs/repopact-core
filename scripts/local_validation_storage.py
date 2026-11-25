#!/usr/bin/env python3
"""Inventory and reclaim only explicitly registered local validation artifacts."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from repopact.local_storage import (  # noqa: E402
    LocalStorageError,
    cleanup_owned_target,
    cleanup_owned_worktree,
    create_owned_target,
    create_owned_worktree,
    disk_preflight,
    inventory,
    seal_owned_target,
)


def _path(value: str) -> Path:
    return Path(value).expanduser()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Fail-closed local CI storage inventory and opt-in cleanup"
    )
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="repository worktree root")
    commands = parser.add_subparsers(dest="command", required=True)

    commands.add_parser("inventory", help="measure known repository builds and registered worktrees")
    preflight = commands.add_parser("preflight", help="check free space before local CI")
    preflight.add_argument("--minimum-free-gib", type=float, default=12.0)

    create_target = commands.add_parser("create-target", help="create and register a new Cargo target directory")
    create_target.add_argument("path")

    seal_target = commands.add_parser("seal-target", help="record exact files in a tool-created Cargo target")
    seal_target.add_argument("path")

    cleanup_target = commands.add_parser("cleanup-target", help="dry-run by default; use --apply to reclaim")
    cleanup_target.add_argument("path")
    cleanup_target.add_argument("--apply", action="store_true", help="perform the verified cleanup")

    create_worktree = commands.add_parser("create-worktree", help="create and register a detached disposable worktree")
    create_worktree.add_argument("path")
    create_worktree.add_argument("revision")

    cleanup_worktree = commands.add_parser("cleanup-worktree", help="dry-run by default; use --apply to reclaim")
    cleanup_worktree.add_argument("path")
    cleanup_worktree.add_argument("--apply", action="store_true", help="perform the verified cleanup")

    args = parser.parse_args(argv)
    root = args.root.resolve()
    try:
        if args.command == "inventory":
            result = inventory(root)
        elif args.command == "preflight":
            minimum = int(args.minimum_free_gib * 1024**3)
            if minimum < 0:
                raise LocalStorageError("minimum-free-gib must be nonnegative")
            result = disk_preflight(root, minimum_free_bytes=minimum)
        elif args.command == "create-target":
            result = create_owned_target(root, _path(args.path))
        elif args.command == "seal-target":
            result = seal_owned_target(root, _path(args.path))
        elif args.command == "cleanup-target":
            result = cleanup_owned_target(root, _path(args.path), apply=args.apply)
        elif args.command == "create-worktree":
            result = create_owned_worktree(root, _path(args.path), args.revision)
        else:
            result = cleanup_owned_worktree(root, _path(args.path), apply=args.apply)
    except (LocalStorageError, OSError, ValueError) as exc:
        print(json.dumps({"status": "refused", "error": str(exc)}, indent=2), file=sys.stderr)
        return 2
    print(json.dumps(result, indent=2, sort_keys=True))
    if args.command == "preflight" and not result.get("ready"):
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
