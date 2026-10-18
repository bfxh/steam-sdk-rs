#!/usr/bin/env python3
"""Count the shell escape hatches in the workflow files.

`|| true` and `2>/dev/null` turn "did not run" into "passed". This repository has the
measured case: an advisory step installed three tools with `|| true`, the tools were never
present on the runner, and `error: no such command: geiger` was swallowed -- the job was
green from its first run while checking nothing.

Hatches are not banned outright (some are legitimate: capturing a command's output when the
command may exit non-zero and judging the output separately). They are counted, and the count
may only go down: a new one is red, and removing one is red until the number is re-recorded,
so a baseline can never drift looser than reality.
"""
import os
import sys

WORKFLOW_DIR = ".github/workflows"
PATTERNS = ("|| true", "||:", "2>/dev/null", "1>/dev/null 2>&1 || true", "--no-verify")


def count_hatches(text):
    """Count pattern occurrences on lines that are code, not prose.

    Measured 2026-10-18: of the 9 hatches registered then, **4 were comment text** -- this file's
    own docstring quoting ``|| true`` to explain why it is bad, and ``release.yml`` and
    ``quality-extra.yml`` each describing the switch they no longer use. So "the count may only go
    down" was being enforced on vocabulary: rewording a comment moved the ledger, and a hatch
    written out for the reader counted as many times as it was mentioned. Lines whose first
    non-space character is ``#`` are skipped; a hatch still counts when it sits in a trailing
    comment after code, because that line is code the runner executes.
    """
    total = 0
    for line in text.splitlines():
        if line.lstrip().startswith("#"):
            continue
        total += sum(line.count(pattern) for pattern in PATTERNS)
    return total


def main():
    # The optional argument exists for the canary fixtures; CI runs it with none, on the real
    # workflow directory.
    workflow_dir = sys.argv[1] if len(sys.argv) > 1 else WORKFLOW_DIR
    if not os.path.isdir(workflow_dir):
        print(f"::error {workflow_dir} is missing -- cannot count anything", file=sys.stderr)
        return 2
    rows = []
    total = 0
    for name in sorted(os.listdir(workflow_dir)):
        if not name.endswith((".yml", ".yaml")):
            continue
        path = os.path.join(workflow_dir, name).replace(os.sep, "/")
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        count = count_hatches(text)
        rows.append((path, count))
        total += count
    for path, count in rows:
        print(f"{path} {count}")
    # Self-check: a scan that finds nothing in a repo known to have hatches is a broken scan.
    if total == 0:
        print("::error found no escape hatches at all -- either the patterns are wrong or the "
              "files were not read; an empty scan is not a pass", file=sys.stderr)
        return 2
    print(f"# total {total}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
