# Local CI-equivalent evidence for steamworks-rs PRs #350-#358

Captured 2026-10-13. Purpose: the 9 fork PRs are all stuck at `action_required`
(CI never executes until a maintainer approves), so these are local measurements
of the same commands the upstream `CI` workflow runs.

## Environment (measured, not assumed)

- `rustc 1.99.0 (b940084d7 2026-09-28)`, `cargo 1.99.0 (5f94df478 2026-08-27)`
- Host triple: windows-gnu (see repo knowledge: this machine builds via gnu toolchain)
- Each build: `-j 1`, `CARGO_INCREMENTAL=0`, `RUSTFLAGS="-C debuginfo=0"`,
  shared `CARGO_TARGET_DIR=E:/cargo-target/steamworks`
- Worktrees verified clean (`git status --porcelain` => 0 files) and each local branch
  head == `fork/<branch>` head, so commits measured == commits pushed.

## Results

| PR | branch @ sha | `cargo build --lib` | other |
|----|--------------|---------------------|-------|
| #350 | fix/call-result-handler-self-deadlock @ 20b800a | exit 0 | |
| #351 | fix/call-result-size-cap @ ca23aab | exit 0 | |
| #352 | fix/callback-multi-registration @ 5d2bcc9 | exit 0 | |
| #353 | fix/null-guard-unavailable @ 6abc953 | exit 0 | |
| #354 | fix/cstring-nul-panic @ 5bf2336 | exit 0 | |
| #355 | fix/test-tiering @ 7cd6452 | exit 0 | `cargo fmt --all -- --check` exit 0; `cargo test` exit 0 |
| #356 | fix/ugc-builder-result @ f54fb6c | exit 0 | |
| #357 | fix/callback-dispatch-reentrancy @ ebd1183 | exit 0 | |
| #358 | test/offline-accessor-coverage @ 060ad3f | exit 0 | |

## #355 test-tiering: gate fingerprint reproduced

PR body claims "lib 7 passed / 0 failed / 13 ignored + Doc-tests 14 passed / 0 failed, exit 0".
Measured on this machine (raw: `E:/tmp/sw-ci/pr355-test.log`):

```
test result: ok. 7 passed; 0 failed; 13 ignored; 0 measured; 0 filtered out; finished in 2.47s
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
test exit=0
```

Exact match, including the ignored count. The new `test` job's gate expression
(`grep -E "^test result: ok\. [1-9][0-9]* passed; 0 failed"`) matches this output.

## Limits of this evidence (disclosed)

- Upstream CI builds a 4-OS x 3-toolchain matrix (windows/ubuntu/macos/ubuntu-arm x
  stable/nightly/1.80.0). What is measured here is **one cell**: windows-gnu, rustc 1.99.
  Green here does not predict green on linux/macos or on 1.80.0.
- **The 1.80.0 cell cannot be measured on this machine at all**: `rustup toolchain list`
  offers only `stable`, `nightly`, `1.96.0`, `1.99.0` (windows-gnu/msvc). The crate
  declares `rust-version = "1.80.0"`, so the declared MSRV is verified nowhere locally
  and only upstream CI can cover it.
