## Summary

A coverage audit (491 pub fns: ~358 FFI-bound, ~133 pure-logic) showed the remaining untested pure-logic surface is small accessors. This locks the ones that carry real contracts. No production code changes.

- `AppNetConnectionEnd`: documented range contracts - `normal()` accepts only [1000, 1999] and `exception()` only [2000, 2999] (four `should_panic` boundary tests); the ranges are disjoint for `is_normal`/`is_exception`; generic codes land in each range.
- `LobbyKey`: `try_new` accepts exactly `k_nMaxLobbyKeyLength` bytes and rejects one more; `new()` panics over the limit (documented).
- `StringFilterKind -> ELobbyComparison`: all six arms pinned.
- `NetworkingConfigValue::data_type`: spot checks across the Float / Int32 / Callback groups.
- `AppIDs`: creator/consumer accessors across all three variants.
- `LobbyListFilter`: builder chain round-trips all filter slots.

## Testing

- Gates vs base: fmt clean, build clean, clippy lint-kind multiset identical, serial 19-11-2 (= 7-11-2 + 12 new), parallel 10x clean.
