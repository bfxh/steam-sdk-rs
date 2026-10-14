#!/usr/bin/env python3
"""Check the workflow files against the repository they run in.

Three structural facts, each of which has already bitten this repo once:

  1. **Every path a workflow runs must be tracked.** Two real incidents: gates shipped whose
     baseline files were swallowed by ``*.txt`` in .gitignore, and a clone of a not-yet-committed
     tree proved nothing. A step that points at an untracked file is red in CI and green on the
     machine that wrote it -- the worst possible combination, because it looks like the gate works.
  2. **`permissions:` must not interpolate `${{ ... }}`.** GitHub evaluates it before dispatch
     data exists, so a workflow that does this produces zero jobs and a red run for every event
     that is not `workflow_dispatch` -- measured here on 2026-10-14 (runs 3140a69, cfd33c4).
  3. **Every job needs `timeout-minutes`.** Without it a hung step holds a runner slot for hours.

This is not `actionlint`; it checks the invariants specific to how this repository has failed,
with no third-party binary to fetch mid-run.
"""
import os
import re
import subprocess
import sys
import yaml

WORKFLOW_DIR = ".github/workflows"
# Path-like tokens a run step may reference that must exist in the repository.
PREFIXES = ("scripts/", "src/", "docs/", ".github/", "steamworks-sys/")
# At least this many referenced paths, or the scan is not looking at the workflows it thinks it is.
MIN_REFERENCES = 8


def tracked_files():
    result = subprocess.run(["git", "ls-files"], capture_output=True, text=True)
    if result.returncode != 0:
        print("::error git ls-files failed -- cannot judge what is tracked", file=sys.stderr)
        sys.exit(2)
    return {line.strip() for line in result.stdout.splitlines() if line.strip()}


def referenced_paths(run_body):
    for token in re.findall(r"[\w./$-]+", run_body):
        if token.startswith(PREFIXES) and "." in token:
            cleaned = token.rstrip(".,;:")
            # A step may write into these; only inputs are required to exist.
            if "$" not in cleaned and not cleaned.endswith("/"):
                yield cleaned


def permissions_interpolations(path):
    """Flag any `permissions:` entry whose value interpolates ``${{ ... }}``.

    A block scanner on the raw file, because the document-level view has already lost the
    interpolation (see the note in `main`).
    """
    with open(path, encoding="utf-8") as handle:
        lines = handle.read().splitlines()

    found = []
    block_indent = None  # indent of the `permissions:` key while inside its block
    for number, line in enumerate(lines, 1):
        stripped = line.strip()
        if not stripped or stripped.startswith("#"):
            continue
        indent = len(line) - len(line.lstrip())
        if stripped.startswith("permissions:"):
            if "${{" in stripped:
                found.append(f"{path}:{number}: `permissions` itself interpolates, which GitHub "
                             f"evaluates before the event data exists")
            block_indent = indent
            continue
        if block_indent is not None:
            if indent <= block_indent:
                block_indent = None
            elif "${{" in stripped:
                found.append(f"{path}:{number}: permission entry inside a `permissions:` block "
                             f"interpolates: {stripped}")
    return found


def main():
    if not os.path.isdir(WORKFLOW_DIR):
        print(f"::error {WORKFLOW_DIR} is missing -- nothing to check", file=sys.stderr)
        return 2
    tracked = tracked_files()
    problems = []
    checked = 0

    for name in sorted(os.listdir(WORKFLOW_DIR)):
        if not name.endswith((".yml", ".yaml")):
            continue
        path = os.path.join(WORKFLOW_DIR, name).replace(os.sep, "/")
        try:
            with open(path, encoding="utf-8") as handle:
                document = yaml.safe_load(handle)
        except yaml.YAMLError as error:
            # A crash is not a verdict: report which file cannot be parsed and keep checking
            # the rest, so one broken workflow does not hide a second problem elsewhere.
            print(f"::error:: {path} does not parse: {str(error).splitlines()[0]}")
            problems.append(f"{path}: workflow is not valid YAML")
            continue
        if not isinstance(document, dict):
            problems.append(f"{path}: parses to {type(document).__name__}, not a workflow mapping")
            continue
        # `permissions:` is checked on the raw text, deliberately: YAML swallows the
        # `${{ ... }}` sequence (a probe with `contents: ${{ github.ref_type == 'tag' &&
        # 'write' || 'read' }}` parses to the string `write`), so a parsed-document check
        # cannot see the interpolation at all -- measured, and it is the exact bug that made
        # every non-dispatch run of rebuild-bindings.yml produce zero red jobs.
        for problem in permissions_interpolations(path):
            problems.append(problem)

        jobs = document.get("jobs") or {}
        if not jobs:
            problems.append(f"{path}: parses with no jobs at all")
            continue

        for job_name, job in jobs.items():
            if "timeout-minutes" not in job:
                problems.append(f"{path}: job '{job_name}' has no timeout-minutes")
            for step in job.get("steps") or []:
                run = step.get("run")
                if not isinstance(run, str):
                    continue
                for referenced in referenced_paths(run):
                    checked += 1
                    if referenced not in tracked:
                        problems.append(f"{path}: step '{step.get('name', '(unnamed)')}' runs "
                                        f"'{referenced}' which is not tracked in git")

    if checked < MIN_REFERENCES:
        problems.append(f"only {checked} referenced paths found -- the scan is not reading the "
                        f"workflows, and an empty scan is not a pass")

    for problem in problems:
        print(f"::error:: {problem}")
    if problems:
        return 1
    print(f"workflows check out: {checked} referenced paths are all tracked, every job has a "
          f"timeout, no permissions interpolation")
    return 0


if __name__ == "__main__":
    sys.exit(main())
