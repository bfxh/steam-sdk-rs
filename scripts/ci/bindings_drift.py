#!/usr/bin/env python3
"""Classify a regenerated-bindings diff: runner noise, or a real change to the API surface?

`Rebuild bindings` regenerates the four bindings files on four native runners and must tell a human
whether the drift is worth committing. It cannot ask "did anything change" -- a regeneration always
changes something. Measured: one plain regeneration produced a 12 KB diff whose content was macOS
libc++/SDK version macros (`_LIBCPP_*`, `__MAC_26_*`) and Windows ucrt constants, with no Steam API
line in it at all. So the question is *which kind* of line moved, and this script answers it.

Input: a unified diff on stdin (what `git diff -- steamworks-sys/src` prints).
Output: two `KEY=value` lines for `$GITHUB_OUTPUT` on stdout, one human-readable summary on stderr.

The pattern is applied in two stages on purpose -- select changed content lines first, then match
inside them. The single-stage form this repository shipped with,
`^[+-][^+-].*(SteamAPI_|ISteam)`, silently eats the first character of the payload, so a line whose
matched token starts immediately after the diff sign cannot match: measured against a fixture, it
missed `+pub struct SteamNetworkingFakeIPResult_t {` -- the exact shape a newly materialised struct
arrives in -- while counting the field lines under it. A classifier that under-reads the surface case
is worse than no classifier, because it reports "environment noise" about the one drift a human needs
to see.
"""
import re
import sys

# Lines `git diff` prints that are not content: the `+++`/`---` file headers. Everything else that
# starts with a sign is an added or removed source line.
CONTENT = re.compile(r"^[-+][^-+]")
# Anything here means the bindings' API surface moved, not just the runner's C++ standard library.
SURFACE = re.compile(
    r"SteamAPI_"              # a flat-API function
    r"|SteamGameServerAPI_"
    r"|ISteam\w+"              # an interface name in any position
    r"|pub (struct|union) \w+(_t|Callback_t)?\b"   # a type declaration, new or removed
    r"|pub (unsafe )?fn \w+\("  # a method inside an impl block
    r"|\bm_[A-Za-z0-9_]+\s*:"   # a callback/struct field, Steam's own member naming
    r"|k_(?:E|cch|nMax|Steam)\w*"  # a Steam constant bindgen was told to allowlist
)
NOISE = re.compile(
    r"^[-+][^-+]*\b(_LIBCPP_|__MAC_\d|__ANDROID|_STATIC_INLINE_UCRT|__cpp_|_CRT_|__GNUC)"
)


def classify(diff_text):
    """Return (changed, surface_hits, noise_hits, the surface lines themselves)."""
    changed = False
    surface, noise, hits = 0, 0, []
    for line in diff_text.split("\n"):
        if line.startswith(("+++", "---")) or not line or line[0] not in "+-":
            continue
        changed = True
        if not CONTENT.match(line):
            continue
        if NOISE.match(line):
            noise += 1
            continue
        if SURFACE.search(line[1:]):
            surface += 1
            if len(hits) < 8:
                hits.append(line[:100])
    return changed, surface, noise, hits


def main(argv):
    diff_text = sys.stdin.read()
    changed, surface, noise, hits = classify(diff_text)
    if not changed:
        print("changed=false")
        print("api_changed=false")
        print("bindings diff: empty", file=sys.stderr)
        return 0
    print("changed=true")
    print(f"api_changed={'true' if surface else 'false'}")
    print(
        f"bindings diff: {surface} API-surface line(s), {noise} runner-noise line(s)",
        file=sys.stderr,
    )
    for line in hits:
        print(f"  surface: {line}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
