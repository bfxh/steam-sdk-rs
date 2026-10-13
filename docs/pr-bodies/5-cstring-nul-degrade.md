## Problem

58 call sites do `CString::new(...).unwrap()` on caller-provided strings: any interior NUL byte panics the process. The panic surface is invisible at compile time (`cargo build` is clean) and most sites are reachable from ordinary API use (titles, names, filter values, lobby data keys, ...).

## Fix

A `pub(crate) cstring() -> Option<CString>` helper plus per-return-channel degradation: bool-returning calls -> `false`, Option-returning -> `None`, Result-returning -> the existing error, handle-returning -> 0, name fields -> empty string, callback sites -> `cb(Err(SteamError::IOFailure))`.

Matches how the Steam SDK itself treats invalid input (ignore/skip). Every signature unchanged. 16 files, +424/-70.

## Testing

- 12 in-tree offline tests over the converted paths (null pointers as stand-ins, no Steam client needed).
- Gates: fmt clean / build clean / clippy lint-kind multiset unchanged / serial 19-11-2 (= 7-11-2 + 12 new) / parallel 10x clean.

## Scope note

The 19 panic points inside the UGC builder methods are intentionally NOT converted here - that surface needs the breaking `Result` conversion proposed in the companion PR (ugc-builder-result), and mixing the two would blur the review.
