#!/usr/bin/env python3
"""Fail when REQUIREMENTS.md has an unfilled, expired or dangling entry.

Checked:
- "Commands" and "Numbers": every row fills its columns, or is listed under "Not yet enforced".
- "Enforcement": every rule tagged [rule: <id>] in an AGENTS.md has a row named "path#id", every
  "path#id" row has its tag, and every row names a gate command or a review criterion.
- Every table: as many cells per row as the header, and no two rows with the same first cell.
- "Not yet enforced": each row names an existing row as "Section: row", states its current state
  and plan, and has one Due date (YYYY-MM-DD) that has not passed. A filled row leaves this list.
- In a backticked command under "Commands" or in a gate command, a script or a path into an
  existing top-level directory exists.

A cell that holds only a placeholder (TBD, TODO, n/a, none, ?, -) counts as empty. Write a pipe
inside a cell as \\|. Fenced code blocks are not read.

Exit codes: 0 clean, 1 violations found, 2 could not run (file, table or column missing, no rows
under "Commands", or no AGENTS.md).
Usage: check-requirements.py [--today YYYY-MM-DD] [--root DIR] [path/to/REQUIREMENTS.md]
"""
import argparse
import datetime
import os
import re
import shlex
import sys

FILLED = {
    "Commands": ["Command", "Runs (where, when)", "Wall-time limit", "Baseline (date, host, load)"],
    "Numbers": ["Limit", "Baseline (date, host, load)"],
}
EITHER = ["Gate command", "Review criterion"]
COLUMNS = {**FILLED, "Enforcement": EITHER, "Not yet enforced": ["Current state", "Plan", "Due"]}
PLACEHOLDERS = {"", "tbd", "todo", "n/a", "na", "none", "?", "-", "--", "...", "…"}
SKIP_DIRS = {".git", "node_modules", "target", "vendor", "third_party"}
SCRIPT_SUFFIXES = (".sh", ".py", ".js", ".cjs", ".mjs", ".ts")
RULE_ID = re.compile(r"\[rule: ([a-z0-9][a-z0-9-]*)\]")


class CannotRun(Exception):
    """The input does not have the shape this check reads. Maps to exit code 2."""


def blank(cell):
    return cell.strip("`* ").lower() in PLACEHOLDERS


def unfenced(text):
    """Yield the lines outside fenced code blocks; a fence line itself becomes an empty line."""
    fenced = False
    for line in text.splitlines():
        if line.lstrip().startswith(("```", "~~~")):
            fenced = not fenced
            yield ""
        elif not fenced:
            yield line


def cells(line):
    line = line.strip().removeprefix("|")
    if line.endswith("|") and not line.endswith("\\|"):
        line = line[:-1]
    return [c.strip().replace("\\|", "|") for c in re.split(r"(?<!\\)\|", line)]


def tables(text):
    """Map each level-2 heading to the lines of its first table, each split into cells."""
    out, section, current = {}, None, None
    for line in unfenced(text):
        if line.startswith("## "):
            section, current = line[3:].strip(), None
        elif not line.lstrip().startswith("|"):
            current = None
        elif current is not None:
            current.append(cells(line))
        elif section and section not in out:
            current = out[section] = [cells(line)]
    return out


def agents_tags(root):
    """Return "path#id" for every [rule: <id>] tag in an AGENTS.md under the root."""
    tags, found = set(), False
    for top, dirs, files in os.walk(root):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS and (d == ".github" or not d.startswith("."))]
        if "AGENTS.md" in files:
            found = True
            rel = os.path.relpath(os.path.join(top, "AGENTS.md"), root).replace(os.sep, "/")
            with open(os.path.join(top, "AGENTS.md"), encoding="utf-8") as handle:
                lines = unfenced(handle.read())
            tags.update(f"{rel}#{rule}" for line in lines for rule in RULE_ID.findall(line))
    if not found:
        raise CannotRun("no AGENTS.md under the root")
    return tags


def missing_paths(cell, root):
    """Return the repository paths named in the backticked commands of a cell that do not exist."""
    absent = []
    for command in re.findall(r"`([^`]+)`", cell):
        try:
            words = shlex.split(command)
        except ValueError:
            words = command.split()
        for word in words:
            word = word.removeprefix("./")
            if not word or word[0] in "-/~" or re.search(r"[$*?\[\]{}<>=:;|&()]|\.\.\.", word):
                continue
            scripted = word.endswith(SCRIPT_SUFFIXES)
            in_repo = "/" in word and os.path.exists(os.path.join(root, word.split("/", 1)[0]))
            if (scripted or in_repo) and not os.path.exists(os.path.join(root, word)):
                absent.append(word)
    return absent


