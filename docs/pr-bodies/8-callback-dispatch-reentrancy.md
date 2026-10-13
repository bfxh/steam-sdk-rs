## Problem

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

Proposed fix for the remaining B/C/D paths of the deadlock matrix posted there. Builds on the `(seq, Box)` table from #352 - this branch is based on that commit, so the diff below currently includes #352's commits. Merge #352 first; after it lands this PR will show only the reentrancy changes (if it was squash-merged, I will rebase this branch and force-push).
