#!/usr/bin/env python3
"""Check Markdown links and image targets without third-party dependencies.

Run from a documentation repository with ``python scripts/validate_docs.py``.
Pass ``--repo OWNER/REPO=PATH`` for each neighboring checkout whose public
GitHub links should be checked against local files, for example:

    python scripts/validate_docs.py --root . \
      --repo ForgeWireLabs/repopact-workbench ../repopact-workbench

External links without a supplied local repository mapping are listed as
unchecked; this tool does not make network requests.
"""

from __future__ import annotations

import argparse
import hashlib
import html
import json
import re
import sys
from pathlib import Path, PurePosixPath
from urllib.parse import unquote, urlsplit

SKIP_DIRS = {".git", ".venv", "node_modules", "target", "dist", "build"}
INLINE_LINK = re.compile(r"!?\[[^\]]*\]\(([^\n]*?)\)")
REFERENCE_LINK = re.compile(r"!?\[([^\]]*)\](?:\[([^\]]*)\])?")
REFERENCE_DEF = re.compile(r"^\s{0,3}\[([^\]]+)\]:\s*(?:<([^>]+)>|(\S+))", re.M)
HTML_TARGET = re.compile(r"\b(?:href|src)\s*=\s*['\"]([^'\"]+)['\"]", re.I)
HEADING = re.compile(r"^#{1,6}\s+(.+?)\s*#*\s*$")


def _slug(text: str) -> str:
    text = re.sub(r"`+(.+?)`+", r"\1", text)
    text = re.sub(r"\*\*(.+?)\*\*|__(.+?)__", lambda m: m.group(1) or m.group(2), text)
    text = re.sub(r"(?<!\w)_([^_]+)_(?!\w)|(?<!\w)\*([^*]+)\*(?!\w)",
                  lambda m: m.group(1) or m.group(2), text)
    text = re.sub(r"~~(.+?)~~", r"\1", text)
    text = re.sub(r"!?\[([^\]]+)\]\([^)]*\)", r"\1", text)
    text = re.sub(r"<[^>]+>", "", text)
    text = html.unescape(text).strip().lower()
    # GitHub replaces whitespace before dropping punctuation, so punctuation
    # surrounded by spaces can leave a doubled dash in the final fragment.
    text = re.sub(r"\s+", "-", text)
    return re.sub(r"[^\w\-]", "", text, flags=re.UNICODE)


def _headings(path: Path) -> set[str]:
    result: set[str] = set()
    counts: dict[str, int] = {}
    in_fence = False
    for line in path.read_text(encoding="utf-8-sig").splitlines():
        if re.match(r"^\s{0,3}(```|~~~)", line):
            in_fence = not in_fence
            continue
        if in_fence:
            continue
        match = HEADING.match(line)
        if not match:
            continue
        base = _slug(match.group(1))
        occurrence = counts.get(base, 0)
        counts[base] = occurrence + 1
        result.add(base if occurrence == 0 else f"{base}-{occurrence}")
    return result


def _target_tokens(markdown: str) -> list[str]:
    # Links inside code are examples or syntax, not navigational targets.
    markdown = re.sub(r"(?ms)^\s{0,3}(```|~~~).*?^\s{0,3}\1[^\n]*$", "", markdown)
    markdown = re.sub(r"(`+)(.+?)\1", "", markdown)
    definitions = {
        (label or "").strip().lower(): (angle or bare or "").strip()
        for label, angle, bare in REFERENCE_DEF.findall(markdown)
    }
    targets = []
    for raw in INLINE_LINK.findall(markdown):
        value = raw.strip()
        if value.startswith("<") and ">" in value:
            value = value[1:value.index(">")]
        else:
            value = value.split(None, 1)[0] if value else ""
        if value:
            targets.append(value)
    for match in REFERENCE_LINK.finditer(markdown):
        label, explicit = match.groups()
        if match.start() > 0 and markdown[match.start() - 1] == "!":
            pass
        if "(" in match.group(0):
            continue
        key = (explicit or label or "").strip().lower()
        if key in definitions:
            targets.append(definitions[key])
    targets.extend(HTML_TARGET.findall(markdown))
    return targets


