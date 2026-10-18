#!/usr/bin/env python3
"""Count the public wrappers that hold a raw Steam interface but do not keep the client alive.

`Inner` owns the `Manager`, and `Manager::drop` is what calls `SteamAPI_Shutdown()`
(`src/lib.rs`). So any wrapper that carries a `*mut sys::ISteam*` *without* an `Arc<Inner>` can outlive
the client it came from: safe code drops the `Client`, Steam shuts down, and the wrapper's own methods
-- including its `Drop`, four of which call Steam back -- still hold the interface pointer. That is the
ordering hazard, and it is checkable statically, which is why this is a gate rather than a note.

The rule it judges: a `pub struct` whose body has a field of type `*mut sys::ISteam...` and no
`Arc<Inner>` is an offender, one count per struct, keyed by file. Comments are stripped before the
bodies are read -- a doc comment that *talks about* `Arc<Inner>` (several do, including the one that
explains this rule) must not become the reason a wrapper is considered safe.

The generated bindings are skipped for the same reason every other ruler here skips them: they are
bindgen output, not this crate's design.
"""
import re
import sys
from pathlib import Path

POINTER = re.compile(r"\*mut\s+sys::ISteam\w+")
OWNER = re.compile(r"\bArc<\s*Inner\s*>")
STRUCT = re.compile(r"\bpub\s+struct\s+(\w+)\s*\{")


def strip_comments(text):
    """Blank `//` lines and `/* ... */` blocks, keeping the line count stable.

    Blanking the whole line is the point. The first version of this only removed the `//` marker and
    left the prose, so a comment inside a struct body that *mentions* `Arc<Inner>` -- which is exactly
    what the owner field's own comment does here -- read as an owner, and deleting the field left the
    ruler reporting zero offenders. A gate that cannot see its own fix being undone is not a gate.
    """
    out = []
    in_block = False
    for line in text.splitlines():
        stripped = line.strip()
        if in_block:
            if "*/" in stripped:
                in_block = False
            out.append("")
            continue
        if stripped.startswith("/*"):
            if "*/" not in stripped:
                in_block = True
            out.append("")
            continue
        if stripped.startswith("//"):
            out.append("")
            continue
        # A trailing comment is prose too, but only cut it when it sits outside a string literal.
        marker = line.find("//")
        if marker > 0 and line[:marker].count('"') % 2 == 0:
            line = line[:marker]
        out.append(line)
    return "\n".join(out)


def struct_bodies(text):
    """Yield (name, body) for each `pub struct Name { ... }`."""
    for match in STRUCT.finditer(text):
        depth = 0
        start = match.end() - 1
        for index in range(start, len(text)):
            if text[index] == "{":
                depth += 1
            elif text[index] == "}":
                depth -= 1
                if depth == 0:
                    yield match.group(1), text[start + 1 : index]
                    break


def offenders(root):
    found = []
    shapes = 0
    for path in sorted(Path(root).rglob("*.rs")):
        if "bindings" in path.name:
            continue
        text = strip_comments(path.read_text(encoding="utf-8", errors="replace"))
        for name, body in struct_bodies(text):
            if not POINTER.search(body):
                continue
            shapes += 1
            if not OWNER.search(body):
                found.append(f"{path.as_posix()} {name}")
    return found, shapes


def main():
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("src")
    if not root.is_dir():
        print(f"::error:: {root} is not a directory -- the ruler has nothing to look at",
              file=sys.stderr)
        return 2
    found, shapes = offenders(root)
    files = sum(1 for path in root.rglob("*.rs") if "bindings" not in path.name)
    if files < 20 or shapes == 0:
        print(f"::error:: scanned {files} sources and {shapes} raw-interface wrappers; an empty scan "
              "is not a pass", file=sys.stderr)
        return 2

    counts = {}
    for record in found:
        counts[record.split()[0]] = counts.get(record.split()[0], 0) + 1
    for path in sorted(counts):
        print(f"{path} {counts[path]}")
    print(f"#scanned {files}")
    print(
        f"# {len(found)} wrapper(s) hold a raw interface pointer with no Arc<Inner>, out of {shapes} "
        "that hold a raw interface pointer at all",
        file=sys.stderr,
    )
    for record in found:
        print(f"  offender: {record}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