- `cargo fmt --check` emits a pre-existing nightly warning ("can't set `ignore` ...
  unstable features are only available in nightly") from the repo's `rustfmt.toml`;
  it exits 0 regardless. Upstream only runs fmt under `if: runner.os == 'linux'`.
- `build --examples` (CI step for stable/nightly) was not run here.
- Logs: `E:/tmp/sw-ci/pr355.log`, `pr355-test.log`, `pr355-fmt.log`, `builds.log`,
  `stack8-build.log`, `stack8-test.log`.

## Published PR-body claims audit (2026-10-13, A2 ruler: re-derive every number)

Method: diffstats from `git diff --numstat base..tip` (no regex); structural counts from the diff's
own added lines, cross-checked against regex forms.

| Claim | Recomputed | Verdict |
|-------|-----------|---------|
| #350 `+10/-2` | 1 file, +10/-2 | reproduces |
| #352 6 files +277/-42 | 6 files, +277/-42 | reproduces |
| #353 20 files +1041/-0, 23 structs | 20 files +1041/-0; 23 `unavailable: bool` decls | reproduces |
| **#353 "all 258 guarded methods"** | **246** `if self.unavailable` lines added; only guard form present (8 more `self.unavailable` lines are non-guard uses) | **does not reproduce — corrected publicly** |
| #354 16 files +424/-70, "58 sites" | 16 files +424/-70; diff removes 58 `CString::new` lines (55 same-line `unwrap`, 2 same-line `expect`, 1 further cross-line site) | reproduces |
| #356 4 files +466/-213, "13 UpdateHandle + 9 QueryHandle" | 4 files +466/-213; 13 and 9 `-> Result<Self` per impl block | reproduces |
| #357 3 files +192/-7 | 3 files +192/-7 | reproduces |
| #359 (narrow) | 3 files **+92/-7** = #357 minus the 100 removed test lines | reproduces |

Notes on the measuring sticks themselves (two false alarms caused by my own narrow regexes, i.e.
the A2 failure mode recurring during this audit):
- A single-line regex counted 54 converted sites instead of 58 because rustfmt wrapped `.unwrap()`
  onto the following line at one site — the diff-line count is the ruler that holds.
- The same class of regex missed `pub fn tags<S: AsRef<str>>(` (multi-line generic signature) and
  briefly suggested that method did not exist; it is at `src/ugc.rs:1028` and returns
  `Result<Self, SteamError>`.

Public correction posted: #353 `issuecomment-6130140993` (verified via the comments API). The PR body
was deliberately left unedited so the wrong number stays visible next to its correction.

## Master-anchored claims audit (2026-10-13, second half)

These claims are about baseline `master` rather than a branch, so they can be re-derived without
checking out anything.

| Claim | Ruler used | Recomputed | Verdict |
|-------|-----------|-----------|---------|
| #355: "11 of the **20** unit tests (55%) fail without a Steam client" | `git grep -c "#\[test\]" 6e4a2bc -- src/` summed = **20**; `#[ignore]` on master = 2 (both in `user_stats.rs`, pre-existing); `7cd6452` adds `#[ignore = "requires a running Steam client"]` × **11** | 20 total = 7 runnable + 11 Steam-dependent + 2 pre-existing ignored | reproduces, and reconciles with the 7-11-2 fingerprint |
| #358: "**491** pub fns" | (a) `git grep -h "pub fn" 6e4a2bc -- src/ \| wc -l` = 491; (b) independent regex over `pub [unsafe] fn` on the same tree, excluding `pub(crate)` (17 sites) = 491 | 491 by two unrelated rulers | reproduces exactly |
| #358: "~**358** FFI-bound, ~**133** pure-logic" | brace-matched function bodies (not "slice to next pub fn", which swallows private helpers), classified by presence of `sys::`/`steamworks_sys::` | FFI **359**, no-direct-sys **132** | reproduces within ±1; the body is written with `~`, and the split is ruler-dependent ("no direct `sys::`" ≠ "pure", since some bodies delegate to helpers) |

Cross-check that also fell out of this: #359's claim "lib 24 = base 23 + 1" is consistent with the
master census — 20 unit tests + 3 added by #352 = 23 on `5d2bcc9`, +1 D-only test = 24.

## PR cross-reference & dependency audit (2026-10-13)

All ten PRs use `base=master`; dependencies exist only in prose, not in git structure. #352 must
land before #357 and #359 — both say so in their bodies, and #359 also references the option-4
discussion.

| # | cross-refs in body | dependency stated? | verdict |
|---|--------------------|--------------------|---------|
| #350 | #253, #259 | fixes the case left after #259 | ok |
| #351 | (none needed) | independent | ok |
| #352 | #253 | independent of the others | ok |
| #353 | (none needed) | independent | ok |
| #354 | "companion PR (ugc-builder-result)" | names companion by branch, not by number | ok, minor |
| #355 | (none needed) | independent | ok |
| #356 | describes the same CString surface #354 leaves | no explicit back-reference to #354 | minor gap, not a defect |
| #357 | #253, "builds on #352" | states the `(seq, Box)` dependency on #352 | ok |
| #358 | (none needed) | independent | ok |
| #359 | #253, option 4, "depends on #352", #357 | states #352 dependency and the #357 pairing | ok |

Reconcile check on the two numbers that could have collided: #354 says "the 19 panic points inside
the UGC builder methods are intentionally NOT converted here", and #356 lists its own breakdown as
"CString 11 sites + canonicalize 2 + assert 13 + tags expect 1". These are different scopes, and
both hold under the diff ruler:
- `git diff 5bf2336..f54fb6c -- src/ugc.rs | grep -c "^-.*CString::new"` = **19** ⇒ #354's 19.
- #356's 11 is the subset inside the 22 converted builder methods, stated separately from the
  other three panic classes (2 + 13 + 1 = 16, which together with the 11 CString = 27 converted
  panic points total in #356).

Method note: an initial scan counted 20 `CString::new` inside the 22 methods by slicing source
between method-name matches; that overcounts because the slices absorb private helpers sitting
between methods (the `language` entry appeared twice with different unwrap counts). The diff line
count is the ruler that holds; the scan was used only as a cross-check, per the counting discipline.

## Integration stack (all 9 fixes combined)

`integration/stack8` @ `c8b80c9` (local only, not pushed — it is the merge surface used to
check that the nine branches coexist). Verified: all nine fix commits (`20b800a`, `ca23aab`,
`5d2bcc9`, `6abc953`, `5bf2336`, `7cd6452`, `f54fb6c`, `ebd1183`, `060ad3f`) are ancestors of
its HEAD; worktree clean.

- `cargo build --lib` exit 0
- `cargo test` exit 0: lib `52 passed; 0 failed; 13 ignored` + doc `14 passed; 0 failed`
- Reconciles with the `e3730c5` fingerprint recorded in `SuperSteam-Design.md` (40 passed)
  plus #358's 12 added `#[test]` functions: 40 + 12 = 52.

## A5 upstream duplicate re-check (2026-10-13)

The design doc requires the upstream duplicate check to be re-run before submitting PRs; it
was blocked by network failures on 2026-10-11 and the PRs went out that night. Re-run now:

- `origin/master` is still `6e4a2bc` (0 commits since our base) => the stack verification is
  still on the current base, no replay needed.
- Duplicate reports for the nine defects: **0** (10 keyword searches over open PRs; the only
  foreign hits are #342 NetworkingIdentity, #347 glyph segfault in `input.rs`, #245 unsafe
  tightening).
- Forward-looking conflict risk, not duplication: **#245** (by `james7132`) rewrites 22 source
  files across the same modules the nine branches touch. If upstream merges it first, the nine
  branches need replay on the new base and the 52-0-13 fingerprint is void.

