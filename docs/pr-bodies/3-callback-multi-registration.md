## Problem

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
