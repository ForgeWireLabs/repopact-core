import tempfile
import unittest
from pathlib import Path

from scripts.validate_docs import _markdown_documents, validate


class ValidateDocsTests(unittest.TestCase):
    def test_document_count_excludes_build_and_dependency_trees(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "README.md").write_text("# Project\n", encoding="utf-8")
            (root / "node_modules" / "package").mkdir(parents=True)
            (root / "node_modules" / "package" / "README.md").write_text(
                "# Dependency\n", encoding="utf-8"
            )
            self.assertEqual(_markdown_documents(root), [root / "README.md"])

    def test_resolves_relative_assets_and_heading_anchors(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "docs").mkdir()
            (root / "docs" / "guide.md").write_text("# First Use\n", encoding="utf-8")
            (root / "docs" / "image.svg").write_text("<svg />", encoding="utf-8")
            (root / "README.md").write_text(
                "[Guide](docs/guide.md#first-use) ![Image](docs/image.svg)\n",
                encoding="utf-8",
            )
            errors, _ = validate(root, {})
            self.assertEqual(errors, [])

    def test_keeps_underscores_inside_inline_code_heading(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "findings.md").write_text(
                "## F-017 — `source_of_truth` pointer\n", encoding="utf-8"
            )
            (root / "capture.md").write_text(
                "[Finding](findings.md#f-017--source_of_truth-pointer)\n",
                encoding="utf-8",
            )
            errors, _ = validate(root, {})
            self.assertEqual(errors, [])

    def test_reports_missing_case_sensitive_target_and_anchor(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "docs").mkdir()
            (root / "docs" / "guide.md").write_text("# Present\n", encoding="utf-8")
            (root / "README.md").write_text(
                "[Bad case](docs/Guide.md) [Bad anchor](docs/guide.md#absent)\n",
                encoding="utf-8",
            )
            errors, _ = validate(root, {})
            self.assertEqual(len(errors), 2)

    def test_checks_mapped_cross_repository_link(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            core = root / "core"
            workbench = root / "workbench"
            core.mkdir()
            workbench.mkdir()
            (workbench / "docs").mkdir()
            (workbench / "docs" / "setup.md").write_text("# Setup\n", encoding="utf-8")
            (core / "README.md").write_text(
                "[Setup](https://github.com/ForgeWireLabs/repopact-workbench/blob/main/docs/setup.md#setup)\n",
                encoding="utf-8",
            )
            errors, _ = validate(core, {"forgewirelabs/repopact-workbench": workbench})
            self.assertEqual(errors, [])

    def test_validates_asset_hash_size_and_dimensions(self):
        import hashlib
        import json

        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            image = root / "card.png"
            header = b"\x89PNG\r\n\x1a\n" + b"\x00" * 8
            image.write_bytes(header + (1280).to_bytes(4, "big") + (640).to_bytes(4, "big") + b"data")
            inventory = {
                "social_preview": {
                    "png": "card.png",
                    "size": [1280, 640],
                    "sha256": hashlib.sha256(image.read_bytes()).hexdigest(),
                }
            }
            (root / "visual-asset-inventory.json").write_text(json.dumps(inventory), encoding="utf-8")
            errors, _ = validate(root, {})
            self.assertEqual(errors, [])


if __name__ == "__main__":
    unittest.main()
