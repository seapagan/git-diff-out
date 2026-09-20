# Copyright (c) 2026 Grant Ramsay
"""Regression tests for the optional local complexity checker."""

from __future__ import annotations

import importlib.util
import os
import sys
import tempfile
import unittest
from pathlib import Path
from typing import TYPE_CHECKING, cast

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


def _file_metrics(output: str, expected_files: set[str]) -> dict[str, int]:
    parser = CHECKER.__dict__.get("_file_metrics")
    if parser is None:
        message = "checker lacks _file_metrics"
        raise AssertionError(message)
    return cast("dict[str, int]", parser(output, expected_files))


def _expect_file_metrics_error(
    output: str,
    expected_files: set[str],
    expected_message: str,
) -> None:
    checker_error = CHECKER.__dict__["CheckerError"]
    try:
        _file_metrics(output, expected_files)
    except checker_error as error:
        if expected_message not in str(error):
            message = f"expected {expected_message!r} in {error!r}"
            raise AssertionError(message) from error
    else:
        message = f"expected CheckerError containing {expected_message!r}"
        raise AssertionError(message)


VALID_XML = """\
<root><measure type="File"><labels><label>NCSS</label><label>CCN</label></labels>
<item name="src/main.rs"><value>7</value><value>2</value></item>
</measure></root>
"""

INVALID_XML_CASES = (
    ("<root>", "invalid Lizard XML:"),
    ("<root />", "expected one XML File measure, found 0"),
    (
        '<root><measure type="File"/><measure type="File"/></root>',
        "expected one XML File measure, found 2",
    ),
    (
        (
            '<root><measure type="File"><labels><label>CCN</label></labels>'
            "</measure></root>"
        ),
        "XML File measure lacks NCSS: ['CCN']",
    ),
    (
        (
            '<root><measure type="File"><labels><label>NCSS</label><label>CCN</label>'
            '</labels><item name="src/main.rs"><value>7</value></item></measure></root>'
        ),
        "unexpected XML file record for 'src/main.rs'",
    ),
    (
        (
            '<root><measure type="File"><labels><label>NCSS</label></labels>'
            '<item name="src/main.rs"><value>7</value></item>'
            '<item name="src/main.rs"><value>8</value></item></measure></root>'
        ),
        "unexpected XML file record for 'src/main.rs'",
    ),
    (
        (
            '<root><measure type="File"><labels><label>NCSS</label></labels>'
            '<item name="src/main.rs"><value>many</value></item></measure></root>'
        ),
        "invalid XML NCSS value for 'src/main.rs': 'many'",
    ),
    (
        (
            '<root><measure type="File"><labels><label>NCSS</label></labels>'
            '<item name="src/main.rs"><value>-1</value></item></measure></root>'
        ),
        "invalid XML NCSS value for 'src/main.rs': '-1'",
    ),
)


class ComplexityCheckerTests(unittest.TestCase):
    """Exercise source selection, analyzer commands, and XML validation."""

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

    def test_file_metrics_parses_lizard_xml(self) -> None:
        """Valid Lizard XML produces normalized per-file NCSS values."""
        _expect_equal(_file_metrics(VALID_XML, {"src/main.rs"}), {"src/main.rs": 7})

    def test_file_metrics_rejects_malformed_xml_records(self) -> None:
        """Malformed XML structures and values remain fatal."""
        for output, expected_message in INVALID_XML_CASES:
            with self.subTest(expected_message=expected_message):
                _expect_file_metrics_error(output, {"src/main.rs"}, expected_message)

    def test_file_metrics_reports_missing_and_unexpected_files(self) -> None:
        """Source-set mismatch diagnostics identify both differences."""
        _expect_file_metrics_error(
            VALID_XML,
            {"src/lib.rs"},
            "source analysis file mismatch; missing=['src/lib.rs'], "
            "unexpected=['src/main.rs']",
        )


if __name__ == "__main__":
    unittest.main()
