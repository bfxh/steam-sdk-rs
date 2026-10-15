#!/usr/bin/env python3
"""Every ``src/<path>.rs[:line]`` cited in the docs must resolve *today*.

Why this gate exists: `docs/SuperSteam-Design.md` is the evidence file -- its claims are trusted
because each one cites a file and a line. Three earlier restructures (``src/friends.rs`` and
``src/ugc.rs`` becoming directory modules, ``src/lib.rs`` being thinned) silently invalidated a
chunk of those citations, and nothing went red. A doc whose pointers rot is a doc that reads as
fabricated, and the next person cannot tell which claims were measured and which were not.

What it judges:

  * the cited path exists and is tracked, or the pointer is registered as historical (below);
  * a cited line number is within the file, and does not land on a blank line. A line that
    exists but points somewhere else is *not* detectable mechanically, so this gate does not
    claim to catch it -- it catches the two failure modes that are: the file is gone, and the
    number no longer describes anything at all.

Historical pointers: an evidence log records what was measured on the day it was written, and
rewriting those numbers to today's coordinates would replace a measurement with a guess. So a
pointer naming a file that no longer exists may be listed in
``baselines/doc_pointers_historical.txt`` instead -- one ``<doc> <token>`` pair per line, exactly
as cited. That list is a debt register and only shrinks: a pair in it that is not actually cited
any more is red, and so is a pointer that goes dead without being listed.

Floors, because an empty scan is not a pass: at least ``MIN_TOKENS`` pointer tokens and
``MIN_FILES`` documented files must be seen, or the matcher is broken rather than the docs clean.
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
DOCS = ["README.md", "CHANGELOG.md"]
GLOB_DIRS = ["docs"]
HISTORICAL = Path(__file__).resolve().parent / "baselines" / "doc_pointers_historical.txt"

TOKEN = re.compile(
    # A pointer starts at a boundary: without the lookbehind this also matches the
    # `src/windows_bindings.rs` suffix inside `steamworks-sys/src/windows_bindings.rs`,
    # which is a tracked path that resolves -- measured here on 2026-10-15, where that
    # substring turned four correct citations into registered "dead" ones.
    r"(?<![\w/.-])src/[\w/.-]*\.rs(?::\d+(?:\s*[-/]\s*\d+)*)?"
)
NUMS = re.compile(r"\d+")
MIN_TOKENS = 20
MIN_FILES = 3


def tracked():
    out = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True)
    if out.returncode != 0:
        print("::error git ls-files failed -- cannot judge what is tracked", file=sys.stderr)
        sys.exit(2)
    return {line.strip() for line in out.stdout.splitlines() if line.strip()}


def doc_paths():
    files = list(DOCS)
    for d in GLOB_DIRS:
        files += sorted(p.relative_to(ROOT).as_posix() for p in (ROOT / d).rglob("*.md"))
    missing = [f for f in files if not (ROOT / f).exists()]
    if missing:
        print(f"::error the configured doc set is missing {missing} -- the scan is not looking "
              "at the files it claims to", file=sys.stderr)
        sys.exit(2)
    return files


def read_lines(rel):
    return (ROOT / rel).read_text(encoding="utf-8", errors="replace").replace("\r\n", "\n").split("\n")


def load_historical():
    if not HISTORICAL.exists():
        print(f"::error {HISTORICAL.relative_to(ROOT)} is missing -- dead pointers would be "
              "silently tolerated instead of registered", file=sys.stderr)
        sys.exit(2)
    entries = {}
    for number, line in enumerate(read_lines(str(HISTORICAL.relative_to(ROOT))), 1):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        fields = line.split(None, 1)
        if len(fields) != 2:
            print(f"::error line {number} of {HISTORICAL.name}: not a '<doc> <token>' pair: "
                  f"{line!r}", file=sys.stderr)
            sys.exit(2)
        entries[tuple(fields)] = number
    return entries


def main():
    historical = load_historical()
    names = tracked()
    problems = []
    tokens = 0
    seen_files = set()
    cached = {}

    for rel in doc_paths():
        lines = read_lines(rel)
        for number, line in enumerate(lines, 1):
            for token in TOKEN.findall(line):
                tokens += 1
                seen_files.add(rel)
                path = token.split(":")[0]
                cited = [int(n) for n in NUMS.findall(token[len(path):])]
                if path not in names:
                    if (rel, token) not in historical:
                        problems.append(f"{rel}:{number}: `{token}` names a path that is not "
                                        "tracked any more -- fix the pointer or register it in "
                                        "baselines/doc_pointers_historical.txt")
                    continue
                if not cited:
                    continue
                if path not in cached:
                    cached[path] = read_lines(path)
                body = cached[path]
                for value in cited:
                    if value > len(body):
                        problems.append(f"{rel}:{number}: `{token}` -- {path} has "
                                        f"{len(body)} lines, so line {value} is past its end")
                    elif not body[value - 1].strip():
                        problems.append(f"{rel}:{number}: `{token}` -- line {value} of {path} "
                                        "is blank, which cannot be what the sentence cites")

    if tokens < MIN_TOKENS or len(seen_files) < MIN_FILES:
        print(f"::error scanned only {tokens} pointer tokens across {len(seen_files)} docs "
              f"(floors are {MIN_TOKENS} / {MIN_FILES}) -- an empty scan is not a pass",
              file=sys.stderr)
        return 2

    # A register entry nobody cites any more is a register that is being kept alive by habit.
    cited_pairs = {(rel, token) for rel in doc_paths()
                   for token in TOKEN.findall("\n".join(read_lines(rel)))}
    for pair, number in sorted(historical.items(), key=lambda kv: kv[1]):
        if pair not in cited_pairs:
            problems.append(f"{HISTORICAL.name}:{number}: registers {pair[0]} `{pair[1]}`, which "
                            "no document cites any more -- drop the line (this list only shrinks)")

    print(f"# {tokens} pointer tokens across {len(seen_files)} docs; "
          f"{len(historical)} registered as historical; the live ones must resolve")
    for problem in problems:
        print(f"::error:: {problem}")
    if problems:
        return 1
    print("every documented pointer resolves, and the historical register matches what is cited")
    return 0



if __name__ == "__main__":
    sys.exit(main())
