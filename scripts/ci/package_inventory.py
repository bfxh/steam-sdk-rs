#!/usr/bin/env python3
"""List the files that would go into each published ``.crate``.

``cargo package --list`` is the only thing that answers "what do users actually receive":
the repository contains plenty of files that must not ship (workflows, design notes, a local
``steam_appid.txt`` left behind so the examples run), and a missing file is worse -- a crate
that packages without ``LICENSE-MIT`` is a licensing hole no test notices.

Output is one ``<crate>/<path>`` per line, sorted, so the comparator can name a difference
instead of describing a changed hash.

``--no-verify`` is deliberate: verification would rebuild the crate, and the rebuild is
another gate's job. Here the question is only the file set.
"""
import re
import subprocess
import sys
import os

CRATES = ("steamworks", "steamworks-sys")
# Measured floors. A parse that finds almost nothing has to fail, not pass quietly.
MIN_ENTRIES = {"steamworks": 30, "steamworks-sys": 25}
CARGO = os.environ.get("CARGO", "cargo")


def inventory(crate):
    result = subprocess.run(
        [CARGO, "package", "--list", "--allow-dirty", "--no-verify", "-p", crate],
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        tail = "\n".join(result.stderr.splitlines()[-15:])
        print(f"::error cargo package --list failed for {crate} (exit {result.returncode}):\n{tail}",
              file=sys.stderr)
        sys.exit(2)
    paths = [line.strip() for line in result.stdout.splitlines() if line.strip()]
    # Drop any header line an older cargo may print.
    paths = [p for p in paths if not p.startswith("File ") and "Permission" not in p]
    if len(paths) < MIN_ENTRIES[crate]:
        print(f"::error {crate} inventory lists {len(paths)} files, expected at least "
              f"{MIN_ENTRIES[crate]} -- the parse is not looking at a package", file=sys.stderr)
        sys.exit(2)
    for path in sorted(paths):
        if not re.match(r"^[\w./-]+$", path):
            print(f"::error {crate}: unexpected file name {path!r} -- parse is guessing",
                  file=sys.stderr)
            sys.exit(2)
        print(f"{crate}/{path}")
    return len(paths)


def main():
    counts = {}
    for crate in CRATES:
        counts[crate] = inventory(crate)
    print(f"# counts: {counts}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
