"""Focused incremental build and hostile Git-environment regressions."""
from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import time
import unittest
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
BUILD_SUPPORT = REPO_ROOT / "rust" / "apps" / "repopact-engine" / "build_support.rs"


@unittest.skipUnless(shutil.which("cargo") and shutil.which("git"), "Cargo and Git required")
class EngineBuildProvenanceTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temp = tempfile.TemporaryDirectory(prefix="repopact-build-provenance-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.source = self.root / "source"
        self.source.mkdir()
        self.git(self.source, "init", "-q", "--initial-branch=main")
        self.git(self.source, "config", "user.email", "build@example.invalid")
        self.git(self.source, "config", "user.name", "Build Fixture")
        (self.source / "tracked.txt").write_text("one\n", encoding="utf-8")
        self.git(self.source, "add", "tracked.txt")
        self.git(self.source, "commit", "-qm", "initial")
        self.probe = self.root / "probe"
        (self.probe / "src").mkdir(parents=True)
        (self.probe / "Cargo.toml").write_text(
            '[package]\nname = "git-provenance-probe"\nversion = "0.1.0"\nedition = "2021"\n\n'
            "[build-dependencies]\n",
            encoding="utf-8",
        )
        support_path = BUILD_SUPPORT.as_posix()
        (self.probe / "build.rs").write_text(
            f'''use std::fs;
use std::path::PathBuf;

#[path = "{support_path}"]
mod build_support;

fn main() {{
    println!("cargo:rerun-if-env-changed=PROBE_ROOT");
    println!("cargo:rerun-if-env-changed=PROBE_RUNS_FILE");
    println!("cargo:rerun-if-env-changed=PROBE_INPUTS_FILE");
    let root = PathBuf::from(std::env::var_os("PROBE_ROOT").unwrap());
    let inputs = build_support::git_revision_inputs(&root);
    for input in &inputs {{
        println!("cargo:rerun-if-changed={{}}", input.display());
    }}
    fs::write(
        std::env::var_os("PROBE_INPUTS_FILE").unwrap(),
        inputs.iter().map(|path| path.display().to_string()).collect::<Vec<_>>().join("\\n"),
    ).unwrap();
    let runs_file = PathBuf::from(std::env::var_os("PROBE_RUNS_FILE").unwrap());
    let previous = fs::read_to_string(&runs_file).ok()
        .and_then(|value| value.trim().parse::<u32>().ok()).unwrap_or(0);
    fs::write(runs_file, (previous + 1).to_string()).unwrap();
    let revision = build_support::git_revision(&root).expect("fixture revision must resolve");
    println!("cargo:rustc-env=PROBE_REV={{revision}}");
}}
''',
            encoding="utf-8",
        )
        (self.probe / "src" / "main.rs").write_text(
            'fn main() { println!("{}", env!("PROBE_REV")); }\n',
            encoding="utf-8",
        )
        self.runs_file = self.root / "build-runs.txt"
        self.inputs_file = self.root / "build-inputs.txt"
        self.runs_file.write_text("0", encoding="utf-8")

    @staticmethod
    def clean_git_env() -> dict[str, str]:
        env = {
            key: value
            for key, value in os.environ.items()
            if not key.upper().startswith("GIT_") and key.upper() != "CARGO_TARGET_DIR"
        }
        return env

    def git(self, cwd: Path, *args: str) -> str:
        return subprocess.check_output(
            ["git", *args], cwd=cwd, env=self.clean_git_env(), text=True
        ).strip()

    def revision(self, cwd: Path) -> str:
        return self.git(cwd, "rev-parse", "--verify", "HEAD")

    def hostile_env(self, source: Path) -> dict[str, str]:
        env = self.clean_git_env()
        env.update(
            {
                "PROBE_ROOT": str(source),
                "PROBE_RUNS_FILE": str(self.runs_file),
                "PROBE_INPUTS_FILE": str(self.inputs_file),
                "GIT_DIR": str(self.root / "missing-git-dir"),
                "GIT_WORK_TREE": str(self.root / "redirected-worktree"),
                "GIT_INDEX_FILE": str(self.root / "redirected.index"),
                "GIT_CONFIG_COUNT": "1",
                "GIT_CONFIG_KEY_0": "core.worktree",
                "GIT_CONFIG_VALUE_0": "",
            }
        )
        return env

    def build(self, source: Path) -> str:
        subprocess.run(
            ["cargo", "build", "--offline", "--quiet", "--manifest-path", str(self.probe / "Cargo.toml")],
            cwd=self.probe,
            env=self.hostile_env(source),
            check=True,
            capture_output=True,
            text=True,
            timeout=180,
        )
        exe = self.probe / "target" / "debug" / (
            "git-provenance-probe.exe" if os.name == "nt" else "git-provenance-probe"
        )
        return subprocess.check_output([str(exe)], text=True).strip()

    def runs(self) -> int:
        return int(self.runs_file.read_text(encoding="utf-8"))

    def test_revision_refresh_noop_and_hostile_git_overrides(self):
        self.assertFalse((self.source / ".git" / "packed-refs").exists())
        initial = self.revision(self.source)
        self.assertEqual(initial, self.build(self.source))
        self.assertEqual(1, self.runs())
        self.assertNotIn("packed-refs", self.inputs_file.read_text(encoding="utf-8"))

        started = time.perf_counter()
        self.assertEqual(initial, self.build(self.source))
        no_op_ms = (time.perf_counter() - started) * 1000
        self.assertEqual(1, self.runs(), "no-op Cargo build reran the build script")
        print(f"Cargo no-op build with no packed-refs input: {no_op_ms:.1f} ms")

        (self.source / "tracked.txt").write_text("two\n", encoding="utf-8")
        self.git(self.source, "add", "tracked.txt")
        self.git(self.source, "commit", "-qm", "advance loose branch")
        loose_revision = self.revision(self.source)
        self.assertNotEqual(initial, loose_revision)
        self.assertEqual(loose_revision, self.build(self.source))
        self.assertEqual(2, self.runs())

        self.git(self.source, "pack-refs", "--all", "--prune")
        packed_refs = self.source / ".git" / "packed-refs"
        self.assertTrue(packed_refs.is_file())
        self.assertEqual(loose_revision, self.build(self.source))
        self.assertEqual(3, self.runs())
        self.assertIn("packed-refs", self.inputs_file.read_text(encoding="utf-8"))

        (self.source / "tracked.txt").write_text("three\n", encoding="utf-8")
        self.git(self.source, "add", "tracked.txt")
        self.git(self.source, "commit", "-qm", "advance packed branch")
        packed_advance = self.revision(self.source)
        self.assertNotEqual(loose_revision, packed_advance)
        self.assertEqual(packed_advance, self.build(self.source))
        self.assertEqual(4, self.runs())

        detached = self.root / "linked-detached"
        self.git(self.source, "worktree", "add", "--detach", str(detached), packed_advance)
        self.assertEqual(packed_advance, self.build(detached))
        self.assertEqual(5, self.runs())
        linked_head = self.git(detached, "rev-parse", "--git-path", "HEAD")
        linked_head_path = Path(linked_head)
        if not linked_head_path.is_absolute():
            linked_head_path = detached / linked_head_path
        self.assertIn(
            str(linked_head_path.resolve()).replace("\\", "/"),
            self.inputs_file.read_text(encoding="utf-8"),
        )

        (detached / "tracked.txt").write_text("detached\n", encoding="utf-8")
        self.git(detached, "add", "tracked.txt")
        self.git(detached, "commit", "-qm", "advance detached head")
        detached_revision = self.revision(detached)
        self.assertNotEqual(packed_advance, detached_revision)
        self.assertEqual(detached_revision, self.build(detached))
        self.assertEqual(6, self.runs())
