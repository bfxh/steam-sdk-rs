#!/usr/bin/env python3
"""A `Result<T, ()>` says a call can fail and gives the caller nothing to act on. Exit 1 if any exist.

Three shapes are counted, all of them the same defect seen from different ends:

  * `-> Result<T, ()>` in a **public** signature -- the caller cannot match on a reason, cannot log one,
    and cannot map it without inventing the mapping themselves;
  * `Err(())` written anywhere in the crate's own code -- the point where the reason was thrown away;
  * `.ok_or(())?` / `map_err(|_| ())?` -- the same erasure arriving through a `Result` combinator.

Test material is excluded by the one rule every other ruler here uses (`is_test_material`,
`strip_test_modules`, `strip_test_fns`), and comments are blanked before scanning, so this counts what
a caller can trigger -- not a doc sentence explaining the history, and not an assertion inside a test
that is deliberately checking the failure arm.

**Why it ships at zero rather than as a shrinking ledger.** The crate paid this debt on 2026-10-20: 26
public signatures and 30 `Err(())` sites across six files became `SteamError`, and `join_lobby`'s
callback gained `JoinLobbyError` because `()` there was collapsing "the answer never arrived" together
with "Steam named a reason". A ceiling that allows 26 would let the next one through; zero tolerance is
what keeps a paid-out debt paid out -- the same treatment `cstring_unwraps.txt` and `lock_poisons.txt`
got when they reached their targets.

`--fixture <file>` measures one file instead of `src/`, for the canary's positive control: without it, a
ruler that exits 1 on an empty tree is indistinguishable from one that read nothing.

Stdout is the ledger (`<path> <signatures> <erasures>` plus the `#scanned <N>` marker); stderr carries
the per-site detail and the exit-2 "no verdict" message. Exit codes: 0 clean, 1 at least one site, 2 no
verdict (nothing scanned).
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from api_coverage import (  # noqa: E402  (one test-scoping rule, shared -- not three)
    is_test_material,
    strip_test_fns,
    strip_test_modules,
)
from cstring_unwraps import blank_comments  # noqa: E402

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src"

SIGNATURE = re.compile(r"^\s*pub\s+(?:const\s+)?fn[^;{]*?->\s*Result<[^;{]*?,\s*\(\)\s*>\s*(?:where|\{)", re.M)
ERASURE = re.compile(r"Err\(\s*\(\)\s*\)")
COMBINATOR = re.compile(r"\.ok_or\(\s*\(\)\s*\)|\.map_err\(\s*\|[^|]*\|\s*\(\)\s*\)")


def measure(text):
    """Return (signature count, erasure count) for code the caller can reach."""
    code = blank_comments(text)
    signatures = len(SIGNATURE.findall(code))
    erasures = len(ERASURE.findall(code)) + len(COMBINATOR.findall(code))
    return signatures, erasures


def sources(fixture, scan):
    if fixture:
        return [Path(fixture)]
    return sorted(Path(scan).rglob("*.rs"))


def main():
    fixture = None
    scan = SRC
    args = sys.argv[1:]
    if args and args[0] == "--fixture":
        fixture = Path(args[1])
    elif args:
        # CI calls this as `<ruler> src`, like every other ledger here.
        scan = ROOT / args[0]
    rows = []
    total_sites = 0
    files = 0
    for path in sources(fixture, scan):
        files += 1
        text = path.read_text(encoding="utf-8", errors="replace")
        if is_test_material(path):
            continue
        text = strip_test_fns(strip_test_modules(text))
        signatures, erasures = measure(text)
        if not signatures and not erasures:
            continue
        name = path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else path.name
        rows.append("%s %d %d" % (name, signatures, erasures))
        if erasures:
            print(
                "::error:: %s answers failure with `()` %d time(s): the caller cannot tell why"
                % (name, erasures),
                file=sys.stderr,
            )
        if signatures:
            print(
                "::error:: %s publishes %d signature(s) returning `Result<_, ()>`" % (name, signatures),
                file=sys.stderr,
            )
        total_sites += signatures + erasures
    for row in rows:
        print(row)
    print("#scanned %d" % files)
    if not rows and fixture is None:
        # An empty tree is the passing state, but "I found no files" must not look like it.
        if not any(p.suffix == ".rs" for p in Path(scan).rglob("*.rs")):
            print("::error:: no sources scanned -- that is not a pass", file=sys.stderr)
            return 2
    return 1 if total_sites else 0


if __name__ == "__main__":
    sys.exit(main())
