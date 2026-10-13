## Correction to the description

The line "all **258** guarded methods" in the description is not reproducible. Re-counting against
this branch's own diff (`6e4a2bc..6abc953`, counting added guard statements):

- `if self.unavailable` guards added: **246**
- additional lines referencing `self.unavailable` (non-guard uses): 8
- `unavailable: bool` field declarations: **23** (this part is correct)

So the guarded-method count is **246**, not 258 — the number was carried over from an earlier working
count and never re-derived with a reproducible ruler. Everything else in the description is unchanged:
23 structs, `+1041/-0` across 20 files, no signature changes.

The 246 figure is the number of guard statements, which is 1:1 with guarded methods here since each
guarded method got exactly one such statement; if you count per call path rather than per statement
the number would differ, so flagging which ruler was used on purpose.
