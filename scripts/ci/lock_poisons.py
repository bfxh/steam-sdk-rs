#!/usr/bin/env python3
"""Lock poisoning is a policy, so this ledger makes sure it stays one policy.

Every lock in this crate is taken inside callback dispatch, which runs user code. A user callback that
panics therefore poisons the lock it was holding, and the *next* guard request is where that shows up --
in a different caller, much later, far from the panic that caused it. The repository chose to recover
(see `src/utils/locks.rs`, and the trade written out there: a thread that died mid-update may have left
its table half-changed, and `into_inner` hands it over anyway). What must not happen is for the choice
to be re-litigated line by line, or for one file to recover while its neighbour aborts.

That is what the tree looked like when this rule was written: 22 guard requests, 18 of them aborting
(14 written as `x.lock().unwrap()` on one line, 4 as a chain with `.unwrap()` on its own line -- which is
why the patterns read across newlines, and why a line-based count reported 12 and was wrong), two of
those on `RwLock`s in `src/utils/warning.rs` reached from an `extern "C"` callback, where a panic does
not unwind into Rust at all. The remaining four had already chosen the opposite policy, recovering
inline in `src/networking_sockets/fake_udp.rs`.

Five things are judged, and a guard request outside the policy file must match one of the first four or
the fifth fires:

  1. it aborts -- `.unwrap()` / `.expect(..)` on the guard request;
  2. it inspects the lock's own `Result` (`.is_ok()`, `.err()`, `.then(..)`);
  3. it decides in a `match` arm what a poisoned lock means;
  4. it re-implements the recovery (`unwrap_or_else(..)`, or naming `PoisonError::into_inner`);
  5. none of the above -- the ruler cannot tell what this site does, so it **refuses to answer**
     (exit 2) rather than adding the request to a pass column. "0 violations" must never quietly mean
     "and one site I could not read".

**Why rule 5 exists, in this repository's own terms.** The first shipped version judged rule 1 and the
`into_inner` sweep. The tree it shipped with held a 23rd site it could not see:
`Callbacks::remove_handler` did `match self.callbacks.lock() { Ok(..) => .., Err(..) => eprintln!(..) }`
-- a third policy, report and skip, decided outside the policy home. The gate printed 0. It surfaced not
when the gate ran but when the sentence "22 guard requests, one policy" was checked against `grep`. A
ruler's zero is only as wide as its patterns, so the residue is a verdict now.

The policy file is exempt from rules 2-5 by design -- inspecting and matching is its job -- but not from
rule 1: an `unwrap()` in `src/utils/locks.rs` would put the whole tree's behaviour one edit away from the
thing this rule removes. It is also checked to still *be* the policy: if a helper moves out, "0
violations" would only mean the guards moved somewhere this gate no longer reads.

Test material is excluded by the same three rules every other ruler here uses -- `is_test_material` for
whole test files, `strip_test_modules` for `#[cfg(test)] mod` blocks, `strip_test_fns` for the bare
`#[test]` functions this crate writes at file scope. A test that wants to abort on poison is making a
different claim, about its own fixture, and is not something a caller can trigger.

Stdout is the ledger (one `<path> <count>` per offending file, plus the `#scanned <N>` marker); stderr
carries the per-site reason, so a message that named a `match` as an abort is caught by the canary
rather than at review time. Exit codes: 0 clean, 1 at least one violation, 2 no verdict.
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

GUARD_CALL = re.compile(r"\.(lock|read|write)\(\)")
# Whitespace rustfmt may put between a guard request and whatever it was handed to.
WRAP = "\n\t "
POISON_TAIL = re.compile(r"^\s*\.\s*(unwrap\s*\(\s*\)|expect\b)")
INSPECT_TAIL = re.compile(r"^\s*\.\s*(is_ok|is_err|ok|err|then|then_some)\s*[(=]")
RECOVERY_TAIL = re.compile(r"^\s*\.\s*unwrap_or_else\s*\(")
# A `match` governs the request when nothing but the scrutinee stands between them: `match data.lock()`.
# "does the preceding text end with `match`" is wrong -- the receiver sits in between.
MATCH_HEAD = re.compile(r"\bmatch\b[^{};]*$")
HOME = "src/utils/locks.rs"
# Three kinds of guard, the recovery they are built on, and the one helper allowed to inspect a Result.
HOME_PATTERNS = (r"\.lock\(\)", r"\.read\(\)", r"\.write\(\)", r"PoisonError::into_inner",
                 r"fn try_mutex_guard")
# Floors: a scan that read a handful of files, or saw no guard request at all, is not a clean tree -- it
# is a scan that did not happen.
MIN_FILES = 20
MIN_GUARDS = 4


def line_of(text, offset):
    """The 1-based line of `offset`, counted on the same text the patterns ran on.

    Line-preserving preprocessing is what makes this the line a reader finds in the file: the strip
    helpers blank test bodies rather than dropping them.
    """
    return text[:offset].count("\n") + 1


def classify(tail, head):
    """Which of rules 1-4 explains this call site, or None when none of them does."""
    if POISON_TAIL.match(tail):
        return f"guard request then aborts on poison ({tail[:24].strip()})"
    if INSPECT_TAIL.match(tail):
        return f"inspects the lock's own Result ({tail[:24].strip()})"
    if MATCH_HEAD.search(head):
        return "decides what a poisoned lock means in a match arm"
    if RECOVERY_TAIL.match(tail):
        return f"re-implements the recovery instead of calling {HOME}"
    return None


def sites(root):
    """Returns (violations, guard requests, files read, unclassifiable requests).

    A violation is `(path, line, reason)` -- one entry per offending line, because two patterns can
    describe the same site and the ledger has to count sites, not matches.
    """
    found = []
    blind = []
    guards = 0
    read = 0
    for path in sorted(Path(root).rglob("*.rs")):
        if "bindings" in path.name or is_test_material(path):
            continue
        read += 1
        rel = path.as_posix()
        text = strip_test_fns(strip_test_modules(blank_comments(
            path.read_text(encoding="utf-8", errors="replace"))))
        inside_home = rel == HOME
        for call in GUARD_CALL.finditer(text):
            guards += 1
            tail = text[call.end():call.end() + 60].lstrip(WRAP)
            if inside_home:
                if POISON_TAIL.match(tail):
                    found.append((rel, line_of(text, call.start()),
                                  "the policy file itself aborts on poison"))
                continue
            head = text[max(0, call.start() - 60):call.start()].rstrip(WRAP)
            where = line_of(text, call.start())
            reason = classify(tail, head)
            if reason:
                found.append((rel, where, reason))
            else:
                blind.append(f"{rel}:{where}: requests a guard outside {HOME} and this ruler cannot "
                             f"tell what it does with the Result ({tail[:24].strip()!r})")
        if not inside_home:
            for extra in re.finditer(r"PoisonError::into_inner", text):
                found.append((rel, line_of(text, extra.start()),
                              f"re-implements the recovery outside {HOME}"))
    # One violation per site: the sweep above and the classifier can describe the same line in two
    # words, and a ledger that counted descriptions would be measuring how many patterns fire rather
    # than how many places re-decided the policy. The first reason wins, so the message stays the more
    # specific one.
    unique = {}
    for rel, where, reason in found:
        unique.setdefault((rel, where), reason)
    return [(rel, where, reason) for (rel, where), reason in sorted(unique.items())], guards, read, blind


def main():
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("src")
    if not root.is_dir():
        print(f"::error:: {root} is not a directory -- the ruler has nothing to look at",
              file=sys.stderr)
        return 2
    found, guards, files, blind = sites(root)
    if files < MIN_FILES or guards < MIN_GUARDS:
        print(f"::error:: read {files} sources and saw {guards} guard requests; an empty scan is not "
              "a pass", file=sys.stderr)
        return 2
    home = Path(HOME)
    if not home.is_file():
        print(f"::error:: {HOME} is missing -- the recovery policy has no home to be judged against",
              file=sys.stderr)
        return 2
    home_text = home.read_text(encoding="utf-8", errors="replace")
    absent = [pattern for pattern in HOME_PATTERNS if not re.search(pattern, home_text)]
    if absent:
        print(f"::error:: {HOME} no longer matches {absent} -- the policy file is not the policy "
              "any more", file=sys.stderr)
        return 2
    for note in blind:
        print(f"::error:: {note}", file=sys.stderr)
    if blind:
        print(f"::error:: {len(blind)} guard request(s) this ruler cannot classify -- no verdict "
              "rather than a pass", file=sys.stderr)
        return 2

    counts = {}
    for rel, where, reason in found:
        print(f"::error:: {rel}:{where}: {reason}", file=sys.stderr)
        counts[rel] = counts.get(rel, 0) + 1
    for rel in sorted(counts):
        print(f"{rel} {counts[rel]}")
    print(f"#scanned {files}")
    print(f"# {len(found)} lock-poison policy break(s) among {guards} guard requests "
          f"in {files} hand-written sources", file=sys.stderr)
    return 1 if found else 0


if __name__ == "__main__":
    sys.exit(main())