def due_date(cell):
    try:
        return datetime.date.fromisoformat(cell) if re.fullmatch(r"\d{4}-\d{2}-\d{2}", cell) else None
    except ValueError:
        return None


def check(text, today, root):
    """Return the list of violations. Raise CannotRun when the input has the wrong shape."""
    found = tables(text)
    problems, rows = [], {}
    for section, wanted in COLUMNS.items():
        table = found.get(section)
        if not table or len(table) < 2 or not all(re.fullmatch(r":?-+:?", c) for c in table[1]):
            raise CannotRun(f"no table with a separator row under: {section}")
        header = table[0]
        absent = [w for w in wanted if w not in header]
        if absent:
            raise CannotRun(f"the {section} table lacks columns: {', '.join(absent)}")
        if section == "Commands" and not table[2:]:
            raise CannotRun("the Commands table has no rows")
        rows[section] = {}
        for row in table[2:]:
            if len(row) != len(header):
                problems.append(
                    f"{section}: {row[0] or '(no name)'}: {len(row)} cells, the header has "
                    f"{len(header)} (write a pipe inside a cell as \\|)"
                )
            elif row[0] in rows[section]:
                problems.append(f"{section}: {row[0]}: listed twice")
            else:
                rows[section][row[0]] = dict(zip(header, row))
    tags = agents_tags(root)

    deferred = set()
    for name, row in rows["Not yet enforced"].items():
        section, _, target = name.partition(": ")
        if section not in ("Commands", "Numbers", "Enforcement") or target not in rows[section]:
            problems.append(f"Not yet enforced: {name}: names no row (write Section: row)")
            continue
        deferred.add((section, target))
        if blank(row["Current state"]) or blank(row["Plan"]):
            problems.append(f"Not yet enforced: {name}: state its current state and plan")
        due = due_date(row["Due"])
        if due is None:
            problems.append(f"Not yet enforced: {name}: Due must be one date, YYYY-MM-DD, not {row['Due']!r}")
        elif due < today:
            problems.append(f"Not yet enforced: {name}: Due {due} has passed (do it, or move it with the owner's approval)")

    for section in ("Commands", "Numbers", "Enforcement"):
        for name, row in rows[section].items():
            if section == "Enforcement":
                empty = EITHER if all(blank(row[c]) for c in EITHER) else []
            else:
                empty = [c for c in FILLED[section] if blank(row[c])]
            if empty and (section, name) not in deferred:
                problems.append(f"{section}: {name}: empty {', '.join(empty)} (fill it or list it under Not yet enforced)")
            elif not empty and (section, name) in deferred:
                problems.append(f"{section}: {name}: is filled; remove it from Not yet enforced")

    named = {name for name in rows["Enforcement"] if "#" in name}
    problems.extend(f"Enforcement: {tag}: no row (every tagged rule needs one)" for tag in sorted(tags - named))
    problems.extend(f"Enforcement: {name}: no such [rule: ] tag (renamed or removed?)" for name in sorted(named - tags))

    for section, column in (("Commands", "Command"), ("Enforcement", "Gate command")):
        for name, row in rows[section].items():
            problems.extend(f"{section}: {name}: no such path: {path}" for path in missing_paths(row[column], root))
    return problems


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("path", nargs="?", default="REQUIREMENTS.md")
    parser.add_argument("--root", help="repository root, where AGENTS.md files and command paths are looked up (default: the file's directory)")
    parser.add_argument(
        "--today",
        type=datetime.date.fromisoformat,
        default=datetime.date.today(),
        help="date to compare Due dates with (YYYY-MM-DD); for tests",
    )
    args = parser.parse_args(argv)
    root = args.root or os.path.dirname(os.path.abspath(args.path))
    try:
        with open(args.path, encoding="utf-8") as handle:
            text = handle.read()
        problems = check(text, args.today, root)
    except (OSError, UnicodeDecodeError, CannotRun) as err:
        print(f"check-requirements: {args.path}: {err}", file=sys.stderr)
        return 2
    for problem in problems:
        print(problem)
    return 1 if problems else 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as err:  # a crash must not exit 1, which means "violations found"
        print(f"check-requirements: internal error: {err!r}", file=sys.stderr)
        sys.exit(2)
