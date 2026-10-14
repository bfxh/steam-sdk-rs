# Changelog

Notable changes to this repository. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate version in
`Cargo.toml` tracks the Steamworks SDK release it bundles.

Entries below cite the commit that made the change, so each one is checkable against
`git log`. Everything under **Unreleased** is on `master` but not yet tagged; the crate
still declares version `0.13.1`.

## [Unreleased]

### Added — interface wrappers that upstream never bound

Each entry is the full interface, not a subset: every method the generated bindings expose,
plus the callbacks the wrapper can deliver.

- `ISteamInventory` — `client.inventory()` (`35a3a9e`). Item queries, consumption, exchange,
  trade, item drops, prices and purchases. `InventoryResult` is RAII (destroys the Steam handle
  on `Drop`), and its result is `is_ready`/`status`-checked rather than assumed.
- `ISteamHTTP` — `client.http()` (`e3ee5ac`). `HttpRequest` is a fallible builder chain (every
  step returns `Result<Self, SteamError>`); `CookieContainer` and `HttpResponse` are RAII, and
  the response status is copied out of the callback so it does not depend on a live handle.
  Two methods are deliberately **not** wrapped: `PrioritizeHTTPRequest` (this crate's binding
  takes no priority argument) and `GetHTTPStreamingResponseBodyData` (buffer size by value, no
  byte count reported back, so the final chunk length is unrecoverable).
- `ISteamParentalSettings` — `client.parental_settings()` (`64f7a01`), including the 15
  `ParentalFeature` mappings pinned by pairwise-distinctness tests.
- `Server::http()` and `Server::inventory()` (`a64a7c7`). These reuse the client wrappers:
  `SteamAPI_SteamGameServerHTTP_v003` returns `*mut ISteamHTTP` and
  `SteamAPI_SteamGameServerInventory_v003` returns `*mut ISteamInventory`, so no second
  implementation exists to keep in sync.
- `ISteamGameServerStats` — `server.stats()` (`7637d01`).
- `ISteamMusic` and `ISteamVideo` (`0de6c9d`).
- `ISteamParties` — `client.parties()` (`e0a7776`), with beacons, join tickets and six callbacks.
- `ISteamHTMLSurface` — `client.html_surface()` (`372f4e1`). All 37 methods plus 12 callbacks;
  `Browser` releases itself on drop. Two limits are documented in the source instead of being
  papered over: `HtmlKeyModifier` is a single choice because the generated
  `EHTMLKeyModifiers` is a `#[repr(i32)]` enum whose combinations have no valid discriminant
  (constructing one would be undefined behaviour, not merely a wrong number), and this SDK's
  `isteamhtmlsurface.h` declares no `RunFrame`, so the wrapper invents none.
- `RemoteStorage` one-shot cloud file I/O — the interface was bound, but only through the
  streaming/async path, so every read or write needed a `SteamFileWriter`/`SteamFileReader`.
  Added: `write_file` and `read_file` (whole file in one call), `quota`, `local_file_changes`
  (what changed locally since last time, for conflict handling), and `begin_write_batch`, whose
  guard ends the Steam batch on drop. Argument checks fail closed: an interior nul in a file name
  and a length Steam's int32 cannot express both return `InvalidParameter` rather than panicking
  or clipping. Steam's negative read count maps to `FileNotFound`, since it is the same answer for
  a missing file, an undersized buffer and a failed read. `ISteamRemoteStorage` coverage: 19 of 59
  binding functions called from `src/`, now 26; the remaining ones are mostly the retired legacy
  Workshop surface that `ISteamUGC` supersedes.
- `CallbackStream` behind the `futures` feature (`9575df6` history, `examples/stream_events.rs`),
  plus `SafeUgcUpdate`, a panic-free UGC update builder.
- `ISteamTimeline` completed -- 4 of its 18 functions were reachable; all 18 are now. Added:
  `add_instantaneous_event`, `start_range_event`, `update_range_event`, `end_range_event`,
  `remove_event`, `open_overlay_to_event`, `start_game_phase`, `end_game_phase`,
  `set_game_phase_id`, `add_game_phase_tag`, `set_game_phase_attribute`,
  `open_overlay_to_game_phase`, and the two async queries
  `does_event_recording_exist` / `does_game_phase_recording_exist` with their callbacks
  (`TimelineEventRecordingExists`, `TimelineGamePhaseRecordingExists` -- durations are `Duration`,
  and the fixed 64-byte phase id is cut at the first nul rather than read past it). The functions
  that must produce a value report `SteamError::Disabled` when the client is too old, instead of
  returning a handle from an API call that never happened; strings are checked for interior nuls
  before reaching Steam.
- `ISteamScreenshots` completed -- 4 of its 9 functions were reachable; all 9 are now. Added:
  `write_screenshot` (raw RGB buffer, with the byte count checked against the `u32` Steam takes, so
  an oversized buffer is rejected instead of clipped, and an empty buffer passes an aligned
  never-read address rather than null), `add_vr_screenshot_to_library` with the six
  `VRScreenshotKind` variants, and the three tag calls `set_location`, `tag_user`,
  `tag_published_file`.
- `ISteamUtils` from 16 to 32 of its 37 functions: `seconds_since_app_active`,
  `seconds_since_computer_active`, `ipc_call_count`, `battery_power` (Steam's 255 sentinel becomes
  `BatteryPower::OnAcPower` rather than a 255% reading), `overlay_needs_present`,
  `is_steam_running_in_vr`, `is_steam_china_launcher`, `entered_gamepad_text_length`,
  `dismiss_gamepad_text_input`, `dismiss_floating_gamepad_text_input`,
  `set_overlay_notification_inset`, and the VR headset streaming pair. The remaining five are
  registered one by one with the header text behind each judgement in
  `docs/SuperSteam-Design.md` 6.1d.
