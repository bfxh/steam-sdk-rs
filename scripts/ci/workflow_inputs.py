#!/usr/bin/env python3
"""Check the workflow files against the repository they run in.

Four structural facts, each of which has already bitten this repo once:

  1. **Every path a workflow runs must be tracked.** Two real incidents: gates shipped whose
     baseline files were swallowed by ``*.txt`` in .gitignore, and a clone of a not-yet-committed
     tree proved nothing. A step that points at an untracked file is red in CI and green on the
     machine that wrote it -- the worst possible combination, because it looks like the gate works.
  2. **`permissions:` must not interpolate `${{ ... }}`.** GitHub evaluates it before dispatch
     data exists, so a workflow that does this produces zero jobs and a red run for every event
     that is not `workflow_dispatch` -- measured here on 2026-10-14 (runs 3140a69, cfd33c4).
  3. **Every job needs `timeout-minutes`.** Without it a hung step holds a runner slot for hours.
  4. **The rustc-warnings ban must be on a `cargo check` step and off the `cargo clippy` one.**
     `RUSTFLAGS` reaches dependencies built in the same invocation, and `steamworks-sys` is a
     workspace member, so bindgen's `dead_code` warning made three 1.80.0 jobs red on 2026-10-18
     while every stable job stayed green; scoping the command to `-p steamworks` did not help,
     because package selection does not filter the flags. The fix was an explicit
     `#![allow(dead_code)]` in that crate's own `src/lib.rs`. The ban must not move to the clippy
     step either: measured there it turned this repo's advisory clippy tier into 99 hard errors.
     A workflow with no ban at all is judged red too -- the gate going missing silently is the
     failure this repo has hit most often.

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


WARNINGS_BAN = re.compile(r"(-D|--deny)\s*=?\s*warnings")


def warnings_ban_shape(documents):
    """Judge where the rustc-warnings ban sits, in a shape that can only be right once.

    Aggregated over every workflow rather than judged per file: `release.yml` and
    `rebuild-bindings.yml` legitimately have no check or clippy step, so a per-file floor would call
    them red for the same reason it calls a gutted `ci.yml` red.

    The ban must exist, must be on a `cargo check` step, and must not be on a `cargo clippy` step.
    Both halves have a measured reason. `RUSTFLAGS` reaches dependencies built in the same
    invocation, `steamworks-sys` is a workspace member (so cargo does not cap its lints), and bindgen
    output carries a `dead_code` warning that only the pinned MSRV toolchain reports -- which is what
    made three 1.80.0 jobs red on 2026-10-18 while every stable job stayed green. The answer was
    `#![allow(dead_code)]` in that crate's own hand-written `src/lib.rs`, not a narrower package
    filter: package selection decides who gets built, not who gets the flags.

    The clippy rule is the other tier, recorded in the workflow's own comment: `clippy::correctness`
    and `clippy::suspicious` are denied by `clippy_gate.py` with no baseline, everything else is
    advisory. Measured with a crate-level `#![deny(warnings)]` in place, clippy turned that tier into
    99 hard errors, because a clippy lint is a rustc lint -- so a ban written as a crate attribute is
    not the same guarantee as a ban on the check step.
    """
    problems = []
    banned_checks = 0
    clippy_steps = 0
    for path, document in documents:
        for job_name, job in (document.get("jobs") or {}).items():
            for step in job.get("steps") or []:
                run = step.get("run")
                if not isinstance(run, str):
                    continue
                label = f"{path}: job '{job_name}' step '{step.get('name', '(unnamed)')}'"
                env = step.get("env") or {}
                flagged = bool(WARNINGS_BAN.search(str(env.get("RUSTFLAGS", ""))))
                # Inline `RUSTFLAGS="-D warnings" cargo ...`: the assignment and the command share one
                # body, which is why this half is read off the run text rather than the env mapping.
                if "RUSTFLAGS" in run:
                    flagged = flagged or bool(WARNINGS_BAN.search(run))
                if "cargo clippy" in run:
                    clippy_steps += 1
                    if flagged:
                        problems.append(f"{label}: clippy carries -D warnings, which collapses this "
                                        f"repo's two-tier clippy policy into one hard failure")
                elif "cargo check" in run:
                    if flagged:
                        banned_checks += 1
                elif flagged:
                    problems.append(f"{label}: -D warnings on a step that is neither check nor "
                                    f"clippy -- the ban belongs on the check step, where it is a "
                                    f"verdict")
    if not banned_checks:
        problems.append("no `cargo check` step anywhere carries -D warnings, so rustc warnings are "
                        "advisory again -- the vacuous `matches!(x, Err(Type))` assertion this gate "
                        "caught would pass unnoticed")
    if not clippy_steps:
        problems.append("the scan saw no `cargo clippy` step at all, so it is not reading the "
                        "quality job rather than finding it clean")
    return problems, banned_checks, clippy_steps


def main():
    if not os.path.isdir(WORKFLOW_DIR):
        print(f"::error {WORKFLOW_DIR} is missing -- nothing to check", file=sys.stderr)
        return 2
    tracked = tracked_files()
    problems = []
    checked = 0
    scanned = 0
    documents = []

    for name in sorted(os.listdir(WORKFLOW_DIR)):
        if not name.endswith((".yml", ".yaml")):
            continue
        path = os.path.join(WORKFLOW_DIR, name).replace(os.sep, "/")
        scanned += 1
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
        documents.append((path, document))
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

    shape_problems, banned, clippy = warnings_ban_shape(documents)
    problems.extend(shape_problems)

    for problem in problems:
        print(f"::error:: {problem}")
    if problems:
        return 1
    print(f"workflows check out: {checked} referenced paths across {scanned} workflow files are all "
          f"tracked, every job has a timeout, no permissions interpolation, and the rustc-warnings "
          f"ban is on {banned} check step(s) while {clippy} clippy step(s) stay advisory")
    return 0


if __name__ == "__main__":
    sys.exit(main())
