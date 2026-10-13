"""Write the 9 PR body files (one markdown per fix branch)."""
from pathlib import Path

ROOT = Path(__file__).resolve().parent
OUT = ROOT / "pr-bodies"
OUT.mkdir(exist_ok=True)

BODIES = {
"1-call-result-handler-deadlock.md": """## Problem

`SteamAPICallCompleted_t` handlers are invoked while the `call_results` mutex is held. Any handler that makes another Steam API call registering a call result (e.g. sending a follow-up request from inside a completion callback) re-locks the same mutex and deadlocks the thread.

This is case E of the deadlock matrix documented in #253: the Mutex split in #259 separated `callbacks` from `call_results`, but the `call_results` lock itself is still held across user code.

## Fix

Shrink the guard scope in `run_callbacks_raw`: take the handler out of the map, release the lock, then invoke it. +10/-2, no public API change.

## Testing

- A/B repro in `repro/` (deadlock before, completes after) and in the #253 thread.
- `cargo fmt --check` clean; `cargo clippy` lint set unchanged; test fingerprint unchanged (7 passed / 11 NoSteamClient / 2 ignored on a Steamless machine).
""",

"2-call-result-size-cap.md": """## Problem

`m_cubParam` is a raw `uint32` supplied by Steam. A corrupt value (observed at `usize::MAX` scale) drives `apicall_result.resize(...)` into a huge allocation; allocation failure aborts the process (uncatchable), so a single corrupt callback kills the app.

## Fix

- Cap the accepted size at `MAX_CALL_RESULT_SIZE = 64 MiB` via `call_result_size_ok`, which must also fit `i32`: the size goes back to the C API as a signed `cubCallback`, and `u32 -> i32` silently negates anything above `i32::MAX`.
- Reuse one buffer across the dispatch loop instead of reallocating per call.

## Testing

- In-tree offline unit test `call_result_size_bound` pins the bound and the `i32` path.
- Gates: fmt clean, clippy lint set unchanged, serial test fingerprint unchanged (7-11-2 on a Steamless machine).
""",

"3-callback-multi-registration.md": """## Problem

Registering a second callback of the same type silently overwrote the first: the table stored `HashMap<i32, Box<dyn FnMut>>` keyed by callback ID. The documented "calling more than once will replace the previous callback" semantics apply to only three wrapper APIs; every other caller loses its callback.

Additionally, dropping a `CallbackHandle` removed *by ID*, killing every other registration of that type (stale-handle misfire).

## Fix

- Table becomes `HashMap<i32, Vec<(u64, Box<dyn FnMut>)>>`; each registration gets a unique sequence number; callbacks of the same type coexist and dispatch in registration order.
- `CallbackHandle` removes exactly its own `(id, seq)` entry - stale handles no longer kill others.
- New `register_replacing_callback` helper implements the documented replace semantics for the wrapper APIs that promise it (drops the replaced handle only after the registry lock is released), which also removes five `mem::forget` handle leaks at those sites.

Public API additions only (`register_replacing_callback`); existing signatures unchanged.

## Testing

- Five in-tree offline tests drive the real table: two-callbacks-both-fire-in-order, first-handle-drop-keeps-second, last-drop-removes-entry, replacing-replaces-only-its-own, internal+user coexist.
- Gates: fmt clean / build clean / clippy lint-kind multiset unchanged / serial 12-11-2 (7-11-2 + 5 new) / parallel 10x clean.

## Relation to #253

This restructure is also the base for a follow-up fix of the remaining callback-table deadlocks (cases B/C/D of the matrix posted there).
""",

"4-null-guard-unavailable.md": """## Problem

When the Steam API does not expose an interface accessor, the getter returns a null pointer and the following `assert!(!ptr.is_null())` compiles out in release builds - the null pointer is then dereferenced and the process dies with SIGILL. Reproduced in a release build by touching an interface the local client does not provide.

## Fix

The 23 interface structs carry an `unavailable` flag (same pattern as the existing `disabled` timeline accessor); all 258 guarded methods return their degradation value instead of touching the null pointer (bool -> false, Option -> None, handles -> 0, ...).

Zero panics, zero signature changes. +1041/-0 across 20 files.

## Testing

- Three in-tree offline tests drive guarded methods over null interface pointers (no Steam client needed).
- The tests deliberately leak one `Inner` reference: dropping `Inner` runs `SteamAPI_Shutdown` on an uninitialized context (mechanism detailed in the commit).
- Gates: build clean, serial 10-11-2 (= 7-11-2 + 3 new), clippy per-line identical, fmt clean.
""",

"5-cstring-nul-degrade.md": """## Problem

58 call sites do `CString::new(...).unwrap()` on caller-provided strings: any interior NUL byte panics the process. The panic surface is invisible at compile time (`cargo build` is clean) and most sites are reachable from ordinary API use (titles, names, filter values, lobby data keys, ...).

## Fix

A `pub(crate) cstring() -> Option<CString>` helper plus per-return-channel degradation: bool-returning calls -> `false`, Option-returning -> `None`, Result-returning -> the existing error, handle-returning -> 0, name fields -> empty string, callback sites -> `cb(Err(SteamError::IOFailure))`.

Matches how the Steam SDK itself treats invalid input (ignore/skip). Every signature unchanged. 16 files, +424/-70.

## Testing

- 12 in-tree offline tests over the converted paths (null pointers as stand-ins, no Steam client needed).
- Gates: fmt clean / build clean / clippy lint-kind multiset unchanged / serial 19-11-2 (= 7-11-2 + 12 new) / parallel 10x clean.

## Scope note

The 19 panic points inside the UGC builder methods are intentionally NOT converted here - that surface needs the breaking `Result` conversion proposed in the companion PR (ugc-builder-result), and mixing the two would blur the review.
""",

"6-ugc-builder-result.md": """## Problem

The UGC `UpdateHandle`/`QueryHandle` builder methods panic on bad input: `CString::new(...).unwrap()` on NUL-containing strings (11 sites), `path.canonicalize().unwrap()` for missing content/preview paths (2), `assert!(sys::Set*(...))` when Steam rejects a property (13), and `SteamParamStringArray::new`'s `expect` reached through `tags` (1). A NUL byte in a workshop item title takes down the process.

## Fix (breaking)

22 builder methods return `Result<Self, SteamError>`:

- 13 UpdateHandle: title, description, language, preview_path, content_path, metadata, visibility, tags, add_key_value_tag, remove_key_value_tag, add_content_descriptor, remove_content_descriptor, remove_all_key_value_tags
- 9 QueryHandle: exclude_tag, require_tag, language, add_required_tag, add_excluded_tag, set_language, set_cloud_file_name_filter, set_search_text, add_required_key_value_tag

Error mapping: NUL byte -> `SteamError::InvalidParameter`; canonicalize NotFound -> `FileNotFound`, other io -> `IOFailure`; false from the sys `Set*`/`Add*` call -> `InvalidParameter`. The three "Panics if ..." doc sections become "# Errors" sections. `#[must_use]` is dropped from the converted methods (Result is implicitly `must_use`).

Behavior improvements beyond the signatures:

- QueryHandle string methods previously failed silently in release builds (`debug_assert!` compiles out); they now return the error.
- `SteamParamStringArray::new` returns `Option<...>` and frees already-converted strings on failure.
- `init_for_game_server` keeps its `-> bool` signature (NUL -> false) to avoid cascading into `Server::init_for_game_server`.

The workshop example chains with `?` and reports a preparation error.

Out of scope: `QueryHandle::fetch`'s taken-handle unwrap (fetch-consumed semantics, separate concern) and `submit`'s `change_note` `.ok()` silent drop (no panic; documented degradation).

## Migration

Callers chaining `.title(...).description(...)...` add `?` in a fallible context (or handle the error where the chain is built). Meant for the next major-version window (cf. the breaking-change plans discussed around #253).

## Testing

- 8 offline tests construct handles with a null `ISteamUGC` and assert the error mapping on every converted error path; they pass without any Steam client, and a regression that reached the sys call would fail on `handle.unwrap()` instead of crashing the process.
- Gates: fmt clean / build clean / clippy lint-kind multiset identical / serial 15-11-2 (= 7-11-2 + 8 new) / parallel 10x clean.
""",

"7-test-tiering.md": """## Problem

CI runs fmt and build only - `cargo test` never runs because 11 of the 20 unit tests (55%) fail without a running Steam client (`NoSteamClient`), so the suite cannot gate anything.

## Fix

- The 11 Steam-client-dependent tests are marked `#[ignore = "requires a running Steam client"]` (still runnable via `cargo test --ignored` on a machine with Steam).
- Three doc examples that execute `Client::init()` become `no_run` / `rust,no_run` (still compile-checked). Accounting note: `cargo test` without `--no-fail-fast` stops after the first failing target, so these doc-test failures only surfaced once the lib tier went green.
- `ci.yml` gains a `test` job (ubuntu, stable): run `cargo test` with pipefail+tee, then gate on the summary - every result line must be ok with >0 passed and 0 failed, and the ignored count is echoed to the job log (a suite that only skips must not look green).

## Testing

- Offline fingerprint on this branch: lib 7 passed / 0 failed / 13 ignored + Doc-tests 14 passed / 0 failed, exit 0; parallel 10x clean; clippy lint-kind multiset identical; fmt clean.
""",

"8-callback-dispatch-reentrancy.md": """## Problem

`dispatch()` holds the callback table lock while invoking user callbacks, so three operations deadlock when performed from inside a callback: registering a callback, nesting a `run_callbacks`/dispatch, and dropping a `CallbackHandle` (cases B/C/D of the #253 deadlock matrix; case E, the call-result path, is fixed separately).

## Fix

The dispatcher takes the id's entries out of the table, releases the lock, invokes the callbacks, and puts the entries back:

- A nested dispatch of the same type finds no entries and returns immediately; another type runs normally.
- Registrations made while the bucket is out join the table by sequence number and first fire on the next dispatch (they do not receive the in-flight event).
- A `CallbackHandle` dropped while its entry is in flight records a tombstone (`pending_removal`) that the put-back consults, so the documented "dropping the handle unregisters" semantics survive re-entrancy - including a re-entrant `register_replacing_callback` that re-creates the bucket before dropping the replaced handle (caught by an in-tree test: a bucket-existence check alone misses it).

Behavior change, documented: a panicking callback now drops its in-flight entries instead of poisoning the table lock (the poisoned lock previously disabled that Inner's callback system entirely).

No public API change.

## Testing

- Four in-tree tests drive the real dispatch with a 5s deadlock guard (a regression times out instead of hanging the suite): re-entrant registration, nested same/other-type dispatch, self-handle drop, re-entrant replacing registration.
- Gates: fmt clean / build clean / clippy lint-kind multiset identical / serial 16-11-2 (= base + 4 new) / parallel 10x clean.

## Relation to #253

Proposed fix for the remaining B/C/D paths of the deadlock matrix posted there. Builds on the `(seq, Box)` table from the multi-registration PR - this branch is based on it and should be merged after it (PR base branch is set accordingly).
""",

"9-offline-accessor-coverage.md": """## Summary

A coverage audit (491 pub fns: ~358 FFI-bound, ~133 pure-logic) showed the remaining untested pure-logic surface is small accessors. This locks the ones that carry real contracts. No production code changes.

- `AppNetConnectionEnd`: documented range contracts - `normal()` accepts only [1000, 1999] and `exception()` only [2000, 2999] (four `should_panic` boundary tests); the ranges are disjoint for `is_normal`/`is_exception`; generic codes land in each range.
- `LobbyKey`: `try_new` accepts exactly `k_nMaxLobbyKeyLength` bytes and rejects one more; `new()` panics over the limit (documented).
- `StringFilterKind -> ELobbyComparison`: all six arms pinned.
- `NetworkingConfigValue::data_type`: spot checks across the Float / Int32 / Callback groups.
- `AppIDs`: creator/consumer accessors across all three variants.
- `LobbyListFilter`: builder chain round-trips all filter slots.

## Testing

- Gates vs base: fmt clean, build clean, clippy lint-kind multiset identical, serial 19-11-2 (= 7-11-2 + 12 new), parallel 10x clean.
""",
}

for name, body in BODIES.items():
    (OUT / name).write_text(body, encoding="utf-8", newline="\n")
print(f"[ok] {len(BODIES)} PR body files written to {OUT}")
