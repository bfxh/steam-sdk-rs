#!/usr/bin/env python3
"""Compare two ``<path> <sha256>`` lists and name every difference.

Unlike the ceiling baselines there is no "grow a bit is fine" here: a vendored file's
contents either match what was reviewed, or they do not. Three kinds of difference are
reported separately, because they mean different things:

  * ``changed``  -- the bytes of a vendored file are not what the manifest says;
  * ``added``    -- a file appeared that nobody reviewed;
  * ``removed``  -- a file disappeared (or was renamed).

A missing input file is treated as a tooling failure, not as a code finding -- gates
whose inputs are not in the repository only exist on the machine that wrote them.
"""
import sys


def load(path):
    try:
        handle = open(path, encoding="utf-8")
    except FileNotFoundError:
        print(f"::error:: manifest {path} is missing from this checkout. Either it was never "
              f"committed or a gitignore rule excluded it -- check with "
              f"`git check-ignore -v {path}`.")
        sys.exit(1)
    records = {}
    with handle:
        for number, line in enumerate(handle, 1):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            fields = line.rsplit(None, 1)
            if len(fields) != 2 or len(fields[1]) != 64:
                print(f"::error {path}:{number}: not a '<path> <sha256>' record")
                sys.exit(2)
            records[fields[0]] = fields[1]
    if len(records) < 10:
        print(f"::error {path} holds only {len(records)} records -- the manifest is not the "
              f"whole vendored tree")
        sys.exit(2)
    return records


def main():
    if len(sys.argv) != 3:
        print("usage: compare_hashes.py <manifest> <current>", file=sys.stderr)
        return 2
    wanted, current = load(sys.argv[1]), load(sys.argv[2])
    changed = sorted(key for key in set(wanted) & set(current) if wanted[key] != current[key])
    added = sorted(set(current) - set(wanted))
    removed = sorted(set(wanted) - set(current))
    for kind, keys in (("changed", changed), ("added", added), ("removed", removed)):
        for key in keys:
            print(f"::error:: {kind}: {key}")
    if changed or added or removed:
        print(f"\nvendored SDK differs: {len(changed)} changed, {len(added)} added, "
              f"{len(removed)} removed out of {len(current)} files. If this is a deliberate "
              f"SDK update, regenerate the manifest and review it as a change to the "
              f"interface the wrapper is built against.")
        return 1
    print(f"vendored SDK matches the manifest: {len(current)} files, {len(changed)} differences")
    return 0


if __name__ == "__main__":
    sys.exit(main())
