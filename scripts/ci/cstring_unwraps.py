#!/usr/bin/env python3
"""Count the `CString::new(x).unwrap()` sites whose `x` is something a caller chose.

`CString::new` fails on one input and one input only: a string with an interior nul. That makes
`.unwrap()` on it a *caller-triggerable* panic -- paste a log line, a user-chosen lobby key, a rich
presence value with a nul in it, and safe code aborts the crate's frame inside whatever method it
happened to call.

The classification is the whole design, and it is deliberately narrow:

  * `CString::new("lit").unwrap()` where the literal has no `\\0` escape is **not** a violation. It
    cannot fail, and calling it debt would make the ledger a number nobody can drive to zero.
  * `CString::new("a\\0b")` **is** a violation: an escaped nul is exactly the input that fails, and it
    is spelled like a safe literal, which is the case a naive "starts with a quote" rule would wave
    through.
  * `.expect(...)` counts the same as `.unwrap()`: the message does not make a panic cheaper.
  * Comments are blanked line-and-all, not stripped of their `//` marker. A ruler that leaves the
    prose can read a comment *about* `CString::new(..).unwrap()` as a live site -- and this crate's
    changelog and doc comments talk about that pattern constantly.

Test code is skipped with the same rule `api_coverage.py` uses (imported, not copied): a
`CString::new(..).unwrap()` inside a file-scope `#[cfg(test)]` module is the test *choosing* to
panick on a bad fixture, not something a caller can trigger, so counting it would make the ledger a
number nobody wants to drive to zero.

The argument is found by paren matching rather than one-line regex, so a call wrapped across lines
still counts, and `text[i:i+60]` for the method keeps `.unwrap()` attached to *this* conversion rather
than to something later on the line.

Measured 2026-10-18: 66 panicking conversions, 10 of them provably-safe literals, **56 debt across 23
files**. This ledger is that 56, and it may only shrink.
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from api_coverage import strip_test_modules  # noqa: E402  (one test-scoping rule, not two)

CALL = re.compile(r"CString::new\s*\(")
LITERAL = re.compile(r'^(?:&?r?#*"|r"|#")')
NUL_ESCAPE = re.compile(r"\\0|\\x00|\\u\{0+\}")
# Only the two forms that *panic*. `.unwrap_or(..)` is deliberately absent: it substitutes a value
# instead of failing, so counting it here would make a fallible-but-safe call look like a crash.
METHODS = (".unwrap()", ".expect")


def blank_comments(text):
    """Blank whole `//` lines and `/* ... */` blocks; cut trailing comments outside string literals."""
    out = []
    in_block = False
    for line in text.splitlines():
        stripped = line.strip()
        if in_block:
            if "*/" in stripped:
                in_block = False
            out.append("")
            continue
        if stripped.startswith("/*"):
            if "*/" not in stripped:
                in_block = True
            out.append("")
            continue
        if stripped.startswith("//"):
            out.append("")
            continue
        marker = line.find("//")
        if marker > 0 and line[:marker].count('"') % 2 == 0:
            line = line[:marker]
        out.append(line)
    return "\n".join(out)


def conversions(text):
    """Yield (line_number, method, argument) for each `CString::new(arg).method` in the text."""
    for match in CALL.finditer(text):
        index = match.end()
        depth = 1
        start = index
        while index < len(text) and depth:
            if text[index] == "(":
                depth += 1
            elif text[index] == ")":
                depth -= 1
            index += 1
        if depth:
            continue  # unbalanced -- not our sources, and refusing is better than guessing
        argument = text[start:index - 1].strip()
        tail = text[index:index + 80].lstrip()
        method = next((cand for cand in METHODS if tail.startswith(cand)), None)
        if method is None:
            continue
        yield text[:match.start()].count("\n") + 1, method, argument


def violations(root):
    found = []
    total = 0
    for path in sorted(Path(root).rglob("*.rs")):
        if "bindings" in path.name:
            continue
        text = strip_test_modules(blank_comments(path.read_text(encoding="utf-8", errors="replace")))
        for line, method, argument in conversions(text):
            total += 1
            infallible = LITERAL.match(argument) and not NUL_ESCAPE.search(argument)
            if not infallible:
                found.append(f"{path.as_posix()}:{line}: CString::new({argument[:40]}){method}")
    return found, total


def main():
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("src")
    if not root.is_dir():
        print(f"::error:: {root} is not a directory -- the ruler has nothing to look at",
              file=sys.stderr)
        return 2
    found, total = violations(root)
    files = sum(1 for path in root.rglob("*.rs") if "bindings" not in path.name)
    if files < 20 or total == 0:
        print(f"::error:: scanned {files} sources and {total} CString conversions; an empty scan "
              "is not a pass", file=sys.stderr)
        return 2

    counts = {}
    for record in found:
        key = record.split(":")[0]
        counts[key] = counts.get(key, 0) + 1
    for path in sorted(counts):
        print(f"{path} {counts[path]}")
    print(f"#scanned {files}")
    print(
        f"# {len(found)} caller-triggerable panic site(s), of {total} fallible CString conversions "
        "in the hand-written sources",
        file=sys.stderr,
    )
    for record in found:
        print(f"  site: {record}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
