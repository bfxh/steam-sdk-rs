#!/usr/bin/env python3
"""Count, per Steam interface, which binding functions the wrapper actually calls.

The unit is a *binding function name*: `SteamAPI_ISteamUGC_SetItemTags` is one entry, and
overloads get their own suffixed names (`..._CreateQueryAllUGCRequestPage` / `...Cursor`), so
a name appears exactly once on each side of the comparison. An earlier ruler matched the C++
header's method names against these instead and reported `CreateQueryAllUGCRequest` as never
called -- the wrapper does call it, through both suffixed names. Header name != binding name,
so neither side is guessed here.

Two numbers per interface go into the baseline:

  * `<iface>.declared` -- what the bindings expose. It moves when the SDK is updated, and a
    silent SDK change is worth a deliberate re-record.
  * `<iface>.called`   -- what `src/` calls. This is the wrapper's coverage ledger, and like
    every other baseline here it is strict in both directions: coverage may not drop without
    somebody saying so in a commit message.

Printed as `<key> <value>` lines, sorted, for `compare_baseline.py`.
"""
import pathlib
import re
import sys

BINDINGS = pathlib.Path("steamworks-sys") / "src" / "windows_bindings.rs"
SOURCE_ROOT = pathlib.Path("src")

# Floors on the parse. A ruler that finds almost nothing must fail rather than report a
# coverage figure of zero for every interface.
MIN_INTERFACES = 25
MIN_TOTAL_DECLARED = 500

DECLARED = re.compile(r"pub fn SteamAPI_(ISteam\w+?)_(\w+)")
CALLED = re.compile(r"SteamAPI_(ISteam\w+?)_(\w+)")


def strip_comments(text):
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return "\n".join(line.split("//")[0] for line in text.splitlines())


def strip_test_modules(text):
    lines = text.splitlines()
    kept = []
    skip_until_close = False
    pending = False
    for line in lines:
        if skip_until_close:
            if line == "}":
                skip_until_close = False
            continue
        if pending:
            pending = False
            if re.match(r"^mod \w+\s*\{$", line):
                skip_until_close = True
                continue
        if line.strip() == "#[cfg(test)]":
            pending = True
        kept.append(line)
    return "\n".join(kept)


def code_only(text):
    """Drop comments and the bodies of file-scope `#[cfg(test)]` modules.

    Comments matter because writing "we never call `RequestUGCDetails`" in a doc comment would
    otherwise count as covering it -- the same way a `#[test]` pushed onto the tail of a comment
    line stopped being a test in an earlier round of this repository's tooling.

    Test modules matter because a call from a test is not coverage of the wrapper users see. The
    scope rule is the one rustfmt guarantees for a file-scope module: `mod name {` at column 0,
    closed by a line that is exactly `}`. A `#[cfg(test)]` on anything else (an item inside a
    function, say) is left alone rather than guessed at, and `test_only_calls` below reports if
    such a call is ever found, so the assumption cannot fail silently.
    """
    return strip_test_modules(strip_comments(text))


def test_only_calls(text):
    """Call sites that live inside a `#[cfg(test)]` module, i.e. coverage this ruler refuses
    to count. Non-empty means the file-scope assumption above was wrong for this file."""
    whole = CALLED.findall(strip_comments(text))
    exposed = CALLED.findall(code_only(text))
    return sorted(set(whole) - set(exposed))


def main(bindings_path=BINDINGS, source_root=SOURCE_ROOT):
    try:
        raw_bindings = bindings_path.read_text(encoding="utf-8", errors="replace")
    except OSError as error:
        print(
            f"::error:: cannot read the bindings at {bindings_path} ({error}) -- without the "
            f"declared set every interface would report zero coverage, which is not a verdict",
            file=sys.stderr,
        )
        return 2
    text = code_only(raw_bindings)
    declared = {}
    for interface, name in DECLARED.findall(text):
        declared.setdefault(interface, set()).add(name)

    called = {interface: set() for interface in declared}
    for path in sorted(source_root.rglob("*.rs")):
        raw = path.read_text(encoding="utf-8", errors="replace")
        # A file that is itself test material (`ugc_tests.rs`, `lib_tests.rs`, `tests.rs`) is
        # not a wrapper, so it is skipped whole. `test_only_calls` above cannot see those: they
        # carry no `#[cfg(test)]` inside, because the attribute sits on the `mod` declaration.
        if re.search(r"(^|_)(tests?|probe)\.rs$", path.name):
            if CALLED.search(strip_comments(raw)):
                print(
                    f"::error:: {path} is test material but calls a Steam interface -- decide "
                    "which side it belongs to before trusting the ledger",
                    file=sys.stderr,
                )
                return 2
            continue
        source = code_only(raw)
        for interface, name in CALLED.findall(source):
            if interface in called:
                called[interface].add(name)
        # A call from test code is not coverage of the public wrapper. `code_only` removes
        # file-scope test modules; if any call site was removed that way, say so instead of
        # letting the scope assumption above silently change the number.
        hidden = test_only_calls(raw)
        if hidden:
            print(
                f"::error:: {path} calls {len(hidden)} interface function(s) only from test code "
                f"({hidden[:3]}) -- coverage means 'a wrapper the crate exposes calls it', so this "
                f"ruler cannot answer until those calls move into a wrapper",
                file=sys.stderr,
            )
            return 2
            # A name that is not declared here is not coverage: it either comes from another
            # platform's bindings or is a typo, and neither counts as a wrapped call.

    if len(declared) < MIN_INTERFACES:
        print(
            f"::error:: parsed {len(declared)} interfaces from {bindings_path}, expected at "
            f"least {MIN_INTERFACES} -- the ruler is looking at the wrong file",
            file=sys.stderr,
        )
        return 2
    total = sum(len(names) for names in declared.values())
    if total < MIN_TOTAL_DECLARED:
        print(
            f"::error:: parsed {total} declared functions, expected at least "
            f"{MIN_TOTAL_DECLARED} -- an empty declaration set makes every coverage line read 0",
            file=sys.stderr,
        )
        return 2

    for interface in sorted(declared):
        print(f"{interface}.declared {len(declared[interface])}")
    for interface in sorted(declared):
        live = len(called[interface] & declared[interface])
        print(f"{interface}.called {live}")
    print(
        f"# {len(declared)} interfaces, {total} declared, "
        f"{sum(len(called[i] & declared[i]) for i in declared)} called",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    # Optional overrides exist so the controls can point the ruler at a different tree; the
    # CI step runs it with no arguments at all.
    args = sys.argv[1:]
    sys.exit(
        main(
            pathlib.Path(args[0]) if args else BINDINGS,
            pathlib.Path(args[1]) if len(args) > 1 else SOURCE_ROOT,
        )
    )