def _case_sensitive_path(root: Path, relative: str) -> Path | None:
    current = root
    for part in PurePosixPath(relative).parts:
        if part in ("", "."):
            continue
        if part == "..":
            current = current.parent
            continue
        try:
            names = {child.name for child in current.iterdir()}
        except OSError:
            return None
        if part not in names:
            return None
        current = current / part
    return current if current.exists() else None


def _repo_url_target(url: str, repos: dict[str, Path]) -> tuple[Path, str] | None:
    parsed = urlsplit(url)
    if parsed.netloc.lower() != "github.com":
        return None
    parts = [unquote(part) for part in parsed.path.strip("/").split("/")]
    if len(parts) < 5 or parts[2] not in {"blob", "tree"}:
        return None
    repo_key = f"{parts[0]}/{parts[1]}".lower()
    root = repos.get(repo_key)
    if root is None:
        return None
    # GitHub blob/tree routes contain a ref immediately after the route kind.
    rel = "/".join(parts[4:])
    return root, rel


def _parse_repo_mapping(raw: str) -> tuple[str, Path]:
    if "=" not in raw:
        raise argparse.ArgumentTypeError("expected OWNER/REPO=PATH")
    name, path = raw.split("=", 1)
    if name.count("/") != 1:
        raise argparse.ArgumentTypeError("repository name must be OWNER/REPO")
    return name.lower(), Path(path).resolve()


def _markdown_documents(root: Path) -> list[Path]:
    return sorted(
        path for path in root.rglob("*.md")
        if not any(part in SKIP_DIRS for part in path.relative_to(root).parts)
    )


def validate(root: Path, repos: dict[str, Path]) -> tuple[list[str], int]:
    errors: list[str] = []
    unchecked_external = 0
    documents = _markdown_documents(root)
    heading_cache: dict[Path, set[str]] = {}
    for document in documents:
        try:
            text = document.read_text(encoding="utf-8-sig")
        except (OSError, UnicodeError) as exc:
            errors.append(f"{document}: cannot read Markdown: {exc}")
            continue
        for target in _target_tokens(text):
            parsed = urlsplit(target)
            if parsed.scheme or target.startswith("//"):
                mapped = _repo_url_target(target, repos)
                if mapped is None:
                    if parsed.scheme in {"http", "https"} and parsed.netloc.lower() == "github.com":
                        unchecked_external += 1
                    continue
                target_root, relative = mapped
                destination = _case_sensitive_path(target_root, relative)
                fragment = unquote(parsed.fragment)
            else:
                relative = unquote(parsed.path)
                destination = _case_sensitive_path(document.parent, relative)
                fragment = unquote(parsed.fragment)
                target_root = root
            if destination is None:
                errors.append(f"{document.relative_to(root)}: missing or wrong-case target: {target}")
                continue
            if fragment and destination.is_file() and destination.suffix.lower() in {".md", ".markdown"}:
                if destination not in heading_cache:
                    try:
                        heading_cache[destination] = _headings(destination)
                    except (OSError, UnicodeError) as exc:
                        errors.append(f"{destination}: cannot read linked headings: {exc}")
                        continue
                if fragment not in heading_cache[destination]:
                    errors.append(f"{document.relative_to(root)}: missing heading #{fragment} in {target}")
    errors.extend(_validate_asset_manifests(root))
    return errors, unchecked_external


