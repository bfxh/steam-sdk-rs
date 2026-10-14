#!/usr/bin/env python3
"""Compare two sorted file lists and name every addition and removal.

Used by the packaging gate, where the question is *membership*: a file that appeared in the
``.crate`` was not intended to ship, and one that disappeared means users stopped receiving
it. Either way a hash would only say "something differs" and leave the reader to diff it.

A missing baseline is a tooling failure with its two causes named -- a gate whose inputs are
not in the repository only exists on the machine that wrote it.
"""
import sys


def load(path):
    try:
        handle = open(path, encoding="utf-8")
    except FileNotFoundError:
        print(f"::error:: list {path} is missing from this checkout. Either it was never "
              f"committed or a gitignore rule excluded it -- check with "
              f"`git check-ignore -v {path}`.")
        sys.exit(1)
    with handle:
        entries = {line.strip() for line in handle if line.strip() and not line.startswith("#")}
    if len(entries) < 20:
        print(f"::error {path} holds only {len(entries)} entries -- that is not a package "
              f"inventory, and comparing it would be theatre")
        sys.exit(2)
    return entries


def main():
    if len(sys.argv) != 3:
        print("usage: compare_paths.py <registered> <current>", file=sys.stderr)
        return 2
    wanted, current = load(sys.argv[1]), load(sys.argv[2])
    added = sorted(current - wanted)
    removed = sorted(wanted - current)
    for path in added:
        print(f"::error:: ships now, did not before: {path}")
    for path in removed:
        print(f"::error:: stopped shipping: {path}")
    if added or removed:
        print(f"\ninventory differs: {len(added)} added, {len(removed)} removed of "
              f"{len(current)} entries. Review it as a change to what users receive, then "
              f"regenerate the registered list.")
        return 1
    print(f"inventory matches what was reviewed: {len(current)} files")
    for crate in sorted({entry.split("/", 1)[0] for entry in current}):
        print(f"  {crate}: {sum(1 for e in current if e.startswith(crate + '/'))} files")
    return 0


if __name__ == "__main__":
    sys.exit(main())
