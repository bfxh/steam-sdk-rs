## Problem

CI runs fmt and build only - `cargo test` never runs because 11 of the 20 unit tests (55%) fail without a running Steam client (`NoSteamClient`), so the suite cannot gate anything.

## Fix

- The 11 Steam-client-dependent tests are marked `#[ignore = "requires a running Steam client"]` (still runnable via `cargo test --ignored` on a machine with Steam).
- Three doc examples that execute `Client::init()` become `no_run` / `rust,no_run` (still compile-checked). Accounting note: `cargo test` without `--no-fail-fast` stops after the first failing target, so these doc-test failures only surfaced once the lib tier went green.
- `ci.yml` gains a `test` job (ubuntu, stable): run `cargo test` with pipefail+tee, then gate on the summary - every result line must be ok with >0 passed and 0 failed, and the ignored count is echoed to the job log (a suite that only skips must not look green).

## Testing

- Offline fingerprint on this branch: lib 7 passed / 0 failed / 13 ignored + Doc-tests 14 passed / 0 failed, exit 0; parallel 10x clean; clippy lint-kind multiset identical; fmt clean.