def _validate_asset_manifests(root: Path) -> list[str]:
    errors: list[str] = []
    names = {"asset-inventory.json", "visual-asset-inventory.json"}
    manifests = [path for path in root.rglob("*.json")
                 if path.name in names and not any(part in SKIP_DIRS for part in path.relative_to(root).parts)]
    checked: set[tuple[Path, str | None, str | None, int | None, tuple[object, ...] | None]] = set()

    def check_asset(manifest: Path, raw_path: object, expected_hash: object = None,
                    expected_bytes: object = None, expected_size: object = None) -> None:
        if not isinstance(raw_path, str) or not raw_path:
            return
        size_key = tuple(expected_size) if isinstance(expected_size, list) else None
        key = (manifest, raw_path, expected_hash if isinstance(expected_hash, str) else None,
               expected_bytes if isinstance(expected_bytes, int) else None, size_key)
        if key in checked:
            return
        checked.add(key)
        asset = _case_sensitive_path(root, raw_path)
        label = f"{manifest.relative_to(root)}: asset {raw_path}"
        if asset is None or not asset.is_file():
            errors.append(f"{label}: missing or wrong-case path")
            return
        data = asset.read_bytes()
        if isinstance(expected_hash, str) and expected_hash:
            actual_hash = hashlib.sha256(data).hexdigest()
            if actual_hash.lower() != expected_hash.lower():
                errors.append(f"{label}: SHA-256 mismatch (expected {expected_hash}, got {actual_hash})")
        if isinstance(expected_bytes, int) and len(data) != expected_bytes:
            errors.append(f"{label}: byte count mismatch (expected {expected_bytes}, got {len(data)})")
        if isinstance(expected_size, list) and len(expected_size) == 2 and asset.suffix.lower() == ".png":
            if len(data) < 24 or data[:8] != b"\x89PNG\r\n\x1a\n":
                errors.append(f"{label}: invalid PNG header")
            else:
                actual_size = [int.from_bytes(data[16:20], "big"), int.from_bytes(data[20:24], "big")]
                if actual_size != expected_size:
                    errors.append(f"{label}: dimensions mismatch (expected {expected_size}, got {actual_size})")

    def walk(manifest: Path, value: object) -> None:
        if isinstance(value, dict):
            if "path" in value:
                check_asset(manifest, value.get("path"), value.get("sha256"), value.get("bytes"))
            if "png" in value:
                check_asset(manifest, value.get("png"), value.get("sha256"), expected_size=value.get("size"))
            if "svg" in value:
                check_asset(manifest, value.get("svg"))
            for path_key, hash_key in (("screenshot_source", "screenshot_source_sha256"),
                                       ("cropped_screenshot", "cropped_screenshot_sha256")):
                if path_key in value:
                    check_asset(manifest, value.get(path_key), value.get(hash_key))
            if "readme_crop" in value and isinstance(value["readme_crop"], dict):
                crop = value["readme_crop"]
                check_asset(manifest, crop.get("path"), crop.get("sha256"))
                check_asset(manifest, crop.get("source"), crop.get("source_sha256"))
            for item in value.values():
                walk(manifest, item)
        elif isinstance(value, list):
            for item in value:
                walk(manifest, item)

    for manifest in manifests:
        try:
            content = json.loads(manifest.read_text(encoding="utf-8-sig"))
        except (OSError, UnicodeError, json.JSONDecodeError) as exc:
            errors.append(f"{manifest.relative_to(root)}: cannot read asset inventory: {exc}")
            continue
        walk(manifest, content)
        if isinstance(content, dict):
            social_preview = content.get("social_preview")
            if isinstance(social_preview, dict) and "png" in social_preview:
                check_asset(manifest, social_preview.get("png"), social_preview.get("sha256"),
                            expected_size=social_preview.get("size"))
            if "dimensions" in content and isinstance(content.get("assets"), list):
                for asset in content["assets"]:
                    if isinstance(asset, dict) and str(asset.get("path", "")).startswith("social/"):
                        check_asset(manifest, asset.get("path"), asset.get("sha256"), asset.get("bytes"),
                                    expected_size=content.get("dimensions") if str(asset.get("path", "")).lower().endswith(".png") else None)
    return errors


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path.cwd(), help="documentation repository root")
    parser.add_argument("--repo", action="append", type=_parse_repo_mapping, default=[], metavar="OWNER/REPO=PATH",
                        help="local checkout for verifying GitHub blob/tree links; repeat as needed")
    args = parser.parse_args(argv)
    root = args.root.resolve()
    if not root.is_dir():
        parser.error(f"not a directory: {root}")
    repos = dict(args.repo)
    errors, unchecked = validate(root, repos)
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        print(f"Documentation links: {len(errors)} error(s), {unchecked} external GitHub link(s) unchecked")
        return 1
    print(f"Documentation links: PASS ({len(_markdown_documents(root))} Markdown source files); {unchecked} external GitHub link(s) unchecked")
    return 0


if __name__ == "__main__":
    sys.exit(main())
