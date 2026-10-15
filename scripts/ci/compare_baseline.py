#!/usr/bin/env python3
"""Compare a measured current-state file against a registered baseline.

Both files are line-based records of ``<key> <value>`` (one record per line).

The gate is strict in both directions:
  * a key whose value grew           -> red (the surface got bigger);
  * a key whose value shrank         -> red with "re-record it" (a baseline that is
    looser than reality is a lie, and the next batch would be measured against it);
  * a key present in only one of them -> red (a new module/type appeared without a
    registered ceiling, or one was removed and its record left behind).

No tolerance, no percentage: a single unit of growth is a failure.
"""
import re
import sys

# A measurement file with no records is only meaningful if the ruler proved it looked at
# something. Rulers emit `#scanned <files>` for that; without it, "0 violations" and "the ruler
# read nothing" are indistinguishable, and the second one is not a pass.
SCANNED = re.compile(r"^#scanned (\d+)$")


def load(path):
    records = {}
    try:
        handle = open(path, encoding="utf-8")
    except FileNotFoundError:
        print(f"::error:: baseline {path} is missing from this checkout. Either it was never "
              f"committed or a gitignore rule excluded it -- check with "
              f"`git check-ignore -v {path}`. A gate whose inputs are not in the repository "
              f"only exists on the machine that wrote it.")
        sys.exit(1)
    with handle:
        for number, line in enumerate(handle, 1):
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            fields = line.rsplit(None, 1)
            if len(fields) != 2:
                print(f"::error line {path}:{number}: not a '<key> <value>' record: {line!r}")
                sys.exit(2)
            key, value = fields
            try:
                records[key] = int(value)
            except ValueError:
                print(f"::error line {path}:{number}: value is not an integer: {value!r}")
                sys.exit(2)
    return records


def load_with_marker(path):
    records = load(path)
    marker = None
    try:
        handle = open(path, encoding="utf-8")
    except FileNotFoundError:
        return records, None
    with handle:
        for line in handle:
            m = SCANNED.match(line.strip())
            if m:
                marker = int(m.group(1))
    return records, marker


def main():
    if len(sys.argv) != 3:
        print("usage: compare_baseline.py <baseline> <current>", file=sys.stderr)
        return 2
    baseline = load(sys.argv[1])
    current, scanned = load_with_marker(sys.argv[2])
    if not current and scanned is None:
        print(f"::error:: {sys.argv[2]} holds no records and no `#scanned <N>` marker, so "
              "nothing was measured. An empty measurement is not a pass -- if the surface "
              "really is empty, have the ruler print `#scanned <files seen>`.",
              file=sys.stderr)
        return 2
    problems = []

    for key in sorted(set(baseline) | set(current)):
        if key not in current:
            problems.append(f"{key}: registered but not measured any more -- was it removed? drop the line")
            continue
        if key not in baseline:
            problems.append(f"{key}={current[key]}: new entry with no registered ceiling")
            continue
        before, after = baseline[key], current[key]
        if after > before:
            problems.append(f"{key}: {before} -> {after} (+{after - before}) -- ceiling exceeded")
        elif after < before:
            problems.append(f"{key}: {before} -> {after} ({after - before}) -- tighten the baseline to the truth")

    if problems:
        for problem in problems:
            print(f"::error:: {problem}")
        print(f"\n{len(problems)} of {len(current)} entries disagree with the baseline "
              f"({sys.argv[1]}). Re-measure, then either shrink the surface or update the "
              f"baseline deliberately.")
        return 1

    print(f"baseline holds: {len(current)} entries, none above their registered value")
    total = sum(current.values())
    print(f"registered total: {total}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
