#!/usr/bin/env python3
"""Measure the size of every Rust module in ``src/``.

One line per file: ``<path-with-<>-separators> <lines>``. Paired with
``type_surface.py`` this is the guard against a single file accreting until it is the
only place anybody can add code -- the failure mode where a new feature is dropped
into whatever giant file already exists because opening a new module feels expensive.
"""
import os
import sys

# Floor on the number of modules: a parse that sees almost none is broken, not clean.
MIN_FILES = 20


def main(root):
    rows = []
    for directory, _, files in os.walk(root):
        for name in sorted(files):
            if not name.endswith(".rs"):
                continue
            path = os.path.join(directory, name).replace(os.sep, "/")
            with open(path, encoding="utf-8", errors="replace") as handle:
                rows.append((path, sum(1 for _ in handle)))
    if len(rows) < MIN_FILES:
        print(f"::error measured {len(rows)} modules, expected at least {MIN_FILES} "
              f"-- the ruler is broken", file=sys.stderr)
        return 2
    for path, lines in sorted(rows, key=lambda row: (-row[1], row[0])):
        print(f"{path} {lines}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1] if len(sys.argv) > 1 else "src"))
