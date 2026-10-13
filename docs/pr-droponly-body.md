## Problem

`Callbacks::dispatch()` holds the callback table lock while invoking user callbacks. Dropping a
`CallbackHandle` from inside a callback therefore re-locks a `std::sync::Mutex` on the same thread
and deadlocks. This is case **D** of the matrix in #253 (`src/callback.rs:165`, measured on current
`master`).

D is singled out here because it is the one case that does **not** depend on the registered-handler
API surviving. A handle is dropped by whoever owns it, including from a callback handed to
`process_callbacks`, so it stays reachable under option 4 (deprecating `run_callbacks`) — the
direction #253 settled on in June 2025 — while cases B (registering from inside a callback) and C
(nested `run_callbacks`) disappear with `run_callbacks` itself.

## Fix

`dispatch()` takes the id's entries out of the table, releases the lock, invokes them, and puts them
back. A handle whose entry is out of the table at drop time records a tombstone (`pending_removal`)
that the put-back consults, so the documented "dropping the handle unregisters" semantics hold
across re-entrancy. The take-out is required for D specifically: the drop happens during dispatch,
so removal has to be deferred rather than attempted under the lock.

Scope, deliberately narrow: dropping the lock around the invocation also happens to unblock B and C,
but neither is claimed or tested in this PR. If the registered table is meant to stay permanent,
#357 is the wider variant covering B/C/D; if option 4 is what lands, this is the slice that remains
relevant.

If a callback panics, its entries are dropped with the in-flight Vec instead of poisoning the table
lock; other callback types keep working (behavior change vs. the poisoned-mutex status quo).

No public API signature changes. Depends on #352, since the tombstone keys on the per-registration
sequence numbers that branch introduces — the branch itself is rooted at #352's tip (`5d2bcc9`), so
the incremental change is best viewed against that rather than `master` (same sequencing situation as
#357, whose base is also `master`).

## Testing

Measured locally, rustc 1.99.0 / windows-gnu, `-j 1`, `CARGO_INCREMENTAL=0`,
`RUSTFLAGS="-C debuginfo=0"`:

- `cargo fmt --all -- --check` 0, `cargo build --lib` 0
- `cargo clippy --lib` 0, 121 warnings — same per-lint multiset as the base
- `cargo test` 0: lib **24 passed / 0 failed / 2 ignored**, doc 14 passed / 0 failed. Same-ruler
  control on the base `5d2bcc9` gives 23 / 0 / 2, so the delta is exactly the one new test.
- One in-tree test (`dropping_own_handle_inside_callback_unregisters_it`) drives the real dispatch
  under a 5s deadlock guard, so a regression fails on timeout instead of hanging the suite.

Caveat on the numbers: those counts were taken with a Steam client running on the test machine, so
the Steam-dependent tests execute rather than failing; they are not comparable to a Steamless CI run.
This is one matrix cell (windows-gnu, stable) and does not speak to linux/macos/arm or the 1.80.0 leg.
