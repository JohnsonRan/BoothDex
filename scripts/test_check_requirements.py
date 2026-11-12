#!/usr/bin/env python3
"""Tests for check-requirements.py. Run: python3 scripts/test_check_requirements.py

Each test plants one violation in a clean file and expects the check to name it.
"""
import datetime
import importlib.util
import os
import subprocess
import sys
import tempfile
import unittest

SCRIPT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "check-requirements.py")
spec = importlib.util.spec_from_file_location("check_requirements", SCRIPT)
cr = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cr)

TODAY = datetime.date(2026, 11, 12)
FILES = {
    "AGENTS.md": "### Workflow\n",
    "server/AGENTS.md": "- **Endpoints.** [rule: authz]\n```text\n[rule: in-a-fence]\n```\n",
    "node_modules/pkg/AGENTS.md": "[rule: vendored]\n",
    "scripts/check.sh": "",
}

GOOD = """\
# REQUIREMENTS.md

## Commands

| Tier | Command | Runs (where, when) | Wall-time limit | Baseline (date, host, load) |
| --- | --- | --- | --- | --- |
| Merge gate | `scripts/check.sh --strict` | CI, every merge | 15 min | 11 min (2026-11-12, CI, load 1.2) |

## Numbers

| Item | Limit | Baseline (date, host, load) | Enforced by |
| --- | --- | --- | --- |
| Slowest test package | 16 s | 12 s (2026-11-11, CI, load not recorded) | review |
| Context limit | | | orchestrator |

## Enforcement

| Rule | Gate command | Review criterion |
| --- | --- | --- |
| server/AGENTS.md#authz | `scripts/check.sh authz` | `REVIEW.md`, "Authorization" |
| AGENTS.md: Workflow | | `REVIEW.md`, "Pass bar" |

## Not yet enforced

| Row | Current state | Plan | Due |
| --- | --- | --- | --- |
| Numbers: Context limit | not set | set it after the next long job | 2026-12-31 |

```text
| Numbers: Not a row | ignored | inside a fence | 2000-01-01 |
```
"""


class CheckTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = self.tmp.name
        for rel, body in FILES.items():
            self.write(rel, body)

    def tearDown(self):
        self.tmp.cleanup()

    def write(self, rel, body):
        path = os.path.join(self.root, rel)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(body)

    def problems(self, text):
        return cr.check(text, TODAY, self.root)

    def assertNamed(self, text, fragment):
        found = self.problems(text)
        self.assertTrue(any(fragment in p for p in found), f"{fragment!r} not in {found}")

    def run_cli(self, text):
        self.write("REQUIREMENTS.md", text)
        path = os.path.join(self.root, "REQUIREMENTS.md")
        args = [sys.executable, SCRIPT, "--today", TODAY.isoformat(), path]
        return subprocess.run(args, capture_output=True, text=True).returncode

    def test_clean_file_passes(self):
        self.assertEqual(self.problems(GOOD), [])
        self.assertEqual(self.run_cli(GOOD), 0)

    def test_violation_exits_1(self):
        self.assertEqual(self.run_cli(GOOD.replace("| 16 s |", "| TBD |")), 1)

    def test_unfilled_number(self):
        self.assertNamed(GOOD.replace("| 16 s |", "| TBD |"), "Numbers: Slowest test package: empty Limit")

    def test_unfilled_command(self):
        self.assertNamed(GOOD.replace("| 15 min |", "| |"), "Commands: Merge gate: empty Wall-time limit")

    def test_enforcement_row_without_gate_or_review(self):
        text = GOOD.replace('| | `REVIEW.md`, "Pass bar" |', "| | |")
        self.assertNamed(text, "Enforcement: AGENTS.md: Workflow: empty Gate command, Review criterion")

    def test_deferred_row_that_is_filled(self):
        self.assertNamed(GOOD.replace("| Context limit | | |", "| Context limit | 60% | a setting |"), "is filled; remove it")

    def test_due_date_passed(self):
        self.assertNamed(GOOD.replace("2026-12-31", "2026-11-11"), "Due 2026-11-11 has passed")

    def test_due_not_a_date(self):
        self.assertNamed(GOOD.replace("2026-12-31", "next month"), "Due must be one date")

    def test_deferral_without_plan(self):
        self.assertNamed(GOOD.replace("set it after the next long job", "TODO"), "state its current state and plan")

    def test_deferral_naming_no_row(self):
        self.assertNamed(GOOD.replace("| Numbers: Context limit |", "| Numbers: Context |"), "names no row")

    def test_tagged_rule_without_row(self):
        self.write("web/AGENTS.md", "- **Keys.** [rule: secrets]\n")
        self.assertNamed(GOOD, "Enforcement: web/AGENTS.md#secrets: no row")

    def test_row_without_tag(self):
        self.write("server/AGENTS.md", "- **Endpoints.**\n")
        self.assertNamed(GOOD, "Enforcement: server/AGENTS.md#authz: no such [rule: ] tag")

    def test_missing_script(self):
        self.assertNamed(GOOD.replace("`scripts/check.sh authz`", "`scripts/gone.sh`"), "no such path: scripts/gone.sh")

    def test_stray_pipe(self):
        self.assertNamed(GOOD.replace("CI, every merge", "CI | every merge"), "6 cells, the header has 5")

    def test_unfilled_row_hidden_by_a_duplicate(self):
        row = "| Slowest test package | 16 s | 12 s (2026-11-11, CI, load not recorded) | review |\n"
        text = GOOD.replace(row, row.replace("16 s", "TBD") + row)
        self.assertNamed(text, "Numbers: Slowest test package: listed twice")

    def test_expired_deferral_hidden_by_a_duplicate(self):
        row = "| Numbers: Context limit | not set | set it after the next long job | 2026-12-31 |\n"
        text = GOOD.replace(row, row.replace("2026-12-31", "2026-11-11") + row)
        self.assertNamed(text, "Not yet enforced: Numbers: Context limit: listed twice")

    def test_missing_table_exits_2(self):
        self.assertEqual(self.run_cli(GOOD.replace("## Enforcement", "## Gates")), 2)

    def test_missing_column_exits_2(self):
        self.assertEqual(self.run_cli(GOOD.replace("| Review criterion |", "| Review |")), 2)

    def test_no_agents_file_exits_2(self):
        for rel in ("AGENTS.md", "server/AGENTS.md"):
            os.remove(os.path.join(self.root, rel))
        self.assertEqual(self.run_cli(GOOD), 2)

    def test_no_commands_exits_2(self):
        row = next(line for line in GOOD.splitlines() if line.startswith("| Merge gate"))
        self.assertEqual(self.run_cli(GOOD.replace(row + "\n", "")), 2)


if __name__ == "__main__":
    unittest.main()
