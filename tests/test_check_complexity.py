# Copyright (c) 2026 Grant Ramsay
"""Regression tests for the optional local complexity checker."""

from __future__ import annotations

import importlib.util
import os
import sys
import tempfile
import unittest
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from types import ModuleType


def _load_checker() -> ModuleType:
    path = Path(__file__).parents[1] / "scripts" / "check_complexity.py"
    spec = importlib.util.spec_from_file_location("check_complexity", path)
    if spec is None or spec.loader is None:
        message = f"cannot load {path}"
        raise RuntimeError(message)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


CHECKER = _load_checker()


def _expect_equal(actual: object, expected: object) -> None:
    if actual != expected:
        message = f"expected {expected!r}, got {actual!r}"
        raise AssertionError(message)


class ComplexityCheckerTests(unittest.TestCase):
    """Exercise source selection and analyzer command construction."""

    def test_source_files_include_non_ignored_rust_and_python(self) -> None:
        """Git discovery includes both source types and respects ignores."""
        run = CHECKER.__dict__["_run"]
        source_files = CHECKER.__dict__.get("_source_files")
        if source_files is None:
            message = "checker lacks _source_files"
            raise AssertionError(message)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            previous = Path.cwd()
            try:
                os.chdir(root)
                run(["git", "init", "-q"])
                (root / ".gitignore").write_text(".venv/\n__pycache__/\n")
                (root / "tracked.rs").write_text("fn main() {}\n")
                (root / "tracked.py").write_text("pass\n")
                (root / "untracked.rs").write_text("fn helper() {}\n")
                (root / "untracked.py").write_text("pass\n")
                (root / "ignored.txt").write_text("not source\n")
                (root / ".venv").mkdir()
                (root / ".venv" / "ignored.py").write_text("pass\n")
                run(["git", "add", ".gitignore", "tracked.rs", "tracked.py"])
                files = source_files()
            finally:
                os.chdir(previous)

        _expect_equal(
            files,
            ["tracked.py", "tracked.rs", "untracked.py", "untracked.rs"],
        )

    def test_lizard_command_selects_rust_and_python_once(self) -> None:
        """One command explicitly selects both supported languages."""
        lizard_command = CHECKER.__dict__.get("_lizard_command")
        if lizard_command is None:
            message = "checker lacks _lizard_command"
            raise AssertionError(message)
        _expect_equal(
            lizard_command(
                ["-V", "--csv"],
                ["scripts/check_complexity.py", "src/main.rs"],
            ),
            [
                "lizard",
                "-l",
                "rust",
                "-l",
                "python",
                "-i",
                "-1",
                "-V",
                "--csv",
                "scripts/check_complexity.py",
                "src/main.rs",
            ],
        )


if __name__ == "__main__":
    unittest.main()
