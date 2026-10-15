#!/usr/bin/env python3
"""Each gate must prove it can judge red. A gate that cannot fail is a log line.

The mechanical version of the rule this repo already follows by hand: no gate ships without a
positive run *and* a per-item disproof. It exists because a ruler can be wrong in a way that
looks like success -- on 2026-10-15 the documentation-pointer ruler reported four citations as
dead when they were perfectly good, because it matched a path suffix inside a longer, correct
path. Nothing already in the pipeline could have caught that. Only a fixture whose expected
answer is red can.

Every canary builds a throwaway tree under the system temp directory, copies the ruler into the
same relative position it occupies in the repository (so the ruler's own `parents[2]`-style root
discovery points at the fixture rather than at the checkout), runs it and asserts the exit code it
should produce. The real working tree is never mutated. Where a ruler judges "is this tracked",
the fixture gets its own `git init` so that question is answered about the fixture, not us.

Not covered, registered here rather than glossed over:

  * `clippy_gate.py` and `test_floor.sh` -- failing them usefully needs a real cargo build. Their
    anti-vacuity floors (artifact counts, "0 tests ran") are exercised by the real runs in CI.
  * `workflow_inputs.py` -- it reads `.github/workflows` relative to the checkout and judges
    tracked paths; a fixture would have to reproduce half this repo.
  * `package_inventory.py` / `sdk_vendored.py` -- they wrap `cargo package --list` and the vendored
    hash manifest; a fixture would be a copy of the crate.

Usage: `python3 scripts/ci/canaries.py`. Exits 1 if a gate failed to judge as expected, 2 if the
suite itself could not run (which is not a pass either).
"""
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CI = "scripts/ci"
BASELINES = f"{CI}/baselines"
failures = []
checks = 0


def expect(label, got, want):
    global checks
    checks += 1
    if got == want:
        print(f"  ok   {label}: exit={got}")
    else:
        print(f"  FAIL {label}: exit={got} want={want}")
        failures.append(label)


def write(path, text):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8", newline="\n")
    return path


def read(path):
    return path.read_text(encoding="utf-8", errors="replace")


def git(tmp, *args):
    return subprocess.run(["git", *args], cwd=str(tmp), capture_output=True, text=True)


def run_script(script, args=None, cwd=None):
    return subprocess.run([sys.executable, str(script)] + (args or []), cwd=str(cwd or ROOT),
                          capture_output=True, text=True, encoding="utf-8", errors="replace")


def fixture(ruler_name):
    """A temp tree with `ruler_name` sitting where it expects to be, and a git index."""
    tmp = Path(tempfile.mkdtemp(prefix="canary_"))
    dst = tmp / CI / ruler_name.name
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(ruler_name, dst)
    git(tmp, "init", "-q")
    return tmp, dst


def floor_probe(dst, name, old, new):
    """A copy of a ruler with one floor raised out of reach: proves the floor is load-bearing."""
    probe = dst.parent / name
    write(probe, read(dst).replace(old, new, 1))
    return probe


