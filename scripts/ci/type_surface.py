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

Macro-generated methods are counted at their **call sites**, which fixed a registered
blind spot: the ruler used to see 4 methods on ``MatchmakingServers`` while the type
really had 9, because ``gen_server_list_fn!`` wrote ``pub fn $name`` -- a ``$`` is not
``[A-Za-z_]``, so the declaration line never matched and only the four hand-written
methods were counted. The rule now is: collect the names of every ``macro_rules!`` whose
body contains a ``pub fn $`` (a *generating* macro), then count each invocation of one
inside an inherent impl as one method. Counting call sites rather than macro bodies is
what keeps the attribution honest -- a macro declared and never expanded adds no surface,
and adding a generated method means writing a new invocation, which is exactly the line a
reviewer has to touch.
"""
import os
import re
import sys

IMPL_HEADER = re.compile(r"^impl(?:\s*<[^>]*>)?\s+([\w:]+)(?:\s*<[^>]*>)?\s*\{$")
TRAIT_IMPL = re.compile(r"^impl\b[^{]*\sfor\s+[\w:<>]+[^{]*\{$")
PUB_FN = re.compile(r"^\s*pub\s+(?:const\s+)?fn\s+[A-Za-z_]")
MACRO_DEF = re.compile(r"^macro_rules!\s*([A-Za-z_][\w]*)\s*\{", re.M)
GENERATES_METHOD = re.compile(r"pub\s+(?:const\s+)?fn\s+\$")
# A type with at least this many registered methods is worth a name in the baseline;
# below it, the noise of one-off helpers would make the file unreadable.
MIN_SURFACE = 1
# The ruler itself must not silently find nothing.
MIN_TYPES = 25


def generating_macros(root):
    """Names of macros whose own body writes a ``pub fn $name``.

    Scanned file-wide because an expansion's method belongs to the type whose ``impl``
    block holds the *invocation*, not to the file that defines the macro.
    """
    names = set()
    for directory, _, files in os.walk(root):
        for name in sorted(files):
            if not name.endswith(".rs"):
                continue
            with open(os.path.join(directory, name), encoding="utf-8", errors="replace") as handle:
                text = handle.read()
            for match in MACRO_DEF.finditer(text):
                start = match.end()
                depth = 1
                end = len(text)
                for index in range(start, len(text)):
                    if text[index] == "{":
                        depth += 1
                    elif text[index] == "}":
                        depth -= 1
                        if depth == 0:
                            end = index
                            break
                if GENERATES_METHOD.search(text[start:end]):
                    names.add(match.group(1))
    return names


def measure(root):
    totals = {}
    macro_names = sorted(generating_macros(root))
    if macro_names:
        pattern = r"^\s*(%s)!\s*[\(\{\[]" % "|".join(map(re.escape, macro_names))
        calls = re.compile(pattern)
    else:
        calls = None
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
                if inside is not None and (
                    PUB_FN.match(line) or (calls is not None and calls.match(line))
                ):
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
