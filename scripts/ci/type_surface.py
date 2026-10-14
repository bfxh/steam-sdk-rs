#!/usr/bin/env python3
"""Measure the inherent public surface of every type in ``src/``.

For each *inherent* ``impl Type { .. }`` block (one without ``for Trait``), count the
``pub fn`` / ``pub const fn`` it declares, summed per type name. That is the number
which grows one method at a time until a single type is doing everything, so the CI
gate keeps it under a registered per-type ceiling.

Two parsing rules that came out of getting this wrong before:
  * brace depth is tracked, because a ``pub fn`` inside a nested block (a closure, a
    macro body) is not a method of the type;
  * comment lines are skipped, because a doc comment that mentions ``pub fn`` is not
    a declaration either.
"""
import os
import re
import sys

IMPL_HEADER = re.compile(r"^impl(?:\s*<[^>]*>)?\s+([\w:]+)(?:\s*<[^>]*>)?\s*\{$")
TRAIT_IMPL = re.compile(r"^impl\b[^{]*\sfor\s+[\w:<>]+[^{]*\{$")
PUB_FN = re.compile(r"^\s*pub\s+(?:const\s+)?fn\s+[A-Za-z_]")
# A type with at least this many registered methods is worth a name in the baseline;
# below it, the noise of one-off helpers would make the file unreadable.
MIN_SURFACE = 1
# The ruler itself must not silently find nothing.
MIN_TYPES = 25


def measure(root):
    totals = {}
    for directory, _, files in os.walk(root):
        for name in sorted(files):
            if not name.endswith(".rs"):
                continue
            with open(os.path.join(directory, name), encoding="utf-8", errors="replace") as handle:
                lines = handle.read().splitlines()

            depth = 0
            inside = None  # type name of the inherent impl we are inside
            methods = 0
            for line in lines:
                code = line.strip()
                if code.startswith("//") or code.startswith("///") or code.startswith("//!"):
                    continue
                if depth == 0:
                    header = IMPL_HEADER.match(code)
                    # `impl Foo for Bar {` is a trait implementation: it does not add a
                    # method to Bar's own surface, it satisfies a contract.
                    if header and not TRAIT_IMPL.match(code):
                        inside, methods = header.group(1), 0
                if inside is not None and PUB_FN.match(line):
                    methods += 1

                delta = code.count("{") - code.count("}")
                depth = max(0, depth + delta)
                if inside is not None and depth == 0:
                    if methods >= MIN_SURFACE:
                        totals[inside] = totals.get(inside, 0) + methods
                    inside = None
    return totals


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else "src"
    totals = measure(root)
    if len(totals) < MIN_TYPES:
        print(f"::error measured {len(totals)} types with methods, expected at least "
              f"{MIN_TYPES} -- the ruler is broken", file=sys.stderr)
        return 2
    for type_name in sorted(totals, key=lambda key: (-totals[key], key)):
        print(f"{type_name} {totals[type_name]}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
