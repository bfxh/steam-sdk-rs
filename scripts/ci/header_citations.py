#!/usr/bin/env python3
"""Judge every SDK-header line citation in ``src/`` against the vendored headers.

Why this exists: two units in a row shipped citations whose numbers were wrong. ``fake_ip.rs`` had
three spans sitting 2-6 lines off and ``fake_udp.rs`` had two off by one, all of them typed from a
reading rather than re-read after the fact, and nothing in the gate chain could see it, because the
hand-written citation tables only cover the two files that bother to carry one. Measured before this
script was written: 119 citation spans across 17 files, of which exactly 2 files were checked by
anything.

Three citation forms appear in the prose and all three are parsed::

    isteamnetworkingsockets.h:865     steamnetworkingfakeip.h:123-125
    h:910-911                         :866

The short forms inherit the header most recently named before them in the same comment block, which
is how the sentences use them; a short form with nothing named earlier in its block is counted as
unjudged debt rather than guessed at.

Judging happens per *block*, not per citation, and that choice is measured too: pairing each citation
with its nearest quote made the prototype report one phrase against all three citations of a block
that held three citations and one quote -- a heuristic failure pretending to be a documentation
failure. The union rule instead says: every quoted phrase in a block that is verbatim text of header
H must fall inside at least one span this block cites from H. Weaker per citation, and it cannot fail
because of a guess about which number a quote belonged to.

Five outcomes:

    RANGE         cited past the end of the header, or the header is not in the tree -- red
    BLANK         every line of the cited span is blank -- red
    WRONG-PLACE   the phrase is verbatim in a cited header but outside every span cited for it -- red
    WRONG-HEADER  the phrase is verbatim in another vendored header and in none of the cited ones -- red
    UNJUDGED      no phrase, or a phrase no header contains verbatim (the author's own wording inside
                  quotation marks) -- not judged, counted per file, and the count may only shrink

The unjudged column exists because 41 of the 119 phrases matched and 44 did not; scoring the second
group as verified would be the same lie as a green exit code from a ruler that read nothing.

Output is ``<path> <red>`` and ``<path>~unjudged <n>`` for every source file that cites a header,
plus ``#scanned <files>`` so an empty scan cannot pass.
"""
import os
import re
import sys

SRC = "src"
SDK_DIR = os.path.join("steamworks-sys", "lib", "steam", "public", "steam")
CITE = re.compile(r"\b([A-Za-z_]+\.h|h):(\d+)(?:-(\d+))?")
BARE = re.compile(r"(?<![\w.]):(\d+)(?:-(\d+))?")
QUOTE = re.compile(r'"([^"]{4,})"')
COMMENT = re.compile(r"^\s*/{2,3}!?\s?")
ELISION = re.compile(r"\.\.\.|…")
# Shorter than this and the "phrase" is a common English fragment: matching it proves nothing about
# a line number, and a gate that approves by accident is worse than one that stays quiet.
MIN_PHRASE = 10


def norm(text):
    """Strip Markdown emphasis and collapse whitespace: prose decorates, headers do not."""
    return re.sub(r"\s+", " ", text.replace("`", "").replace("*", "")).strip().lower()


def load_headers(sdk_dir):
    """Every vendored header as (lines, normalised-whole-text), keyed by file name."""
    headers = {}
    for name in sorted(os.listdir(sdk_dir)):
        if not name.endswith(".h"):
            continue
        with open(os.path.join(sdk_dir, name), encoding="utf-8", errors="replace") as handle:
            lines = handle.read().splitlines()
        headers[name] = (lines, norm(" ".join(lines)))
    return headers


def citation_spans(block):
    """Map each cited header to the (first, last) pairs this block cites it at."""
    named = list(CITE.finditer(block))
    real = [item for item in named if item.group(1) != "h"]
    spans = {}
    for item in named:
        name = item.group(1)
        if name == "h":
            earlier = [c for c in real if c.start() < item.start()]
            if not earlier:
                continue
            name = earlier[-1].group(1)
        spans.setdefault(name, []).append((int(item.group(2)), int(item.group(3) or item.group(2))))
    for item in BARE.finditer(block):
        earlier = [c for c in real if c.start() < item.start()]
        if not earlier:
            continue
        spans.setdefault(earlier[-1].group(1), []).append(
            (int(item.group(1)), int(item.group(2) or item.group(1)))
        )
    return spans


def comment_blocks(lines):
    index = 0
    while index < len(lines):
        if not lines[index].strip().startswith("//"):
            index += 1
            continue
        start = index
        while index < len(lines) and lines[index].strip().startswith("//"):
            index += 1
        yield start + 1, " ".join(COMMENT.sub("", line) for line in lines[start:index])


