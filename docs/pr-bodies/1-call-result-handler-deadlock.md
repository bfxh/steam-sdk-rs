## Problem

`SteamAPICallCompleted_t` handlers are invoked while the `call_results` mutex is held. Any handler that makes another Steam API call registering a call result (e.g. sending a follow-up request from inside a completion callback) re-locks the same mutex and deadlocks the thread.

This is case E of the deadlock matrix documented in #253: the Mutex split in #259 separated `callbacks` from `call_results`, but the `call_results` lock itself is still held across user code.

## Fix

Shrink the guard scope in `run_callbacks_raw`: take the handler out of the map, release the lock, then invoke it. +10/-2, no public API change.

## Testing

- A/B repro in `repro/` (deadlock before, completes after) and in the #253 thread.
- `cargo fmt --check` clean; `cargo clippy` lint set unchanged; test fingerprint unchanged (7 passed / 11 NoSteamClient / 2 ignored on a Steamless machine).
