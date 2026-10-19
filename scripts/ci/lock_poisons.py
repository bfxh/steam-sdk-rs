#!/usr/bin/env python3
"""Lock poisoning is a policy, so this ledger makes sure it stays one policy.

Every lock in this crate is taken inside callback dispatch, which runs user code. A user callback that
panics therefore poisons the lock it was holding, and the *next* guard request is where that shows up --
in a different caller, much later, far from the panic that caused it. The repository chose to recover
(see `src/utils/locks.rs`, and the trade that is written out there: a thread that died mid-update may
have left its table half-changed, and `into_inner` hands it over anyway). What must not happen is for
the choice to be re-litigated line by line, or for one file to recover while its neighbour aborts --
which is precisely the shape the tree was in when this rule was written: 22 guard requests, of which 18
aborted (14 written as `x.lock().unwrap()` on one line, 4 written as a chain with `.unwrap()` on its own
line, which is why the ruler matches across newlines instead of by line), two of those 18 being
`.expect("warning func lock poisoned")` on `RwLock`s in `src/utils/warning.rs` that sit on the
`extern "C"` boundary, where a panic unwinds into C rather than into Rust. The other four had already
chosen the opposite policy, recovering inline in `src/networking_sockets/fake_udp.rs`.

So the rule is a zero target with two halves, and both are judged:

  * no site may take a guard and abort on poison -- `.lock()/.read()/.write()` followed by
    `.unwrap()` or `.expect(..)`;
  * no file may re-implement the recovery either. `PoisonError::into_inner` is allowed in
    `src/utils/locks.rs` and nowhere else, so there is one place that decides what a poisoned lock means.

Test material is excluded by the same three rules every other ruler here uses -- `is_test_material` for
whole test files, `strip_test_modules` for `#[cfg(test)] mod` blocks, and `strip_test_fns` for the bare
`#[test]` functions this crate writes at file scope. A test that wants to abort on poison is making a
different claim, about its own fixture, and it is not something a caller can trigger.

Exit codes: 0 clean, 1 at least one site, 2 the scan could not establish that it looked.
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from api_coverage import is_test_material, strip_test_fns, strip_test_modules  # noqa: E402
from cstring_unwraps import blank_comments  # noqa: E402

POISON_TAIL = re.compile(r"\.\s*(unwrap\s*\(\s*\)|expect\b)")
GUARD_CALL = re.compile(r"\.(lock|read|write)\(\)")
HOME = "src/utils/locks.rs"
# The recovery lives in one file, and that file is part of what this gate checks: three helpers, three
# kinds of guard, one policy. If it goes missing the scan has nothing to report on and refuses to answer,
# because "no violations found" from a ruler whose subject was deleted is the exact false green this
# repository keeps having to design against.
HOME_PATTERNS = (r"\.lock\(\)", r"\.read\(\)", r"\.write\(\)", r"PoisonError::into_inner")


def sites(root):
    """Returns (offenders, guards seen, files read)."""
    found = []
    guards = 0
    read = 0
    for path in sorted(Path(root).rglob("*.rs")):
        if "bindings" in path.name or is_test_material(path):
            continue
        read += 1
        text = strip_test_fns(strip_test_modules(blank_comments(
            path.read_text(encoding="utf-8", errors="replace"))))
        for call in GUARD_CALL.finditer(text):
            guards += 1
            tail = text[call.end():call.end() + 60]
            if not POISON_TAIL.match(tail.lstrip("\n\t ")):
                continue
            line = text[:call.start()].count("\n") + 1
            snippet = text[call.start():call.end()].strip()
            found.append(f"{path.as_posix()}:{line}: guard{snippet} then aborts on poison")
        if path.as_posix() != HOME:
            for extra in re.finditer(r"PoisonError::into_inner", text):
                line = text[:extra.start()].count("\n") + 1
                found.append(f"{path.as_posix()}:{line}: re-implements the recovery outside {HOME}")
    return found, guards, read


def main():
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("src")
    if not root.is_dir():
        print(f"::error:: {root} is not a directory -- the ruler has nothing to look at", file=sys.stderr)
        return 2
    found, guards, files = sites(root)
    if files < 20 or guards < 3:
        print(f"::error:: read {files} sources and saw {guards} guard calls; an empty scan is not a pass",
              file=sys.stderr)
        return 2
    home = Path(HOME)
    if not home.is_file():
        print(f"::error:: {HOME} is missing -- the recovery policy has no home to be judged against",
              file=sys.stderr)
        return 2
    home_text = home.read_text(encoding="utf-8", errors="replace")
    absent = [pattern for pattern in HOME_PATTERNS if not re.search(pattern, home_text)]
    if absent:
        print(f"::error:: {HOME} no longer contains {absent} -- the policy file is not the policy any more",
              file=sys.stderr)
        return 2
    counts = {}
    for record in found:
        key = record.split(":")[0]
        counts[key] = counts.get(key, 0) + 1
    for path in sorted(counts):
        print(f"{path} {counts[path]}")
    print(f"#scanned {files}")
    print(f"# {len(found)} lock-poison policy break(s) among {guards} guard requests "
          f"in {files} hand-written sources", file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
