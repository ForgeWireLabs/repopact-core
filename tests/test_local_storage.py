from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace

from repopact.local_storage import (
    LocalStorageError,
    cleanup_owned_target,
    cleanup_owned_worktree,
    create_owned_target,
    create_owned_worktree,
    disk_preflight,
    inventory,
    seal_owned_target,
    tree_usage,
)


class LocalValidationStorageTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="repopact-local-storage-")
        self.addCleanup(self.temp.cleanup)
        self.workspace = Path(self.temp.name)
        self.root = self.workspace / "repo"
        self.root.mkdir()
        (self.root / "rust").mkdir()
        (self.root / "evidence" / "runs").mkdir(parents=True)
        env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
        subprocess.run(["git", "init", "-q"], cwd=self.root, env=env, check=True)
        subprocess.run(["git", "config", "user.email", "fixture@example.invalid"], cwd=self.root, env=env, check=True)
        subprocess.run(["git", "config", "user.name", "Local Storage Fixture"], cwd=self.root, env=env, check=True)
        (self.root / "README.md").write_text("storage fixture\n", encoding="utf-8")
        (self.root / ".gitignore").write_text("ignored-user-file.txt\n", encoding="utf-8")
        subprocess.run(["git", "add", "README.md", ".gitignore"], cwd=self.root, env=env, check=True)
        subprocess.run(["git", "commit", "-qm", "fixture"], cwd=self.root, env=env, check=True)

    def test_disk_preflight_reports_low_free_space_for_external_target(self) -> None:
        calls: list[Path] = []

        def fake_usage(path: Path):
            calls.append(path)
            return SimpleNamespace(total=1000, used=750, free=250)

        target = self.workspace / "external-target"
        result = disk_preflight(
            self.root,
            minimum_free_bytes=500,
            cargo_target_dir=str(target),
            usage_fn=fake_usage,
        )
        self.assertFalse(result["ready"])
        self.assertEqual(250, result["volumes"][0]["free_bytes"])
        self.assertGreaterEqual(len(calls), 1)
        self.assertEqual(str(target), result["cargo_target_dir"])

    def test_inventory_reports_evidence_as_immutable_and_does_not_delete(self) -> None:
        evidence = self.root / "evidence" / "runs" / "historical.json"
        evidence.write_text("{}\n", encoding="utf-8")
        report = inventory(self.root)
        paths = {item["purpose"]: item for item in report["known_paths"]}
        self.assertEqual("never-auto-delete-immutable-evidence", paths["evidence-runs-immutable"]["cleanup_policy"])
        self.assertTrue(evidence.exists())

    def test_inventory_does_not_traverse_symlinked_build_output(self) -> None:
        external = self.workspace / "external-user-data"
        external.mkdir()
        sentinel = external / "keep.txt"
        sentinel.write_bytes(b"user-owned bytes")
        link = self.root / "build"
        try:
            link.symlink_to(external, target_is_directory=True)
        except OSError as exc:
            self.skipTest(f"host cannot create a directory symlink: {exc}")
        report = tree_usage(link)
        self.assertEqual("symlink-or-junction", report["excluded"])
        self.assertEqual(0, report["files"])
        self.assertEqual(0, report["bytes"])
        self.assertEqual(b"user-owned bytes", sentinel.read_bytes())

    def test_owned_target_cleanup_is_dry_run_and_refuses_unowned_content(self) -> None:
        target = self.root / "rust" / "target"
        create_owned_target(self.root, target)
        payload = target / "owned.bin"
        payload.write_bytes(b"owned cargo output")
        seal_owned_target(self.root, target)

        preview = cleanup_owned_target(self.root, target)
        self.assertEqual("would-remove", preview["action"])
        self.assertTrue(payload.exists())

        unknown = target / "operator-notes.txt"
        unknown.write_text("leave this file\n", encoding="utf-8")
        with self.assertRaisesRegex(LocalStorageError, "differ from the sealed ownership inventory"):
            cleanup_owned_target(self.root, target, apply=True)
        self.assertTrue(payload.exists())
        self.assertTrue(unknown.exists())

    def test_owned_target_apply_removes_only_sealed_test_fixture(self) -> None:
        target = self.root / "rust" / "target"
        create_owned_target(self.root, target)
        nested = target / "debug" / "deps"
        nested.mkdir(parents=True)
        (nested / "fixture.rlib").write_bytes(b"test-only cargo artifact")
        seal_owned_target(self.root, target)

        result = cleanup_owned_target(self.root, target, apply=True)
        self.assertEqual("removed", result["action"])
        self.assertFalse(target.exists())

    def test_unregistered_target_is_never_removed(self) -> None:
        target = self.root / "rust" / "user-target"
        target.mkdir()
        sentinel = target / "user.txt"
        sentinel.write_text("user data\n", encoding="utf-8")
        with self.assertRaisesRegex(LocalStorageError, "unregistered target"):
            cleanup_owned_target(self.root, target, apply=True)
        self.assertEqual("user data\n", sentinel.read_text(encoding="utf-8"))

    def test_worktree_cleanup_requires_registered_clean_detached_checkout(self) -> None:
        worktree = self.workspace / "owned-worktrees" / "fixture"
        worktree.parent.mkdir()
        create_owned_worktree(self.root, worktree, "HEAD")
        preview = cleanup_owned_worktree(self.root, worktree)
        self.assertEqual("would-remove", preview["action"])
        self.assertTrue(worktree.exists())

        user_file = worktree / "untracked-user-file.txt"
        user_file.write_text("do not delete\n", encoding="utf-8")
        with self.assertRaisesRegex(LocalStorageError, "user changes"):
            cleanup_owned_worktree(self.root, worktree, apply=True)
        self.assertTrue(user_file.exists())

        user_file.unlink()
        result = cleanup_owned_worktree(self.root, worktree, apply=True)
        self.assertEqual("removed", result["action"])
        self.assertFalse(worktree.exists())

    def test_unregistered_worktree_path_is_never_removed(self) -> None:
        path = self.workspace / "unowned-worktree"
        path.mkdir()
        sentinel = path / "user.txt"
        sentinel.write_text("keep\n", encoding="utf-8")
        with self.assertRaisesRegex(LocalStorageError, "unregistered worktree"):
            cleanup_owned_worktree(self.root, path, apply=True)
        self.assertEqual("keep\n", sentinel.read_text(encoding="utf-8"))

    def test_worktree_cleanup_refuses_ignored_user_files(self) -> None:
        worktree = self.workspace / "owned-worktrees" / "ignored-fixture"
        worktree.parent.mkdir()
        create_owned_worktree(self.root, worktree, "HEAD")
        ignored = worktree / "ignored-user-file.txt"
        ignored.write_text("preserve ignored user data\n", encoding="utf-8")
        with self.assertRaisesRegex(LocalStorageError, "contains ignored files"):
            cleanup_owned_worktree(self.root, worktree, apply=True)
        self.assertEqual("preserve ignored user data\n", ignored.read_text(encoding="utf-8"))
        self.assertTrue(worktree.exists())

    def test_worktree_cleanup_refuses_unbounded_inventory(self) -> None:
        from unittest.mock import patch

        worktree = self.workspace / "owned-worktrees" / "bounded-fixture"
        worktree.parent.mkdir()
        create_owned_worktree(self.root, worktree, "HEAD")
        with patch(
            "repopact.local_storage.tree_usage",
            return_value={"path": str(worktree), "exists": True, "files": 1, "bytes": 1, "truncated": True},
        ):
            with self.assertRaisesRegex(LocalStorageError, "bounded cleanup inventory"):
                cleanup_owned_worktree(self.root, worktree, apply=True)
        self.assertTrue(worktree.exists())


if __name__ == "__main__":
    unittest.main()
