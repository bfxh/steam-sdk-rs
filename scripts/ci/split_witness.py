#!/usr/bin/env python3
"""Re-derive the "moved, not lost" witness for one of this crate's directory-module splits.

Why this exists: seven split commits each publish a sentence of the form "N non-blank lines before,
X lost, Y added", and until now the only way to check one was to rewrite the multiset script in a
scratch buffer. A number in this repository is supposed to come with the command that produces it.

Both sides are read from git objects, so a split can be re-derived long after the working tree moved
on -- which is exactly what the seven published claims need, since each one described a tree that no
longer exists.

What it judges, and what it does not:

  * lost = lines the old file had that no new file has. Every one of those must be a declaration the
    split deliberately widened (`pub(super)`) or a doc line it rewrote. A lost line nobody can name is
    a line the move destroyed.
  * added = the reverse, and the report separates wrapper-shaped lines (module headers, `use`, `mod`,
    `pub use`, `impl` openers and braces) from the rest. Anything non-wrapper that appeared needs an
    explanation in the commit that added it.
  * it cannot see mis-assignment: a block moved into the wrong file still balances the multiset, so
    "0 lost" is not proof the split was sensible. `cargo check` owns that half -- twice in this
    repository it was the compiler, not this witness, that found the wrong destination.

Usage: split_witness.py <old-commit> <old-path> <new-commit> <new-dir> [file name to exclude] ...
"""
import subprocess
import sys
from collections import Counter
from pathlib import Path

WRAPPER_PREFIXES = ("//!", "mod ", "pub use ", "pub(crate) use ", "impl ", "use ", "}")


def blob(commit, path):
    out = subprocess.run(
        ["git", "show", f"{commit}:{path}"], capture_output=True, check=True
    ).stdout.decode("utf-8", errors="replace")
    return out.replace("\r\n", "\n")


def multiset(text):
    return Counter(line.rstrip() for line in text.split("\n") if line.strip())


def main(argv):
    if len(argv) < 4:
        print(__doc__)
        return 2
    old_commit, old_path, new_commit, new_dir = argv[0], argv[1], argv[2], argv[3]
    skip = set(argv[4:])

    listing = subprocess.run(
        ["git", "ls-tree", "-r", "--name-only", new_commit, "--", new_dir],
        capture_output=True, check=True, text=True
    ).stdout.split()
    paths = sorted(p for p in listing if p.endswith(".rs") and Path(p).name not in skip)
    if not paths:
        print(f"::error:: no .rs files found under {new_dir} at {new_commit}", file=sys.stderr)
        return 2

    old = multiset(blob(old_commit, old_path))
    new = Counter()
    for path in paths:
        new += multiset(blob(new_commit, path))

    lost, added = old - new, new - old
    other = {l: n for l, n in added.items() if not l.strip().startswith(WRAPPER_PREFIXES)}
    print(f"old {old_path} @ {old_commit[:9]}: {sum(old.values())} non-blank lines")
    print(f"new {new_dir} ({len(paths)} files) @ {new_commit[:9]}: {sum(new.values())} non-blank lines")
    print(f"lost: {sum(lost.values())}")
    for line, n in sorted(lost.items()):
        print(f"  LOST x{n}: {line.strip()[:76]}")
    print(f"added: {sum(added.values())}, of which non-wrapper {sum(other.values())}")
    for line, n in sorted(other.items()):
        print(f"  ADDED-NONWRAPPER x{n}: {line.strip()[:76]}")
    # This is a report, not a gate: it exits 0 whenever it measured something, and a human reads the
    # two lists. The floor that stops it lying is the empty-file-list check above.
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