def canary_comparator():
    print("compare_baseline.py -- both directions, new and vanished keys, and the marker rule")
    tmp = Path(tempfile.mkdtemp(prefix="canary_cmp_"))
    try:
        cmp_script = ROOT / CI / "compare_baseline.py"
        base = write(tmp / "base.txt", "thing 5\n")
        cases = [
            ("growth above the ceiling", "thing 6\n", 1),
            ("shrink must be re-recorded", "thing 4\n", 1),
            ("a new key with no registered ceiling", "thing 5\nother 1\n", 1),
            ("a registered key that vanished (with a marker)", "other 1\n#scanned 3\n", 1),
            ("a matching measurement passes", "thing 5\n", 0),
        ]
        for label, current, want in cases:
            write(tmp / "cur.txt", current)
            expect(label, run_script(cmp_script, [base, tmp / "cur.txt"]).returncode, want)
        write(tmp / "cur.txt", "#scanned 51\n")
        expect("empty ledger WITH the `#scanned` marker passes",
               run_script(cmp_script, [write(tmp / "base.txt", ""), tmp / "cur.txt"]).returncode, 0)
        write(tmp / "cur.txt", "")
        expect("empty ledger WITHOUT the marker is refused",
               run_script(cmp_script, [tmp / "base.txt", tmp / "cur.txt"]).returncode, 2)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def canary_panic_ledger():
    print("no_panic_on_unknown_enum.py -- a violation reaches the comparator as red")
    ruler = ROOT / CI / "no_panic_on_unknown_enum.py"
    tmp, dst = fixture(ruler)
    try:
        write(tmp / "src" / "bad.rs",
              "pub enum E { A }\nfn map(e: E) -> u8 {\n"
              "    match e {\n        E::A => 1,\n        _ => unreachable!(),\n    }\n}\n")
        for i in range(20):  # the ruler's own floor is 20 scanned sources
            write(tmp / "src" / f"f{i}.rs", f"pub fn f{i}() {{}}\n")
        out = run_script(dst, ["src"], cwd=tmp)
        expect("the violating site is emitted as a record",
               0 if "bad.rs 1" in out.stdout else 1, 0)
        expect("the scan marker is emitted", 0 if re.search(r"^#scanned \d+$", out.stdout, re.M) else 1, 0)
        measure = write(tmp / "cur.txt", out.stdout)
        baseline = write(tmp / "base.txt", "")
        expect("the comparator turns that record red",
               run_script(ROOT / CI / "compare_baseline.py", [baseline, measure]).returncode, 1)
        # and the floor: a tree with too few sources must refuse, not report zero sites
        probe = floor_probe(dst, "floor_probe.py", "MIN_SCANNED_FILES = 20", "MIN_SCANNED_FILES = 500")
        expect("too few scanned sources is refused rather than passed",
               run_script(probe, ["src"], cwd=tmp).returncode, 2)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def replace_block(text, start, new):
    """Swap a whole `NAME = { ... }` literal for a fixture one.

    Injecting instead of replacing is what made the first version of this canary wrong: the real
    mapping still named ten ledgers the fixture tree does not have, so the gate judged
    "READERS declares X, which is no longer in baselines" for every one of them -- a red that
    proved nothing about the rule under test.
    """
    at = text.index(start)
    end = text.index("\n}\n", at) + len("\n}\n")
    return text[:at] + new + text[end:]


