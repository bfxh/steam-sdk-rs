#!/usr/bin/env python3
"""List the vendored Steam SDK files with a content hash.

These come from Valve and nothing in the crate's own test suite would go red if one of
them were edited: the generated bindings are checked in, so a changed header only shows
up the next time somebody regenerates them -- which silently changes what the wrapper is
a wrapper *of*. This manifest makes an edit, an addition or a deletion a named, blocking
difference.

Scope is measured, not assumed: 45 headers plus `steam_api.json`, the interface-version
manifest used by the binding regeneration workflow. Binaries (`.so`, `.dll`, `.lib`,
`.dylib`) are not hashed here -- they are platform artifacts, and putting them in the same
list would make an SDK update look like tampering.
"""
import hashlib
import os
import sys

SDK_ROOT = "steamworks-sys/lib/steam/public/steam"
# Measured 2026-10-14: 45 `.h` + `steam_api.json`.
MIN_FILES = 46
SUFFIXES = (".h", ".json")


def main(root=SDK_ROOT):
    if not os.path.isdir(root):
        print(f"::error vendored SDK directory {root} is missing -- cannot judge anything",
              file=sys.stderr)
        return 2
    rows = []
    for name in sorted(os.listdir(root)):
        if not name.endswith(SUFFIXES):
            continue
        path = os.path.join(root, name)
        if not os.path.isfile(path):
            continue
        digest = hashlib.sha256(open(path, "rb").read()).hexdigest()
        rows.append(f"{path.replace(os.sep, '/')} {digest}")
    if len(rows) < MIN_FILES:
        print(f"::error found {len(rows)} vendored files, expected at least {MIN_FILES} "
              f"-- the ruler is not looking at the right tree", file=sys.stderr)
        return 2
    print("\n".join(rows))
    return 0


if __name__ == "__main__":
    sys.exit(main(*(sys.argv[1:] or [])))
