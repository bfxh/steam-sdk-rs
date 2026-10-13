## Problem

`m_cubParam` is a raw `uint32` supplied by Steam. A corrupt value (observed at `usize::MAX` scale) drives `apicall_result.resize(...)` into a huge allocation; allocation failure aborts the process (uncatchable), so a single corrupt callback kills the app.

## Fix

- Cap the accepted size at `MAX_CALL_RESULT_SIZE = 64 MiB` via `call_result_size_ok`, which must also fit `i32`: the size goes back to the C API as a signed `cubCallback`, and `u32 -> i32` silently negates anything above `i32::MAX`.
- Reuse one buffer across the dispatch loop instead of reallocating per call.

## Testing

- In-tree offline unit test `call_result_size_bound` pins the bound and the `i32` path.
- Gates: fmt clean, clippy lint set unchanged, serial test fingerprint unchanged (7-11-2 on a Steamless machine).
