## Problem

When the Steam API does not expose an interface accessor, the getter returns a null pointer and the following `assert!(!ptr.is_null())` compiles out in release builds - the null pointer is then dereferenced and the process dies with SIGILL. Reproduced in a release build by touching an interface the local client does not provide.

## Fix

The 23 interface structs carry an `unavailable` flag (same pattern as the existing `disabled` timeline accessor); all 258 guarded methods return their degradation value instead of touching the null pointer (bool -> false, Option -> None, handles -> 0, ...).

Zero panics, zero signature changes. +1041/-0 across 20 files.

## Testing

- Three in-tree offline tests drive guarded methods over null interface pointers (no Steam client needed).
- The tests deliberately leak one `Inner` reference: dropping `Inner` runs `SteamAPI_Shutdown` on an uninitialized context (mechanism detailed in the commit).
- Gates: build clean, serial 10-11-2 (= 7-11-2 + 3 new), clippy per-line identical, fmt clean.
