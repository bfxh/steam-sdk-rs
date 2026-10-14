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


def main():
    if not os.path.isdir(WORKFLOW_DIR):
        print(f"::error {WORKFLOW_DIR} is missing -- cannot count anything", file=sys.stderr)
        return 2
    rows = []
    total = 0
    for name in sorted(os.listdir(WORKFLOW_DIR)):
        if not name.endswith((".yml", ".yaml")):
            continue
        path = os.path.join(WORKFLOW_DIR, name).replace(os.sep, "/")
        with open(path, encoding="utf-8") as handle:
            text = handle.read()
        count = sum(text.count(pattern) for pattern in PATTERNS)
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