def canary_doc_pointers():
    print("doc_pointers.py -- dead path, past EOF, blank line, stale register, vacuous scan")
    ruler = ROOT / CI / "doc_pointers.py"
    tmp, dst = fixture(ruler)
    try:
        write(tmp / "src" / "real.rs", "line one\n\nline three\n")
        write(tmp / "README.md", "see src/real.rs:1\n")
        # the ruler refuses to guess at history: a missing register file is exit 2, so the fixture
        # has to carry the empty one before any judgement is meaningful
        write(tmp / BASELINES / "doc_pointers_historical.txt", "# nothing registered\n")

        def corpus(bad_line):
            # the ruler's floors (20 tokens / 3 docs) must be met, or the verdict we get is "the
            # scan was vacuous" instead of the one under test
            write(tmp / "CHANGELOG.md", bad_line)
            for i in range(24):
                write(tmp / "docs" / f"d{i}.md", "see src/real.rs:1\n")
            git(tmp, "add", "-A")
            return run_script(dst, cwd=tmp).returncode

        expect("a path that is not tracked", corpus("see src/gone.rs:1\n"), 1)
        expect("a line past the end of the file", corpus("see src/real.rs:99\n"), 1)
        expect("a line landing on a blank line", corpus("see src/real.rs:2\n"), 1)
        expect("a pointer that resolves passes", corpus("see src/real.rs:1\n"), 0)
        write(tmp / BASELINES / "doc_pointers_historical.txt", "CHANGELOG.md src/never_cited.rs:1\n")
        git(tmp, "add", "-A")
        expect("a register entry nobody cites any more", run_script(dst, cwd=tmp).returncode, 1)
        write(tmp / BASELINES / "doc_pointers_historical.txt", "# nothing registered\n")
        write(tmp / "docs" / "d0.md", "see src/gone.rs:7\n")
        git(tmp, "add", "-A")
        expect("a dead pointer that is not registered", run_script(dst, cwd=tmp).returncode, 1)
        probe = floor_probe(dst, "floor_probe.py", "MIN_TOKENS = 20", "MIN_TOKENS = 10 ** 6")
        expect("an empty scan is refused, not passed", run_script(probe, cwd=tmp).returncode, 2)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def canary_baseline_files():
    print("baseline_files.py -- untracked, empty, stale excuse, unread, vacuous")
    ruler = ROOT / CI / "baseline_files.py"
    tmp, dst = fixture(ruler)
    try:
        ledgers = ("alpha.txt", "beta.txt", "gamma.txt")
        write(tmp / ".github/workflows/ci.yml",
              "".join(f"run: compare scripts/ci/baselines/{n}\n" for n in ledgers))
        for n in ledgers:
            write(tmp / BASELINES / n, "thing 1\n")
        mapping = ('READERS = {\n'
                   + "".join(f'    "{n}": ".github/workflows/ci.yml",\n' for n in ledgers)
                   + '}\n')
        script = replace_block(read(dst), "READERS = {", mapping)
        script = script.replace("MIN_BASELINES = 10", "MIN_BASELINES = 3")
        probe = write(tmp / CI / "probe.py", script)
        git(tmp, "add", "-A")
        expect("a clean fixture inventory passes", run_script(probe, cwd=tmp).returncode, 0)

        write(tmp / BASELINES / "delta.txt", "thing 1\n")  # deliberately left untracked
        expect("an untracked ledger", run_script(probe, cwd=tmp).returncode, 1)
        (tmp / BASELINES / "delta.txt").unlink()
        git(tmp, "add", "-A")

        write(tmp / BASELINES / "alpha.txt", "")
        expect("an empty ledger with no excuse", run_script(probe, cwd=tmp).returncode, 1)

        stale = replace_block(read(probe), "EXCUSED_EMPTY = {",
                              'EXCUSED_EMPTY = {\n    "alpha.txt": "fixture excuse",\n}\n')
        write(tmp / CI / "probe_stale.py", stale)
        expect("an empty-excuse on a ledger that is empty stays excused",
               run_script(tmp / CI / "probe_stale.py", cwd=tmp).returncode, 0)
        write(tmp / BASELINES / "alpha.txt", "thing 1\n")
        git(tmp, "add", "-A")
        expect("an empty-excuse on a ledger that now has content",
               run_script(tmp / CI / "probe_stale.py", cwd=tmp).returncode, 1)

        unread = read(probe).replace('    "gamma.txt": ".github/workflows/ci.yml",\n', "")
        write(tmp / CI / "probe_unread.py", unread)
        expect("a ledger with no declared reader",
               run_script(tmp / CI / "probe_unread.py", cwd=tmp).returncode, 1)

        # A reader entry for a ledger that has gone away. This has to be spliced into the dict
        # itself: appending `READERS[...] = ...` to the end of the file puts it after
        # `sys.exit(main())`, where it can never run -- which is exactly why the first version of
        # this assertion passed for the wrong reason.
        ghost = replace_block(read(probe), "READERS = {",
                              'READERS = {\n'
                              + "".join(f'    "{n}": ".github/workflows/ci.yml",\n'
                                        for n in ledgers)
                              + '    "gone_forever.txt": "scripts/ci/nothing.py",\n}\n')
        write(tmp / CI / "probe_ghost.py", ghost)
        expect("a reader entry for a ledger that no longer exists",
               run_script(tmp / CI / "probe_ghost.py", cwd=tmp).returncode, 1)

        floor = read(probe).replace("MIN_BASELINES = 3", "MIN_BASELINES = 40")
        write(tmp / CI / "probe_floor.py", floor)
        expect("an empty ledger inventory is refused, not passed",
               run_script(tmp / CI / "probe_floor.py", cwd=tmp).returncode, 2)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def canary_api_coverage():
    print("api_coverage.py -- a wrong input is a refusal, not zero coverage")
    ruler = ROOT / CI / "api_coverage.py"
    expect("bindings path pointed at a directory", run_script(ruler, [ROOT / "src"]).returncode, 2)
    expect("bindings path pointed at a non-bindings file",
           run_script(ruler, [ROOT / "README.md"]).returncode, 2)
    expect("the real invocation still judges", run_script(ruler, []).returncode, 0)


CANARIES = (canary_comparator, canary_panic_ledger, canary_doc_pointers, canary_baseline_files,
            canary_api_coverage)


def main():
    for canary in CANARIES:
        try:
            canary()
        except Exception as error:  # a canary that cannot run is not a pass either
            failures.append(f"{canary.__name__} raised {error!r}")
            print(f"  FAIL {canary.__name__} raised {error!r}")
    print(f"\n{checks} canary assertions run")
    if failures:
        for item in failures:
            print(f"::error:: a gate did not judge as required: {item}")
        return 1
    print("every canary judged red exactly where it was supposed to")
    return 0


if __name__ == "__main__":
    sys.exit(main())