- Text filtering on `ISteamUtils`: `init_text_filter` (Steam's answer for "unavailable in this
  language" is a passthrough, not an error, so the doc says so rather than letting a caller read
  it as clean text) and `filter_text`, which allocates the `strlen(input)+1` buffer Steam asks
  for, cuts the result at the first nul instead of trusting a terminator, and treats a negative
  return as failure rather than zero changes. These stay in the list rather than in the skipped
  register because the newer interface I first assumed replaced them is not in these bindings at
  all (`grep -c SteamAPI_ISteamTextFiltering` = 0).
- `examples/inventory.rs` and `examples/http.rs` (`71ce0e7`) — compile-checked by CI; neither has
  been run, since both need a live Steam client.

### Added — CI and release automation

- `Cargo.lock` is committed and every build step runs `--locked`, so dependency drift fails the
  gate instead of silently changing the graph (`477870d`).
- `examples` job compiles the example crates on all three OSes (`845b430`). They are separate
  workspace members, so before this job nothing built them. It enables `steamworks/futures`
  explicitly: `stream_events.rs` declares `required-features = ["futures"]`, and cargo skips a
  target whose required features are absent without failing — measured with
  `cargo check --all-targets --message-format json`, which produces no `stream_events` artifact
  until the feature is named.
- `release.yml` builds both crates and the rustdoc site from the locked graph and attaches them
  to a GitHub Release on a `v*` tag (`b7fc121`). It does **not** publish to crates.io: the
  `steamworks` name belongs to the original author, so that remains a maintainer action.
- Advisory tools (udeps / geiger / audit) are installed as their own step, so a missing
  subcommand can no longer be swallowed (`fdb6a12`), and each one must now *answer*: udeps has to
  show it read the dependency graph and reach a conclusion line, geiger and audit have to produce a
  parseable non-empty report, and audit's advisory database must be populated and under 30 days old.
  A vacuous "green" is a failure, not a pass.
- `cargo audit` is now **blocking** (`--deny warnings`) with exactly six registered exceptions:
  RUSTSEC-2021-0019, -2020-0097, -2025-0051 (xcb), -2025-0035 (macroquad), -2022-0056 (clipboard),
  -2026-0192 (ttf-parser). All six sit on the `examples/*` dependency chain -- verified with
  `cargo tree -p steamworks -e normal,build -i <crate>`, which only reaches the example members --
  so none of them is in a published `.crate`. Any new advisory turns CI red instead of printing a
  hint nobody is required to read.

### Fixed

- **Registering a callback from inside a callback, dropping a `CallbackHandle` from inside one,
  and calling `run_callbacks()` re-entrantly all deadlocked** (`8bb88f2`). The dispatch loop runs
  user code while holding the callbacks lock and `std::sync::Mutex` is not reentrant. Such
  mutations are now queued and applied by the dispatch loop after each callback; a nested dispatch
  of the same context returns as a no-op. Three tests pin the mechanism, each mutation-checked.
  The dispatch loop itself needs a real Steam pipe, so no end-to-end run was performed — see the
  defect #2 status note in `docs/SuperSteam-Design.md`.
- `has_flag` compared `u16` against an `i32` on Windows but `u32` on Linux/macOS, so
  `ISteamInventory` did not compile off Windows (`1f91e00`); found only by CI, because the
  dev box had Steam installed.
- The license files were missing from the tree while `README.md` linked them and both manifests
  declared an invalid SPDX expression (`1f0e577`). `LICENSE-MIT` / `LICENSE-APACHE` were
  recovered from the commit that added them (`b23da13`) rather than fetched, and the expression
  is now `MIT OR Apache-2.0`.
- `steamworks-sys/src/windows_bindings.rs` was stored with mixed line endings; with our own
  sources pinned to `eol=lf` that made the release job's `cargo package` judge the tree dirty
  (`b83dc78`, verified line-ending-only with `git diff --cached --ignore-cr-at-eol`).
- `add_timeline_event` passed its arguments to `AddRangeTimelineEvent` in the wrong order: the
  call site gave `(icon, title, description)` while the binding declares
  `(pchTitle, pchDescription, pchIcon)`, so the icon name showed up as the event title and the
  title as the description. Fixed after checking both `isteamtimeline.h` and the generated
  signature; the wrapper's own parameter list is unchanged, so existing callers keep compiling and
  now land in the right UI fields.
- Doc comments: `src/networking.rs` had a malformed intra-doc link (`[../networking_sockets]`)
  and still claimed the `ISteamNetworkingSockets` wrapper was "unfinished" — it is not. Six other
  doc comments carried redundant explicit link targets. Rustdoc now runs with `-D warnings` in
  CI, measured clean on both the default and `--all-features` sets before tightening.

### Changed

- The Steam-dependent tests are `#[ignore]`d (14 of them today), so CI is green without a client
  (`26974e7` marked the first 11). They still run on demand via `cargo test -- --ignored`.
- The MSRV (1.80.0) jobs build the default feature set only; `--all-features` is covered by the
  stable jobs. Measured reason: the optional `image` feature's transitive dependencies cannot be
  parsed by cargo 1.80 at all.
- `cargo fmt --check` now runs on Windows too (`b4bbb6d`), after line endings were pinned for
  sources this repo writes. Vendored SDK headers are marked `-text` and stay byte-for-byte as
  shipped; they are stored with CRLF in the index (46 files) and would otherwise renormalize into
  a meaningless diff.
- The packaged `.crate` no longer contains analysis drafts, logs or CI configuration
  (`b7fc121`): `cargo package` ships untracked-but-not-ignored files, which is how session
  scratch got into a release artifact.
