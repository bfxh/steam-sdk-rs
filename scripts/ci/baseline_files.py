#!/usr/bin/env python3
"""Every registered ledger must be real, tracked, non-empty (or excused), and read by someone.

Why: this repository's gates are files. Two failures have already happened here.

  * A baseline shipped whose ``*.txt`` rule in ``.gitignore`` swallowed it, so CI judged against a
    file that existed only on the machine that wrote it -- red in CI, green locally, the worst
    combination, and the reason `workflow_inputs.py` checks referenced paths today.
  * A ledger legitimately became empty (the panic-site count reached its target of zero). Empty is
    not automatically wrong, but it must be *declared* empty: otherwise "0 violations" and "the
    measurement never landed" look identical on the way past the gate.

So this gate judges four things, and only these:

  1. each file under ``scripts/ci/baselines/`` is tracked by git;
  2. each is non-empty, unless it appears in ``EXCUSED_EMPTY`` with a reason -- and an excuse whose
     file has since gained content is itself red, because that means the excuse was never retired;
  3. each has a declared reader (a workflow or a script) whose own text mentions that file name,
     either literally or through the parameterised prefix it builds. A ledger nobody reads is not a
     gate, it is a file;
  4. the inventory is not vacuous: at least ``MIN_BASELINES`` files must be found, or the scan
     exits 2 instead of passing on an empty directory.

It deliberately does not re-check the paths a workflow references -- `workflow_inputs.py` owns
that, and two rulers over the same thing drift.
"""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BASELINES = ROOT / "scripts" / "ci" / "baselines"

# Each ledger and the file that consumes it. `test_floor_*.txt` is built by test_floor.sh from a
# suite argument, so the reader mentions the prefix rather than the whole name -- rule 3 accepts
# that, and says so instead of pretending the match is literal.
READERS = {
    "api_coverage.txt": ".github/workflows/ci.yml",
    "doc_pointers_historical.txt": "scripts/ci/doc_pointers.py",
    "cstring_unwraps.txt": ".github/workflows/ci.yml",
    "handle_owners.txt": ".github/workflows/ci.yml",
    "header_citations.txt": ".github/workflows/ci.yml",
    "module_sizes.txt": ".github/workflows/ci.yml",
    "no_panic_sites.txt": ".github/workflows/ci.yml",
    "package_inventory.txt": ".github/workflows/ci.yml",
    "sdk_vendored.txt": ".github/workflows/ci.yml",
    "shell_hatches.txt": ".github/workflows/ci.yml",
    "test_floor_doc.txt": "scripts/ci/test_floor.sh",
    "test_floor_lib.txt": "scripts/ci/test_floor.sh",
    "type_surface.txt": ".github/workflows/ci.yml",
}

EXCUSED_EMPTY = {
    # The panic-site ledger's target IS zero, so an empty baseline is the passing state. The
    # measurement side is protected separately: compare_baseline.py refuses a zero-record
    # measurement that carries no `#scanned <N>` marker, so an empty ledger cannot be confused
    # with a ruler that read nothing.
    "cstring_unwraps.txt": "target is zero since 2026-10-19 (56 caller-triggerable sites paid out in "
                           "five batches the same day); the measurement side must prove it scanned "
                           "(compare_baseline marker rule)",
    "handle_owners.txt": "target is zero (every raw-interface wrapper now holds the client); the "
                         "measurement side must prove it scanned (compare_baseline marker rule)",
    "no_panic_sites.txt": "target is zero; the measurement side must prove it scanned (compare_baseline marker rule)",
}

# Measured on 2026-10-16: 11 files (the twelfth candidate, `header_citations.txt`, arrived with the
# SDK-citation gate that day). Below this the scan is not looking at the real inventory.
MIN_BASELINES = 13


def tracked():
    out = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True)
    if out.returncode != 0:
        print("::error git ls-files failed -- cannot judge what is tracked", file=sys.stderr)
        sys.exit(2)
    return {line.strip() for line in out.stdout.splitlines() if line.strip()}


def main():
    if not BASELINES.is_dir():
        print(f"::error {BASELINES} is not a directory -- the gate has nothing to look at",
              file=sys.stderr)
        return 2
    found = sorted(p.name for p in BASELINES.iterdir() if p.is_file())
    if len(found) < MIN_BASELINES:
        print(f"::error found only {len(found)} ledger files under "
              f"{BASELINES.relative_to(ROOT)} (floor is {MIN_BASELINES}) -- an empty inventory "
              "is not a pass", file=sys.stderr)
        return 2

    names = tracked()
    problems = []
    for name in found:
        rel = f"scripts/ci/baselines/{name}"
        if rel not in names:
            problems.append(f"{rel} is not tracked by git -- a gate whose input is not in the "
                            "repository only exists on the machine that wrote it "
                            "(check `git check-ignore -v`)")
        size = (BASELINES / name).stat().st_size
        if size == 0:
            if name not in EXCUSED_EMPTY:
                problems.append(f"{rel} is empty and not excused -- either re-record it from a "
                                "measurement or declare why empty is the passing state")
        elif name in EXCUSED_EMPTY:
            problems.append(f"{rel} is excused as empty but now holds {size} bytes -- retire the "
                            "excuse (this list is meant to shrink, not to be kept alive)")
        reader = READERS.get(name)
        if reader is None:
            problems.append(f"{rel} has no declared reader -- a ledger nobody reads is a file, "
                            "not a gate; add it to READERS in this script with the step that "
                            "consumes it")
            continue
        reader_path = ROOT / reader
        if not reader_path.exists():
            problems.append(f"{rel} declares reader {reader}, which does not exist")
            continue
        text = reader_path.read_text(encoding="utf-8", errors="replace")
        # Two leading segments, not one: `test_floor_lib.txt` is built as
        # `test_floor_${suite}.txt`, and matching on the single word `test` would let almost any
        # reader pass. A prefix this short is a matcher that approves things it never looked at.
        stem = "_".join(name.split("_")[:2]) + "_"
        literal = name in text
        parameterised = (stem in text and ".txt" in text
                         and re.search(r"\$\{|%s|f\"", text) is not None)
        if not (literal or parameterised):
            problems.append(f"{rel} declares reader {reader}, but that file never mentions it "
                            "(literally or as a parameterised prefix) -- the declared reader is "
                            "stale")

    # A reader mapping that still lists a file that has gone away.
    for name in READERS:
        if name not in found:
            problems.append(f"READERS declares {name}, which is no longer in "
                            "scripts/ci/baselines/ -- drop the line or restore the ledger")

    print(f"# {len(found)} ledger files, {len(EXCUSED_EMPTY)} excused as empty, "
          f"{len(READERS)} with a declared reader")
    for problem in problems:
        print(f"::error:: {problem}")
    if problems:
        return 1
    print("every ledger is tracked, populated (or excused), and read by a step that names it")
    return 0


if __name__ == "__main__":
    sys.exit(main())
