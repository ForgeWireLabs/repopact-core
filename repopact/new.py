"""Stamp a new record from a template (work item 003, B2).

    repopact new work-item "Harden the widget"
    repopact new decision "Adopt event sourcing"
    repopact new policy "No silent retries"

Fills the next free id and today's date, then writes the record to its canonical
location. The stamped record is a valid starting point; fill in the body.
"""

from __future__ import annotations

import argparse
import re
import sys
from datetime import date
from pathlib import Path

from . import init_repo
from .repo_model import STATUSES

ROOT = Path(__file__).resolve().parents[1]


def slugify(title: str) -> str:
    return re.sub(r"-+", "-", re.sub(r"[^a-z0-9]+", "-", title.lower())).strip("-")


def _next_numeric(paths: list[Path], width: int, start: int) -> str:
    used = []
    for path in paths:
        match = re.match(r"([0-9]+)", path.name)
        if match:
            used.append(int(match.group(1)))
    return str(max(used + [start - 1]) + 1).zfill(width)


def _template_text(root: Path, name: str) -> str:
    local = root / "templates" / name
    if local.is_file():
        return local.read_text(encoding="utf-8")
    return init_repo._seed_dir("templates").joinpath(name).read_text(encoding="utf-8")


def new_work_item(title: str, today: date, root: Path = ROOT, status: str = "active") -> Path:
    from .engine_client import EngineClient, EngineProtocolError, validated_mutation_result

    response = EngineClient().call(
        "work.create", root=root,
        params={"title": title, "date": today.isoformat(), "status": status},
    )
    result = validated_mutation_result(response)
    paths = result["changed_paths"]
    record = next((item for item in paths if isinstance(item, str) and item.endswith("/work-item.json")), None)
    if record is None:
        raise EngineProtocolError("Rust engine work.create omitted the created work-item path")
    relative = Path(record)
    if relative.is_absolute() or ".." in relative.parts:
        raise EngineProtocolError("Rust engine work.create returned a path outside the repository")
    return root.resolve() / relative


def new_markdown(kind: str, title: str, today: date, root: Path = ROOT) -> Path:
    from .engine_client import EngineClient, write_dashboard_canonically
    EngineClient().check_compatibility("validate", "dashboard.write")
    slug = slugify(title)
    if kind == "decision":
        directory, width, template = root / "decisions", 4, "decision.md"
    else:
        directory, width, template = root / "governance" / "policies", 3, "policy.md"
    record_id = _next_numeric(list(directory.glob("*.md")), width, 1)
    text = _template_text(root, template)
    text = text.replace("NNNN", record_id).replace("NNN", record_id)
    text = text.replace("YYYY-MM-DD", today.isoformat())
    text = text.replace("Decision Title", title).replace("Policy Title", title)
    path = directory / f"{record_id}-{slug}.md"
    path.write_text(text, encoding="utf-8")
    write_dashboard_canonically(root)
    return path


def main() -> int:
    parser = argparse.ArgumentParser(description="Stamp a new record from a template")
    parser.add_argument("kind", choices=["work-item", "decision", "policy"])
    parser.add_argument("title")
    parser.add_argument("--status", choices=STATUSES, default="active",
                        help="Lifecycle status for a new work item")
    args = parser.parse_args()
    today = date.today()
    from .engine_client import EngineClient, EngineError, render_validation
    try:
        if args.kind == "work-item":
            path = new_work_item(args.title, today, status=args.status)
        else:
            path = new_markdown(args.kind, args.title, today)
        result = render_validation(EngineClient().call("validate", root=ROOT))
    except EngineError as exc:
        print(f"Rust engine compatibility error: {exc}", file=sys.stderr)
        return 1
    if result:
        return result
    print(f"Created {path.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
