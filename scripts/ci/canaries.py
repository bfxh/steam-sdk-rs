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


def canary_panic_stub():
    print("no_panic_on_unknown_enum.py -- unimplemented!/todo! count, prose does not")
    ruler = ROOT / CI / "no_panic_on_unknown_enum.py"
    tmp, dst = fixture(ruler)
    try:
        write(tmp / "src" / "stub.rs", "pub fn a() -> u8 {\n    unimplemented!()\n}\n")
        write(tmp / "src" / "later.rs", "pub fn b() -> u8 {\n    todo!()\n}\n")
        # Prose about a panic that used to be there is not a panic site: this is the line the
        # changelog and the identity.rs history both write, and a ruler that counted it would
        # refuse its own documentation.
        write(tmp / "src" / "notes.rs", "// the fallback used to be `unimplemented!()`\n")
        for i in range(20):  # the ruler's own floor is 20 scanned sources
            write(tmp / "src" / f"f{i}.rs", f"pub fn f{i}() {{}}\n")
        out = run_script(dst, ["src"], cwd=tmp)
        expect("a stubbed method is emitted as a record",
               0 if "src/stub.rs 1" in out.stdout else 1, 0)
        expect("a todo!() is emitted as a record",
               0 if "src/later.rs 1" in out.stdout else 1, 0)
        expect("a comment about the old panic is not a record",
               0 if "notes.rs" not in out.stdout else 1, 0)
        expect("the scan marker is emitted",
               0 if re.search(r"^#scanned \d+$", out.stdout, re.M) else 1, 0)
        measure = write(tmp / "cur.txt", out.stdout)
        expect("the comparator turns the stub records red",
               run_script(ROOT / CI / "compare_baseline.py",
                          [write(tmp / "base.txt", ""), measure]).returncode, 1)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def canary_handle_owners():
    print("handle_owners.py -- a raw interface pointer without Arc<Inner> is the only thing that counts")
    ruler = ROOT / CI / "handle_owners.py"
    tmp, dst = fixture(ruler)
    try:
        write(tmp / "src" / "bare.rs",
              "pub struct Bare {\n    iface: *mut sys::ISteamHTTP,\n    handle: u32,\n}\n")
        write(tmp / "src" / "held.rs",
              "pub struct Held {\n    iface: *mut sys::ISteamNetworkingSockets,\n"
              "    _inner: Arc<Inner>,\n}\n")
        # The trap this ruler has to avoid: a comment that talks about `Arc<Inner>` does not make the
        # wrapper own one. This is the shape that actually bit -- the owner field's own comment sits
        # inside the body, so deleting the field leaves the prose behind, and a stripper that only
        # removes the `//` marker would report the wrapper as safe.
        write(tmp / "src" / "prose.rs",
              "pub struct Prose {\n"
              "    /// carries no Arc<Inner>, which is the whole problem\n"
              "    // an Arc<Inner> would be held here\n"
              "    iface: *mut sys::ISteamUtils,\n}\n")
        # And a private struct is not a public wrapper's contract, but the hazard is the same, so the
        # rule judges `pub struct` only -- a non-pub one must not pad the count.
        write(tmp / "src" / "internal.rs",
              "struct Internal {\n    iface: *mut sys::ISteamInventory,\n}\n")
        for i in range(20):  # the ruler's own floor is 20 scanned sources
            write(tmp / "src" / f"f{i}.rs", f"pub fn f{i}() {{}}\n")
        out = run_script(dst, ["src"], cwd=tmp)
        expect("the bare wrapper is an offender",
               0 if "src/bare.rs 1" in out.stdout else 1, 0)
        expect("the comment about Arc<Inner> is stripped, so prose.rs is still an offender",
               0 if "src/prose.rs 1" in out.stdout else 1, 0)
        expect("a wrapper that owns the client is not an offender",
               0 if "held.rs" not in out.stdout else 1, 0)
        expect("the scan marker is emitted",
               0 if re.search(r"^#scanned \d+$", out.stdout, re.M) else 1, 0)
        expect("the coverage half of the claim is stated (44-shape style count)",
               0 if "out of 3 that hold a raw interface pointer" in out.stderr else 1, 0)
        measure = write(tmp / "cur.txt", out.stdout)
        expect("the comparator turns the offenders red against an empty ledger",
               run_script(ROOT / CI / "compare_baseline.py",
                          [write(tmp / "base.txt", ""), measure]).returncode, 1)
        # a ruler that scanned nothing must not report "zero offenders" as a pass
        probe = floor_probe(dst, "floor_probe.py", "if files < 20 or shapes == 0:",
                            "if files < 5000 or shapes == 0:")
        expect("too little scanned is refused rather than passed",
               run_script(probe, ["src"], cwd=tmp).returncode, 2)
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def canary_cstring_unwraps():
    print("cstring_unwraps.py -- which CString::new(..) panics, and which only looks like one")
    ruler = ROOT / CI / "cstring_unwraps.py"
    tmp, dst = fixture(ruler)
    try:
        write(tmp / "src" / "caller.rs",
              "pub fn set(value: &str) -> CString { CString::new(value).unwrap() }\n")
        write(tmp / "src" / "safe_lit.rs",
              'pub fn fixed() -> CString { CString::new("Handle was dropped").unwrap() }\n')
        # The one literal shape that *does* fail: an escaped nul is the input CString::new rejects.
        write(tmp / "src" / "nul_lit.rs",
              'pub fn sneaky() -> CString { CString::new("a\\0b").unwrap() }\n')
        write(tmp / "src" / "expect.rs",
              'pub fn with_msg(value: &str) -> CString {\n'
              '    CString::new(value).expect("no nul in the name")\n}\n')
        # A substitution is not a panic; counting it would let a safe fallback be reported as debt.
        write(tmp / "src" / "or_default.rs",
              "pub fn fallback(value: &str) -> CString {\n"
              "    CString::new(value).unwrap_or_default()\n}\n")
        # And prose about the pattern must not become a site -- the changelog is full of it.
        write(tmp / "src" / "prose.rs",
              "// Upstream used CString::new(name).unwrap() here.\n"
              "pub fn clean(value: &str) -> Result<CString, ()> { CString::new(value).map_err(|_| ()) }\n")
        for i in range(20):  # the ruler's own floor is 20 scanned sources
            write(tmp / "src" / f"f{i}.rs", f"pub fn f{i}() {{}}\n")
        out = run_script(dst, ["src"], cwd=tmp)
        expect("a caller-supplied unwrap is a site",
               0 if "src/caller.rs 1" in out.stdout else 1, 0)
        expect("a literal with no nul is not a site",
               0 if "safe_lit.rs" not in out.stdout else 1, 0)
        expect("an escaped nul inside a literal is a site",
               0 if "src/nul_lit.rs 1" in out.stdout else 1, 0)
        expect("expect(..) counts the same as unwrap()",
               0 if "src/expect.rs 1" in out.stdout else 1, 0)
        expect("unwrap_or_default is not a panic",
               0 if "or_default.rs" not in out.stdout else 1, 0)
        expect("a comment about the pattern is not a site",
               0 if "prose.rs" not in out.stdout else 1, 0)
        expect("the coverage half of the claim is stated",
               0 if "of 4 fallible CString conversions" in out.stderr else 1, 0)
        measure = write(tmp / "cur.txt", out.stdout)
        expect("the comparator turns the sites red against an empty ledger",
               run_script(ROOT / CI / "compare_baseline.py",
                          [write(tmp / "base.txt", ""), measure]).returncode, 1)
        probe = floor_probe(dst, "floor_probe.py", "if files < 20 or total == 0:",
                            "if files < 5000 or total == 0:")
        expect("too little scanned is refused rather than passed",
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
        # Set the floor by pattern, not by the value the repository happens to carry: this fixture
        # held three ledgers and the literal replacement `"MIN_BASELINES = 10"` stopped matching the
        # day the inventory grew to eleven, so the fixture ran against the real floor and every
        # assertion in it exited 2 -- a canary that fails because a constant moved is not testing the
        # rule it claims to test.
        script = re.sub(r"^MIN_BASELINES = \d+$", "MIN_BASELINES = 3", script, flags=re.M)
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

        floor = re.sub(r"^MIN_BASELINES = \d+$", "MIN_BASELINES = 40", read(probe), flags=re.M)
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


NOISE_FIXTURE = """--- a/steamworks-sys/src/macos_bindings.rs
+++ b/steamworks-sys/src/macos_bindings.rs
@@ -1,3 +1,3 @@
-const _LIBCPP_VERSION: u32 = 190000;
+const _LIBCPP_VERSION: u32 = 200000;
+const __MAC_26_0: u32 = 260000;
-const _STATIC_INLINE_UCRT_FUNCTIONS: u32 = 1;
+const _STATIC_INLINE_UCRT_FUNCTIONS: u32 = 0;
"""

STRUCT_FIXTURE = """--- a/steamworks-sys/src/windows_bindings.rs
+++ b/steamworks-sys/src/windows_bindings.rs
@@ -17499,3 +17499,7 @@
-pub struct SteamNetworkingFakeIPResult_t {
-    _unused: [u8; 0],
-}
+pub struct SteamNetworkingFakeIPResult_t {
+    pub m_eResult: EResult,
+    pub m_unIP: u32,
+}
"""

COLUMN_ONE_FIXTURE = """--- a/x
+++ b/x
@@ -1 +1 @@
+SteamAPI_ISteamNetworkingSockets_GetFakeIP(self: *mut c_void);
"""


def canary_bindings_drift():
    """The bindings classifier must separate runner noise from a real surface change -- and it must
    see a surface line that starts immediately after the diff sign, which is the case the inline
    `^[+-][^+-].*(...)` pattern it replaced ate one character past."""
    print("bindings_drift.py -- noise is noise, and a struct is not")
    ruler = ROOT / CI / "bindings_drift.py"

    def verdict(diff):
        done = subprocess.run([sys.executable, str(ruler)], input=diff, capture_output=True,
                              text=True, encoding="utf-8", errors="replace")
        return " ".join(done.stdout.split())

    global checks
    for label, diff, want in (
        ("a bindings diff of pure libc++/ucrt noise", NOISE_FIXTURE,
         "changed=true api_changed=false"),
        ("a callback struct gaining its real fields", STRUCT_FIXTURE,
         "changed=true api_changed=true"),
        ("a surface line whose token starts at column one", COLUMN_ONE_FIXTURE,
         "changed=true api_changed=true"),
        ("an empty diff", "", "changed=false api_changed=false"),
    ):
        got = verdict(diff)
        checks += 1
        if got == want:
            print(f"  ok   {label}: {got}")
        else:
            print(f"  FAIL {label}: got [{got}] want [{want}]")
            failures.append(f"bindings_drift: {label}")


HEADER_FIXTURE = """// line one
// the phrase that matters is right here on line two

// line four says something else entirely about widgets and gadgets
// and this sentence keeps talking on the line after it
// deprecated - "this sentence is quoted inside the header comment"
"""
OTHER_FIXTURE = "// a sentence that only lives in the other header, about trampolines\n"


def canary_header_citations():
    """The header-citation ruler must judge each outcome it claims, and the two short citation forms
    it exists to parse. Proven against fixture headers, so every assertion is about the ruler rather
    than about whichever line numbers the real SDK happens to put text on."""
    print("header_citations.py -- span drift, blanks, ranges, shorthand, wrong header, empty scan")
    root = Path(tempfile.mkdtemp(prefix="hc-canary-"))
    sdk = root / "sdk"
    src = root / "src"
    write(sdk / "fake.h", HEADER_FIXTURE)
    write(sdk / "other.h", OTHER_FIXTURE)
    ruler = "ruler.py"
    shutil.copy(ROOT / CI / "header_citations.py", root / ruler)

    def run(body):
        write(src / "lib.rs", body)
        done = subprocess.run(
            [sys.executable, ruler, "src", "sdk"],
            cwd=str(root), capture_output=True, text=True, encoding="utf-8", errors="replace",
        )
        return done.returncode, done.stdout

    global checks
    for label, body, want in (
        ("phrase inside the cited span", '/// `fake.h:2` says "the phrase that matters"\n', 0),
        ("the same phrase one line off", '/// `fake.h:4` says "the phrase that matters"\n', 1),
        ("a span of blank lines", '/// `fake.h:3` says "the phrase that matters"\n', 1),
        ("a span past the end of the header", '/// `fake.h:99` says "the phrase that matters"\n', 1),
        ("the `h:` shorthand is judged, not skipped",
         '/// `fake.h:2` then `h:4` -- "line four says something else"\n', 0),
        ("the `h:` shorthand pointing at the wrong line",
         '/// `fake.h:2` then `h:2` -- "line four says something else"\n', 1),
        ("a phrase belonging to a header this block never cites",
         '/// `fake.h:2` quotes "a sentence that only lives in the other header"\n', 1),
        ("the author's own summary in quotes is debt, not a verdict",
         '/// `fake.h:2` is "a paraphrase no header contains verbatim at all"\n', 0),
        # Paired so the inner-quote case cannot pass by matching nothing: the same phrase cited one line
        # early is verbatim in the header but outside the span, which the gate must call red.
        ("an inner quotation mark in Steam's comment is still matchable",
         '/// `fake.h:6` says "deprecated - this sentence is quoted inside the header comment"\n', 0),
        ("that same phrase, cited one line early, is red",
         '/// `fake.h:5` says "deprecated - this sentence is quoted inside the header comment"\n', 1),
        ("a one-word emphasis quote is neither judged nor debt",
         '/// `fake.h:2` sets "colour", nothing else quoted\n', 0),
        # Both halves of this pair exist because the real ruler once joined header lines with their
        # `//` markers still attached, which makes any phrase Steam wrapped across two lines
        # unmatchable: measured on the repository, 25 of 93 verbatim quotes were invisible to it, and
        # the WRONG-PLACE verdicts for those quotes were suppressed along with them.
        ("a phrase wrapped over two header lines matches",
         '/// `fake.h:4-5` says "widgets and gadgets and this sentence keeps talking"\n', 0),
        ("the same wrapped phrase cited one line short is red",
         '/// `fake.h:4` says "widgets and gadgets and this sentence keeps talking"\n', 1),
    ):
        code, _ = run(body)
        expect(f"header_citations: {label}", code, want)

    _, output = run('/// `fake.h:2` is "a paraphrase no header contains verbatim at all"\n')
    checks += 1
    if "src/lib.rs~unjudged 1" in output and "src/lib.rs~undeclared 0" in output:
        print("  ok   a paraphrase is one unit of unjudged debt and anchors the block (undeclared 0)")
    else:
        print(f"  FAIL paraphrase columns wrong: [{output.strip()}]")
        failures.append("header_citations: paraphrase columns")

    _, output = run('/// the flag lives at `fake.h:2` with nothing quoted\n')
    checks += 1
    if "src/lib.rs~undeclared 1" in output and "src/lib.rs~unjudged 0" in output:
        print("  ok   a phrase-less citation is registered as one undeclared span")
    else:
        print(f"  FAIL undeclared column wrong: [{output.strip()}]")
        failures.append("header_citations: undeclared column")

    shutil.rmtree(src)
    done = subprocess.run(
        [sys.executable, ruler, "gone-src", "sdk"],
        cwd=str(root), capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    expect("header_citations: a scan with no sources at all", done.returncode, 2)
    shutil.rmtree(root, ignore_errors=True)


PROSE_ONLY_YAML = """# An escape hatch is a line the runner executes. This file executes nothing: every mention of
# `|| true` below is prose explaining why the step stopped using it, and prose that names
# `2>/dev/null` is not a hole in the pipeline.
name: prose-only
'on': push
"""

ONE_HATCH_YAML = """# prose again: the old form was `cargo geiger || true` with no judgement of the output,
# and `2>/dev/null` swallowed a missing interpreter.
name: one-hatch
'on': push
jobs:
  advisory:
    run: |
      cargo geiger > geiger.json || true
      python3 judge.py geiger.json
"""

TRAILING_YAML = """name: trailing-comment-counts
'on': push
jobs:
  advisory:
    run: |
      cargo audit --json || true   # the old fallback here was 2>/dev/null, which is worse
"""


def canary_shell_hatches():
    """The hatch counter must count the lines a runner executes and nothing else.

    Measured on the real repository 2026-10-18: 4 of the 9 registered hatches were comment prose --
    this repository's own habit of writing out `|| true` to explain why a step no longer uses it.
    Left alone, that makes the ledger a vocabulary test: reword a comment and the count moves, while
    a genuine new hatch can hide behind the prose it is described in.
    """
    print("shell_hatches.py -- prose is not a hatch, trailing comments are, an all-prose scan reports")
    root = Path(tempfile.mkdtemp(prefix="hatch-canary-"))
    ruler = root / "shell_hatches.py"
    shutil.copyfile(ROOT / CI / "shell_hatches.py", ruler)
    global checks

    def run(text):
        wf = root / "wf"
        shutil.rmtree(wf, ignore_errors=True)
        write(wf / "a.yml", text)
        done = run_script(ruler, ["wf"], cwd=root)
        return done.returncode, done.stdout

    code, out = run(PROSE_ONLY_YAML)
    expect("hatch counter: prose-only file is an empty scan, not a green one", code, 2)

    code, out = run(ONE_HATCH_YAML)
    checks += 1
    if code == 0 and "wf/a.yml 1" in out:
        print("  ok   hatch counter: one code-line hatch, two prose mentions ignored")
    else:
        print(f"  FAIL hatch counter counted prose (want 'wf/a.yml 1', exit 0): exit={code} "
              f"[{out.strip()}]")
        failures.append("hatch counter: prose counted")

    code, out = run(TRAILING_YAML)
    checks += 1
    if code == 0 and "wf/a.yml 2" in out:
        print("  ok   hatch counter: a trailing comment after code still counts both patterns")
    else:
        print(f"  FAIL hatch counter skipped a trailing comment (want 'wf/a.yml 2'): exit={code} "
              f"[{out.strip()}]")
        failures.append("hatch counter: trailing comment missed")

    shutil.rmtree(root, ignore_errors=True)


CANARIES = (canary_comparator, canary_panic_ledger, canary_panic_stub, canary_handle_owners,
            canary_cstring_unwraps,
            canary_doc_pointers,
            canary_baseline_files,
            canary_api_coverage, canary_bindings_drift, canary_header_citations,
            canary_shell_hatches)


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