def resolve(line, block, headers, sdk_dir):
    """Turn one block's citations into checked spans, returning the red verdicts it earned."""
    problems = []
    spans = {}
    for name, pairs in citation_spans(block).items():
        entry = headers.get(name)
        if entry is None:
            problems.append(f"{line}: {name} is not a header in {sdk_dir}")
            continue
        for first, last in pairs:
            header_lines = entry[0]
            if first < 1 or last > len(header_lines):
                problems.append(
                    f"{line}: {name}:{first}-{last} is past the end of the header "
                    f"({len(header_lines)} lines)"
                )
                continue
            window = header_lines[first - 1 : last]
            if not any(text.strip() for text in window):
                problems.append(
                    f"{line}: {name}:{first}-{last} is blank, which cannot be what the "
                    "sentence cites"
                )
                continue
            spans.setdefault(name, []).append(norm(" ".join(window)))
    return spans, problems


def judge_quotes(line, block, spans, headers):
    """Every phrase in the block that a header could vouch for, judged against the cited spans."""
    problems = []
    unjudged = 0
    for quote in QUOTE.finditer(block):
        fragments = [
            piece for piece in (norm(x) for x in ELISION.split(quote.group(1)))
            if len(piece) >= MIN_PHRASE
        ]
        if not fragments:
            unjudged += 1
            continue
        if any(
            all(any(piece in span for span in joined) for piece in fragments)
            for joined in spans.values()
        ):
            continue
        cited_but_moved = [
            name for name, (_, whole) in headers.items()
            if name in spans and all(piece in whole for piece in fragments)
        ]
        if cited_but_moved:
            problems.append(
                f"{line}: {quote.group(1)[:60]!r} is text of "
                f"{', '.join(cited_but_moved)} but sits outside every span cited for it here"
            )
            continue
        uncited = sorted(
            name for name, (_, whole) in headers.items()
            if name not in spans and all(piece in whole for piece in fragments)
        )
        if len(uncited) > 1:
            # The fragment is boilerplate several headers share ("not allowed", "invalid parameter"),
            # so it attributes to none of them and cannot say anything about a line number. Calling
            # this a finding would teach the reader to ignore the gate's output, which is worse than
            # not judging it: measured on today's corpus, one such fragment accounted for a quarter
            # of all reds the first version produced.
            unjudged += 1
        elif uncited:
            problems.append(
                f"{line}: {quote.group(1)[:60]!r} is verbatim in {', '.join(uncited)}, "
                "which this block does not cite"
            )
        else:
            unjudged += 1
    return problems, unjudged


def scan(path, headers, sdk_dir):
    with open(path, encoding="utf-8", errors="replace") as handle:
        lines = handle.read().splitlines()
    problems = []
    unjudged = 0
    spans = 0
    cited = False
    for line, block in comment_blocks(lines):
        found, resolved = resolve(line, block, headers, sdk_dir)
        if not found and not resolved:
            continue
        cited = True
        spans += sum(len(pairs) for pairs in citation_spans(block).values())
        problems.extend(resolved)
        found_quotes, extra = judge_quotes(line, block, found, headers)
        problems.extend(found_quotes)
        unjudged += extra
    return cited, problems, unjudged, spans


def main():
    # Two optional arguments exist for the canary fixtures; the CI step runs it with none, on the
    # repository's own `src/` and vendored SDK.
    src = sys.argv[1] if len(sys.argv) > 1 else SRC
    sdk_dir = sys.argv[2] if len(sys.argv) > 2 else SDK_DIR
    if not os.path.isdir(sdk_dir):
        print(f"::error {sdk_dir} is missing, so no citation can be checked against anything",
              file=sys.stderr)
        return 2
    headers = load_headers(sdk_dir)
    if not headers:
        print(f"::error {sdk_dir} holds no .h files -- an empty scan is not a pass", file=sys.stderr)
        return 2

    sources = []
    for directory, _, names in os.walk(src):
        for name in sorted(names):
            if name.endswith(".rs"):
                sources.append(os.path.join(directory, name).replace(os.sep, "/"))

    red = 0
    debt = 0
    rows = 0
    span_total = 0
    messages = []
    for path in sources:
        cited, found, unjudged, spans = scan(path, headers, sdk_dir)
        if not cited:
            continue
        print(f"{path} {len(found)}")
        print(f"{path}~unjudged {unjudged}")
        rows += 2
        span_total += spans
        red += len(found)
        debt += unjudged
        messages.extend(f"{path} {problem}" for problem in found)

    print(f"#scanned {rows}", file=sys.stderr)
    for message in messages:
        print(f"::error:: {message}", file=sys.stderr)
    print(
        f"header citations: {rows // 2} files cite the SDK, {span_total} spans, "
        f"{red} judged wrong, {debt} unjudged and registered (may only shrink)",
        file=sys.stderr,
    )
    if red:
        return 1
    if rows == 0:
        print("::error:: no source cites any SDK header -- that is not a pass", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
