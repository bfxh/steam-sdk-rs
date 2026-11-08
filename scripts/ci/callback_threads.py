#!/usr/bin/env python3
"""An `unsafe impl Send` is a promise that nothing reachable through the type is thread-unsound.
This ruler counts what can actually be checked about such a promise. Exit 1 if either judged column
is non-zero.

Columns, for every file that carries at least one promise:

  * **promises** -- how many `unsafe impl Send` the file declares. Not judged: this is the surface a
    reviewer has to read, and the ledger makes its growth as loud as its shrink.
  * **hazards** -- tokens of the one class a `Send` promise cannot survive: a non-atomic refcount
    (`Rc`) or interior mutability with no lock (`Cell`, `RefCell`, `UnsafeCell`). Zero tolerance.
    `Mutex` and `RwLock` are deliberately excluded -- they are the mechanism that makes sharing
    sound, and counting them would punish the fix for the defect this column looks for.
  * **unexplained** -- promises with no `// SAFETY:` line in the comment run above them, i.e. nobody
    wrote which boundary needs the promise and what was measured about it. Zero tolerance.

**Why there is no `unclaimed` column any more, and the measurement that decided it.** This ruler used
to require an `assert_send::<T>` beside every promise and reported 14 unverified ones. That column
could not be satisfied honestly: what it counted was exactly the types whose `Send` is granted by the
`unsafe impl` in the same file -- measured, all 14 names are a promise in their own file -- so an
`assert_send::<T>` written for them is proven by the very impl it is supposed to audit and can never
fail. Keeping it invited paying real debt with a test that cannot go red. What does say something
about a promise is **removal**: deleting all 15 `unsafe impl Send` in `src/` makes
`cargo check --all-targets` fail with 16 error lines and five distinct
`cannot be sent between threads safely` complaints (`*mut ISteamHTTP`, `*mut ISteamHTMLSurface`,
`*mut ISteamNetworkingMessages`, `*mut ISteamNetworkingSockets`,
`*mut steamworks_sys::SteamNetworkingMessage_t`) -- and that is what the `// SAFETY:` lines record
per promise now. `assert_send::<T>` keeps its value where `Send` is *derived*: the four `*Callbacks`
types in `matchmaking_servers.rs` carry such claims, and dropping `+ Send + Sync` from their
closures makes rustc refuse with E0277 twice.

Two blanking rules, both learned the hard way. Hazards are counted on comment-blanked text, so a
sentence explaining why an `Rc` is gone is not a refcount. `unexplained` reads raw lines, because its
evidence *is* a comment -- blanking first would erase the thing it looks for and report a clean file
as broken.

Stdout is the ledger (`<path> <promises> <hazards> <unexplained>` plus the `#scanned <N>` marker);
stderr carries the per-file detail and the exit-2 "no verdict" message.

`--fixture <file>` measures one file instead of `src/`, for the canary's positive control: without it,
a ruler that exits 1 on an empty tree is indistinguishable from one that read nothing.
Exit codes: 0 clean, 1 at least one violation, 2 no verdict (nothing scanned).
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cstring_unwraps import blank_comments  # noqa: E402  (one comment-blanking rule, shared)

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src"

CLAIM = re.compile(r"^\s*unsafe\s+impl\s+Send\s+for\s+([A-Za-z_][\w]*)", re.M)
HAZARD = re.compile(r"\bRc\b|\bRefCell\b|\bCell\b|\bUnsafeCell\b")


def measure(text):
    """Return (promises, hazards, unexplained) for one file."""
    promises = CLAIM.findall(text)
    hazards = len(HAZARD.findall(blank_comments(text))) if promises else 0
    lines = text.splitlines()
    unexplained = 0
    for index, line in enumerate(lines):
        if not CLAIM.match(line):
            continue
        # Walk the contiguous comment run above the promise: a doc block may sit between the
        # `// SAFETY:` line and the impl, and requiring adjacency would push the justification below
        # the prose -- the reason belongs with the promise, not nearest to it.
        block = []
        for back in range(index - 1, -1, -1):
            stripped = lines[back].strip()
            if not stripped or not stripped.startswith("//"):
                break
            block.append(stripped)
        if not any(entry.startswith("// SAFETY:") for entry in block):
            unexplained += 1
    return len(promises), hazards, unexplained


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
    violations = 0
    files = 0
    for path in sources(fixture, scan):
        files += 1
        text = path.read_text(encoding="utf-8", errors="replace")
        promises, hazards, unexplained = measure(text)
        if not promises:
            continue
        name = path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else path.name
        rows.append("%s %d %d %d" % (name, promises, hazards, unexplained))
        if hazards:
            print("::error:: %s carries %d non-atomic-refcount / unlocked-interior-mutability "
                  "token(s) beside a Send promise -- the class rustc cannot see"
                  % (name, hazards), file=sys.stderr)
        if unexplained:
            print("::error:: %s has %d Send promise(s) with no `// SAFETY:` line naming the boundary "
                  "that needs them and what was measured about them" % (name, unexplained),
                  file=sys.stderr)
        violations += hazards + unexplained
    for row in rows:
        print(row)
    print("#scanned %d" % files)
    if fixture is None and files == 0:
        print("::error:: no sources scanned -- that is not a pass", file=sys.stderr)
        return 2
    return 1 if violations else 0


if __name__ == "__main__":
    sys.exit(main())
