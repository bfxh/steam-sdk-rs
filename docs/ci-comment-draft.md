CI on these PRs is parked at **action_required** — the workflow runs were created but
never executed (this is a fork PR from a first-time contributor), so there are no check
runs yet. They need a maintainer to approve a run once; after that, pushes from this
contributor run automatically.

While waiting, I ran the same commands locally on one matrix cell
(rustc 1.99.0, windows-gnu, `-j 1`, `CARGO_INCREMENTAL=0`, `RUSTFLAGS="-C debuginfo=0"`):

- `cargo build --lib` exits 0 on all nine branches (#350-#358).
- #355 additionally: `cargo fmt --all -- --check` clean, and `cargo test` reproduces the
  fingerprint in its description — `7 passed; 0 failed; 13 ignored` for the lib tier plus
  `14 passed; 0 failed` for doc-tests, exit 0 — so the new `test` job's gate expression
  matches this output.

This is only one cell, so it doesn't stand in for the linux/macos/arm and
stable/nightly/1.80.0 legs; the real CI run is what we actually need. Would you mind
approving the queued runs when convenient?
