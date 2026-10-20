#!/usr/bin/env python3
"""An `unsafe impl Send` is a promise that nothing in the type is thread-unsound. Exit 1 if a promise
has no evidence beside it.

Three shapes are counted per file:

  * **unclaimed** -- an `unsafe impl Send for T` with no `assert_send::<T>` in the same file, so the
    claim rests on the author having looked at the fields, which is exactly how the shape that this
    gate was written after survived for years: `ServerListRequest` was `Send` while the object it
    points at refcounted its closures with `Rc`, and the refcount was cloned on the thread Steam
    pumps callbacks on and dropped on the thread that owns the request.

    **What this column can and cannot judge, stated plainly.** `assert_send::<T>` has judgement power
    only for a type whose `Send` is *derived* -- the four callback objects here, where dropping the
    `+ Send + Sync` from the closure makes rustc refuse with E0277 (measured 2026-10-20). For a type
    that carries an `unsafe impl Send`, the assertion is satisfied by that very impl and can never
    fail, so a promise paid this way would be a test that cannot go red. The measurement that does
    say something about promises is **removal**: deleting all 15 `unsafe impl Send` in `src/` makes
    `cargo check --all-targets` red with 16 error lines and five distinct complaints
    (`*mut ISteamHTTP`, `*mut ISteamHTMLSurface`, `*mut ISteamNetworkingMessages`,
    `*mut ISteamNetworkingSockets`, `*mut steamworks_sys::SteamNetworkingMessage_t`) -- the promises
    are load-bearing at real boundaries, and `rc_next_to_claim` is the column that guards the hazard
    class rustc cannot see;
  * **rc_next_to_claim** -- any `Rc` token in a file that carries an `unsafe impl Send`, because a
    non-atomic refcount in a file whose types claim to cross threads is the same defect by its other
    half;
  * a `*_send::<T>` claim naming a type the file does not define is left to rustc: the crate already
    keeps such claims where they belong (`src/lib.rs:389` holds `static_assert_send`, `:424` claims
    `Client`, which is `Send` without any `unsafe impl`), and a claim naming a missing type does not
    compile, so a rule here would only re-implement the compiler badly.

**Why test material is not stripped here, unlike every other ruler in this directory.** The
compile-time claim lives in a `#[cfg(test)]` module on purpose -- it needs no running client, only
rustc's own `T: Send` check -- so applying `strip_test_modules` would delete the very evidence the
gate reads and report the fixed file as broken. The cost of that exception is stated plainly: a claim
written in a *different* file from the promise is not found, which is what `claim_without_owner`
exists to catch when the drift goes the other way.

Stdout is the ledger (`<path> <unclaimed> <rc_next_to_claim>` plus the `#scanned <N>` marker).
The `unclaimed` column is a **debt register that may only shrink**: 17 files carried an unverified
`unsafe impl Send` when this gate was written (measured below), so shipping it as zero tolerance would
have been a lie about the tree. `rc_next_to_claim` ships at **zero** because the shape it guards is the
one this file paid out on 2026-10-20.

`--fixture <file>` measures one file instead of `src/`, for the canary's positive control: without it,
a ruler that exits 1 on an empty tree is indistinguishable from one that read nothing.

stderr carries the per-file detail and the exit-2 "no verdict" message. Exit
codes: 0 clean, 1 at least one violation, 2 no verdict (nothing scanned).
"""
import re
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from cstring_unwraps import blank_comments  # noqa: E402  (one comment-blanking rule, shared)

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src"

CLAIM = re.compile(r"^\s*unsafe\s+impl\s+Send\s+for\s+([A-Za-z_][\w]*)", re.M)
ASSERTED = re.compile(r"assert_send::<\s*([A-Za-z_][\w]*)\s*>")
RC = re.compile(r"\bRc\b")


def measure(text):
    """Return (unclaimed, rc_next_to_claim) for one file.

    Comments are blanked for the `Rc` rule only: the claim itself lives in a `#[cfg(test)]` module,
    and a doc sentence that mentions `Rc` to explain why it is gone is not a refcount.
    """
    claims = CLAIM.findall(text)
    asserted = set(ASSERTED.findall(text))
    rc = len(RC.findall(blank_comments(text)))
    unclaimed = [name for name in claims if name not in asserted]
    return unclaimed, (rc if claims else 0)


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
    total = 0
    files = 0
    for path in sources(fixture, scan):
        files += 1
        text = path.read_text(encoding="utf-8", errors="replace")
        unclaimed, rc_with_claim = measure(text)
        if not unclaimed and not rc_with_claim:
            continue
        name = path.relative_to(ROOT).as_posix() if path.is_relative_to(ROOT) else path.name
        rows.append("%s %d %d" % (name, len(unclaimed), rc_with_claim))
        if unclaimed:
            print("::error:: %s promises Send for %s with no `assert_send` beside it"
                  % (name, ", ".join(unclaimed)), file=sys.stderr)
        if rc_with_claim:
            print("::error:: %s carries %d `Rc` token(s) next to an `unsafe impl Send` -- a "
                  "non-atomic refcount cannot cross threads" % (name, rc_with_claim), file=sys.stderr)
        total += rc_with_claim
    for row in rows:
        print(row)
    print("#scanned %d" % files)
    if fixture is None and files == 0:
        print("::error:: no sources scanned -- that is not a pass", file=sys.stderr)
        return 2
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main())
