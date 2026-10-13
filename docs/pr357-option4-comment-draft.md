Scope note I should have put in the description up front, since #253 already settled a direction.

Under **option 4** (deprecate `run_callbacks`, keep `process_callbacks` — the direction both
@james7132 and @Noxime endorsed in June 2025), two of the three cases this PR fixes disappear with
`run_callbacks` itself:

- **B** (`register_callback` from inside a handler) — the registration API goes away.
- **C** (nested `run_callbacks`) — the function goes away.
- **D** (`CallbackHandle::Drop` taking the `callbacks` lock — `src/callback.rs:165` on `master`
  `6e4a2bc`, line 169 on this branch's base) — **survives**
  option 4, because a handle is dropped by its owner wherever it was created, including from a
  handler handed to `process_callbacks`.

I have put the D-only slice up as **#359**, on the same base (`5d2bcc9`) with identical mechanism
code: the take-out/put-back is kept because a handle dropped mid-dispatch needs it to defer removal
(the tombstone), but B and C are neither claimed nor tested there. If you'd rather keep the
registered table permanently, this PR is the one to take; if option 4 is what lands, #359 is the
slice that stays relevant and this one can be closed in its favour.

To be explicit about evidence: the A/B/C/D/E matrix in the #253 comments was measured on current
`master`. The statement that B and C vanish under option 4 is a reading of that option's design —
I have not prototyped option 4 against this codebase.
