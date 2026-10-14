#!/usr/bin/env python3
"""Prove the `required-features` example really is compiled by the job that claims to compile it.

Cargo silently skips a target whose `required-features` are not enabled -- `--all-targets`
does not override that. So a job written as `cargo check --all-targets` can be green while
never looking at the one target that needs a feature, and nothing in the output says so.
This is measured, not assumed: the workflow already names `steamworks/futures` explicitly,
and this gate keeps that naming from rotting.

Two assertions, both from cargo's own JSON, and each is a control on the other:

  1. with the feature enabled, the target is compiled;
  2. without it, the target is NOT compiled.

If (2) ever stops holding, cargo changed its skipping behaviour and this gate's premise has
to be re-examined -- that is reported as a failure rather than silently passing, because the
whole reason the step exists is that skipping.
"""
import json
import os
import subprocess
import sys
import time

TARGET = "stream_events"
PACKAGE = "steamworks"
CARGO = os.environ.get("CARGO", "cargo")


def compiled_targets(extra_features):
    """Ask cargo which example targets it actually builds.

    The target's source is touched first: cargo reports artifacts for units it builds, and a
    warm cache would make it build nothing and report nothing -- a green job and an empty
    measurement would then look identical.
    """
    source = os.path.join("examples", f"{TARGET}.rs")
    if not os.path.exists(source):
        print(f"::error example source {source} is missing -- the gate has nothing to measure",
              file=sys.stderr)
        sys.exit(2)
    now = time.time()
    os.utime(source, (now, now))

    command = [CARGO, "check", "--locked", "--workspace", "--all-targets", "--message-format", "json"]
    command += [f"--features={feature}" for feature in extra_features]
    result = subprocess.run(command, capture_output=True, text=True)
    if result.returncode != 0:
        tail = "\n".join(result.stderr.splitlines()[-25:])
        print(f"::error cargo check failed (exit {result.returncode}):\n{tail}", file=sys.stderr)
        sys.exit(1)

    built = set()
    for line in result.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-artifact":
            continue
        target = message.get("target") or {}
        package_id = message.get("package_id", "")
        if target.get("name") and PACKAGE in package_id and target.get("kind") == ["example"]:
            built.add(target["name"])
    return built


def main():
    with_feature = compiled_targets([f"{PACKAGE}/futures"])
    without_feature = compiled_targets([])

    print(f"examples compiled with --features {PACKAGE}/futures: {sorted(with_feature)}")
    print(f"examples compiled without any feature:              {sorted(without_feature)}")

    problems = []
    if TARGET not in with_feature:
        problems.append(f"{TARGET} was NOT compiled even with the feature named: the job that "
                        f"claims to build it is not building it")
    if TARGET in without_feature:
        problems.append(f"{TARGET} was compiled without the feature: cargo no longer skips a "
                        f"required-features target, so this gate's premise needs re-reading "
                        f"and the explicit --features flag may no longer be what protects it")
    if len(with_feature) < 2:
        problems.append(f"only {len(with_feature)} example(s) seen -- the parse is not looking "
                        f"at the whole set")

    for problem in problems:
        print(f"::error:: {problem}")
    if problems:
        return 1
    print(f"compile coverage holds: {TARGET} needs the feature, and the job names it")
    return 0


if __name__ == "__main__":
    sys.exit(main())
