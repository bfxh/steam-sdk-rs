#!/usr/bin/env python3
"""Zero tolerance for `unreachable!()` in the hand-written sources.

Every one of these sat in the fallback arm of a `match` over a `#[non_exhaustive]` Steam enum
-- `EBeginAuthSessionResult`, `EUserHasLicenseForAppResult`, `EWorkshopFileType`, the content
descriptors, `EItemUpdateStatus`, `ERemoteStoragePublishedFileVisibility`,
`ESteamAPIInitResult`, `EChatMemberStateChange`, `EAuthSessionResponse`. A Steam runtime newer
than the vendored headers can answer with a value the binding does not name, and then the
wrapper panics inside the caller's process. The crate already has a name for that answer --
`FriendState::Unknown`, `ChatEntryType::Invalid`, `PreviewKind::Unknown` -- and the fallback
arm is where it belongs.

The count is not a baseline to slide: it is zero. A comment mentioning the word does not
count, because a doc comment that describes the old panic is not a new panic -- and skipping
comment lines is what stops this ruler from reading its own documentation as a violation.
"""
import re
import sys
from pathlib import Path

PATTERN = re.compile(r"\bunreachable!\s*\(")


def violations(root):
    found = []
    for path in sorted(Path(root).rglob("*.rs")):
        if "bindings" in path.name:
            continue  # bindgen output, not ours to fix here
        in_block_comment = False
        for number, line in enumerate(
            path.read_text(encoding="utf-8", errors="replace").splitlines(), 1
        ):
            stripped = line.strip()
            # `/* ... */` blocks can span lines; a ruler that only strips `//` would read the
            # prose inside them as code, which is how a doc comment turns into a fake violation.
            if in_block_comment:
                if "*/" in stripped:
                    in_block_comment = False
                continue
            if stripped.startswith("/*"):
                if "*/" not in stripped:
                    in_block_comment = True
                continue
            if stripped.startswith("///") or stripped.startswith("//"):
                continue
            code = stripped.split("//")[0]
            if PATTERN.search(code):
                found.append(f"{path.as_posix()}:{number}: {stripped}")
    return found


def main():
    root = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("src")
    if not root.is_dir():
        print(f"::error:: {root} is not a directory -- the ruler has nothing to look at",
              file=sys.stderr)
        return 2
    found = violations(root)
    files = scanned_file_count(root)
    if files < MIN_SCANNED_FILES:
        print(
            f"::error:: scanned only {files} sources under {root}, expected at least "
            f"{MIN_SCANNED_FILES} -- an empty ledger is not a pass",
            file=sys.stderr,
        )
        return 2

    # One `<file> <count>` record per file that still has a site, which is what
    # `compare_baseline.py` judges: a new site is red, and so is a fixed one until the
    # baseline is re-recorded downward. Fixing one is the intended direction; the gate just
    # refuses to let it happen silently.
    counts = {}
    for violation in found:
        path = violation.split(":")[0]
        counts[path] = counts.get(path, 0) + 1
    for path in sorted(counts):
        print(f"{path} {counts[path]}")
    # The ledger is empty by design, so `compare_baseline.py` needs proof the ruler looked at
    # sources rather than found none: this marker is that proof and is skipped as a comment.
    print(f"#scanned {files}")
    for violation in found:
        print(f"  registered site: {violation}", file=sys.stderr)
    print(
        f"# {len(found)} site(s) across {files} scanned sources; the target is zero",
        file=sys.stderr,
    )
    return 0


MIN_SCANNED_FILES = 20


def scanned_file_count(root):
    return sum(1 for path in root.rglob("*.rs") if "bindings" not in path.name)


if __name__ == "__main__":
    sys.exit(main())
