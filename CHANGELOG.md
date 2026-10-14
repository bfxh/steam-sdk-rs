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
- `ISteamApps` from 16 to 29 of its 33 functions: `dlc_count`, `dlcs` (each entry carrying the
  ownership bit Steam reports), `install_dlc` / `uninstall_dlc`, `dlc_download_progress`,
  `is_subscribed_from_family_sharing`, `timed_trial` (Steam's bool -- not the second counts --
  decides whether there is a trial at all, so a trial with zero allowance stays a trial),
  `earliest_purchase_unix_time` (Steam's 0 for "no purchase record" maps to `None`),
  `mark_content_corrupt`, the beta branches (`beta_counts`, `betas`, `set_active_beta`), and
  `file_details`, which is asynchronous: the answer arrives as `Result<FileDetails, SteamError>`
  so a Steam-side failure cannot masquerade as a zero-byte file, and the 20-byte SHA-1 ships with
  `sha1_hex()` (40 lowercase hex characters). The remaining four are registered with reasons in
  `docs/SuperSteam-Design.md`: two legacy proof-of-purchase key calls, `SetDlcContext`, and the
  array-out-parameter `GetInstalledDepots`.
- `ISteamFriends` local-state, clan and asynchronous surface, from 27 to 53 of its 78 functions:
  `persona_state`, `friend_relationship`, `friend_steam_level`, `rich_presence_keys` (the key names
  behind `Friend::rich_presence`), `request_friend_rich_presence` with its
  `FriendRichPresenceUpdate` callback -- without that request the two readers usually see nothing --
  `set_in_game_voice_speaking`, `unread_priority_chat_count`,
  `set_listen_for_friends_messages`, `open_remote_play_together_invite_dialog`, plus clans:
  `clan_count`, `clans`, `clan_name` / `clan_tag` (a `None` while Steam has not cached the text,
  which is a different claim than "this clan has no name"), `clan_owner`, `clan_officer_count`,
  `clan_officers`, `is_clan_public`, `is_clan_official_game_group`, `clan_chat_member_count` and
  `is_clan_chat_admin`. The asynchronous pieces go through `register_call_result`:
  `request_clan_officer_list` (without it the officer reads have nothing to report),
  `download_clan_activity_counts` (Steam returns only a success flag, so the numbers are read back
  with `clan_activity`, whose `None` means "not cached" rather than "no activity"),
  `follower_count` and `is_following` -- both return `SteamAPICall_t`, not the bool/int the header
  names suggest. Still open here: friend groups, `EnumerateFollowingList`, profile items and
  persona-name history.
- `ISteamFriends` chat surface, from 53 to 63 of its 78 functions (`master`, this release):
  `friend_message` / `reply_to_friend_message`, `send_clan_chat_message` / `clan_chat_message`,
  `join_clan_chat_room` (async, answers as `ClanChatRoomJoined` carrying Steam's own
  `ChatRoomEnterResponse` -- a refused join arrives as `Ok` with the reason, because that is what
  Steam actually says), `leave_clan_chat_room`, `clan_chat_members`, and the overlay window trio
  `is_clan_chat_window_open_in_steam` / `open_clan_chat_window_in_steam` /
  `close_clan_chat_window_in_steam`, with the `FriendChatMessageReceived`,
  `ClanChatMessageReceived` and `UnreadChatMessagesChanged` callbacks.
  This closes a door this repo had left half-open: `set_listen_for_friends_messages` shipped in the
  previous batch with a doc saying Steam stops showing messages while it is on, and no way to receive
  or answer them. Three measurements behind those choices: the `EChatEntryType` out-parameter is read as a plain
  32-bit integer (the generated enum is `#[repr(i32)]` on Windows and `#[repr(u32)]` elsewhere, and
  both are `#[non_exhaustive]`, so a code this binding cannot name must not be materialised as one
  of its variants) and mapped without wrapping, so `257` is `Invalid` rather than `ChatMsg`;
  a receive buffer is clamped into `[1, MAX_CHAT_MESSAGE_BYTES]` so the `int` length argument can
  never be handed a wrapped-negative value, and a full buffer is reported as `possibly_truncated`
  instead of silently shortened; and an interior nul in outgoing text is
  `Err(SteamError::InvalidParameter)` -- the argument never reached Steam -- which is kept distinct
  from `Ok(false)`, Steam's own refusal.

- `ISteamUGC` query surface, from 60 to 73 of its 99 functions (`master`, this release):
  `QueryResults::tag` / `tag_display_name` / `tag_count`, `additional_preview` /
  `additional_preview_count` with a new `PreviewKind` (six named Steam types plus `Unknown`,
  because Steam's `ReservedMax` is a real answer and the generated enum is `#[repr(i32)]` on
  Windows and `#[repr(u32)]` elsewhere -- so the out-parameter is read as a plain 32-bit integer
  rather than materialised as a non-exhaustive enum), `first_key_value_tag` (Steam's own header
  answers "false if there was an error or no matching value was found", so `None` covers both and
  it is documented as such), `supported_game_version` / `supported_game_version_count`,
  `QueryHandle::add_required_tag_group` / `set_return_playtime_stats` /
  `set_time_created_date_range` / `set_time_updated_date_range`, and
  `UGC::query_all_with_cursor` for Steam's deep paging.
  Cursor paging now actually closes: `SteamUGCQueryCompleted_t.m_rgchNextCursor` -- present on all
  four platforms -- was dropped on the floor, so a cursor query could never be continued. It is
  read into `QueryResults::next_cursor`, with Steam's empty answer meaning "no next page".
  Buffer sizes come from Steam's own constants (`k_cchTagListMax`, `k_cchPublishedFileURLMax`,
  `k_cchFilenameMax`) rather than invented round numbers, and out-strings are cut at the first nul
  inside the buffer this side owns instead of trusting Steam to terminate, so an answer that fills
  the buffer cannot make the read run past it. One unusable tag fails a whole tag group: Steam
  must not be asked for a partial version of what the caller wrote.
  Measured with six mutation controls (terminator fallback, `Unknown` reported as `Image`, a
  dropped preview arm, an always-`Some` cursor, a silently-stripped nul byte, a shrunk buffer
  constant), each red, each restored by byte-compare. 26 of the 99 remain, classified against
  `isteamugc.h` in `docs/SuperSteam-Design.md` (update-handle previews, the local-cache array
  calls, the asynchronous dependency/favourite/vote/EULA answers, and the deprecated
  `RequestUGCDetails`).
- `ISteamUGC` update and local-cache surface, from 73 to 88 of its 99 functions:
  `UpdateHandle::add_preview_file` / `add_preview_video` / `update_preview_file` /
  `update_preview_video` / `remove_preview` / `set_required_game_versions` / `set_admin_query` /
  `set_allow_legacy_upload`, `UGC::show_workshop_eula`, and the local-cache set
  `downloaded_item_count` / `downloaded_items` / `mark_downloaded_item_as_unused` /
  `set_items_disabled_locally` / `set_subscriptions_load_order` /
  `user_content_descriptor_preferences`.
  These answer `Err` where the inherited setters panic: an unresolvable preview path is
  `FileNotFound` rather than `canonicalize().unwrap()`, an interior nul is `InvalidParameter`, and
  `PreviewKind::Unknown` is refused on the way *in*, since no integer means "a code this wrapper
  cannot name". Two of the 99 binding names were then left un-called, and both are still open:
  `RequestUGCDetails` (the header marks it "DEPRECATED - Use CreateQueryUGCDetailsRequest call
  above instead!") and `GetAppDependencies` (Steam answers it more than once -- see the next
  entry for why that one is not wrapped).

- `ISteamUGC` consumer answers, from 88 to 97 of its 99 functions (`src/ugc/answers.rs`):
  `add_dependency` / `remove_dependency`, `add_app_dependency` / `remove_app_dependency`,
  `add_item_to_favorites` / `remove_item_from_favorites`, `set_user_item_vote`,
  `get_user_item_vote`, `get_workshop_eula_status`. Each is one call with exactly one
  `STEAM_CALL_RESULT` callback, so `FnOnce(Result<P, SteamError>)` describes it honestly, and
  all eight answer types also became `CallbackResult` variants, so the callback stream carries
  them instead of only the registering closure seeing them.
  `GetAppDependencies` stays unwrapped on purpose: `isteamugc.h:380-381` says its callback "may
  be called multiple times until all app dependencies have been returned", and handing the first
  of those to a `FnOnce` would present a partial list as the whole answer.
  `UGC::get_workshop_eula_status` also replaces the "this crate does not wrap yet" note on
  `show_workshop_eula`, which is now the pointer it always wanted to be.
- `GetAppDependenciesResult` -- one page of an item's app dependencies, carried by the callback
  stream. `GetAppDependencies` itself stays unwrapped as a one-shot call (see above), but its
  answer was previously unreachable as a type: the struct is self-describing about pagination
  (`isteamugc.h:604-607`: a fixed `AppId_t m_rgAppIDs[32]`, `m_nNumAppDependencies` "number
  returned in this struct", `m_nTotalNumAppDependencies` "total found"), so the wrapper delivers
  exactly that -- a page whose `app_ids` length is clamped to what the array can hold, with the
  page count and the total kept apart. The clamp is not decoration: with the count trusted
  directly, a reported 33 indexes past a 32-entry array, and control P1 reproduced that as a
  panic at `src/ugc/answers.rs:265`; deleting the stream arm instead turned the same test red at
  the `panic!("GetAppDependenciesResult decoded as ...")` line (P2).
  The stream-coverage floors in `callback.rs` moved to their measured values (78 definitions,
  54 streamed, the difference being the 24-entry debt list) so a matcher that starts skipping
  files trips on the floor rather than lowering the debt quietly.

- `examples/inventory.rs` and `examples/http.rs` (`71ce0e7`) — compile-checked by CI; neither has
  been run, since both need a live Steam client.

- `ISteamNetworkingUtils` ping and POP diagnostics, from 3 called functions to 14 of its 41:
  `local_ping_location` (which returns the *age* of the data and `None` on Steam's -1 "no data
  is available yet"), `estimate_ping_time`, `estimate_ping_time_from_local_host`,
  `ping_location_text` / `parse_ping_location` (the only supported round-trip — Steam says of
  the text form "it is subject to change so please do not parse it yourself", and of the struct
  "Do not serialize it, send it over the wire, or persist it", which is why
  `SteamNetworkPingLocation` has no `Serialize` impl), `ping_data_up_to_date`,
  `ping_to_data_center`, `direct_ping_to_pop`, `pop_count`, `pop_list`, `local_timestamp`.
  Latency answers are `PingEstimate`, which names only the two sentinels Steam documents
  (`k_nSteamNetworkingPing_Failed` -1, `k_nSteamNetworkingPing_Unknown` -2) and leaves every
  other integer a number rather than guessing at a third meaning. `pop_list` sizes its buffer
  from `GetPOPCount` and clamps Steam's filled-count through `pops_to_take`, extracted so the
  two failure directions are testable: a count above capacity truncates to the buffer, a
  negative count takes nothing instead of wrapping into a huge `usize` (controls N1/N2/N3 each
  turned the matching test red, and N1 swaps the two sentinels while N3 drops exactly the
  `.max(0)`).
- `panic sites on unnamed enum values stay counted` (`scripts/ci/no_panic_on_unknown_enum.py`):
  every `unreachable!()` in a fallback arm of a `match` over a `#[non_exhaustive]` Steam enum is
  a panic inside the caller's process the first time Steam answers with a value the vendored
  headers do not name. The gate counts them per file, bidirectionally, and prints each
  `file:line` in the log; the first run measured 10. One is fixed here: `LobbyChatUpdate`'s
  member-state field now maps through `chat_member_state_change`, which returns
  `ChatMemberStateChange::Unknown` for an unnamed code instead of panicking, with the bitmask
  caveat documented (Steam's own field is flags, so a combination is possible, and reading it as
  flags rather than one state is the open change). Controls: swapping `Entered` for `Left` turned
  the named-flag test red (U1); adding a second `unreachable!()` turned the ledger red with
  `src/error.rs: 1 -> 2 (+1)` (U2). The other 9 stay registered as a ceiling that can only move
  toward zero -- `user.rs` (3), `ugc/types.rs` (2), `error.rs`, `remote_storage.rs`, `server.rs`
  and `ugc/update.rs` (1 each) -- because giving nine more public enums an `Unknown` variant is an
  API decision that deserves its own change, not the tail of this one.

### Added — CI and release automation

- Four new strictness gates, each one step with one verdict (`scripts/ci/`, so the
  failure names the gate instead of a mega-step that can half-run):
  - `module sizes stay under their registered ceilings` and
    `no type grows into a god object` measure every `.rs` file's line count and every
    type's inherent `pub fn` count, then compare against baselines recorded from the
    measured tree. The comparison is **strict in both directions**: one unit of growth
    is red, and so is a shrink -- a ceiling looser than reality would let the next
    batch grow back into the same shape, so the number has to be re-put on purpose.
    Controls: three added lines (`friends.rs 1501 -> 1504`) and one added method
    (`UGC 28 -> 29`) each turned the gate red, both restored by byte-compare.
  - `MSRV has a single source` refuses any drift between `Cargo.toml`,
    `steamworks-sys/Cargo.toml` and the version the matrix actually runs (control:
    asking it to check `1.99.0` exits 1).
  - `test floors (a run that tested nothing is not green)` reads the number of tests
    that *reached* the runner. `cargo test` with a filter that matches nothing still
    exits 0 and prints `test result: ok. 0 passed`, which is how a green job can prove
    nothing; the gate refuses a zero run and a run below the registered floor
    (lib 103, doc 16 -- measured, not guessed; lib moved from 102 when `game_played` gained
    its layout test). Control: raising the floor by one turned
    it red with the count printed.
  - `clippy correctness/suspicious (zero tolerance)` runs `cargo clippy --locked --all-targets
    --all-features -- -D clippy::correctness -D clippy::suspicious` and fails on any hit. These
    two categories are clippy's own "probably a bug" buckets, so there is no baseline to slide
    and no percentage to argue about. Four controls, each with the exit code read from the gate:
    re-inserting one `.to_owned()` printed `clippy::suspicious_to_owned: src/error.rs:550` and
    exited 1 (C1); adding a second `#[allow(clippy::…)]` exited 1 with
    "2 clippy suppressions in the sources, 1 registered" (C2) -- because a suppressed lint is
    otherwise this gate's blind spot, so every suppression in the hand-written sources is in a
    ledger that only a deliberate commit may change; a syntax error exited 2 with "failed for a
    reason outside the two denied categories" instead of claiming zero findings (C3); an unknown
    feature, where cargo never reaches the lint pass, exited 2 with "the gate has no verdict, so
    it refuses to report clean" (C4). A first version of the floor ordering reported C1 as
    "no compiled artifact for ['steamworks']" -- true but not the verdict, since a denied lint
    *should* stop that crate from producing an artifact; the findings are now read first.
  - `interface coverage stays at its registered ledger` (`scripts/ci/api_coverage.py`) measures
    every Steam interface as two registered numbers -- `declared` (what the bindings expose) and
    `called` (what `src/` calls) -- across all 33 of them, currently **576 / 913**. The design doc's
    coverage table is this gate's output now, because the hand-copied version had already started to
    drift, and losing one wrapped function turns CI red until somebody re-records the number.
    Four controls, exit codes read from the gate: renaming one call site
    (`AddDependency` -> `AddDependencyTypo`) made the ruler print `ISteamUGC.called 96` and the
    comparator answer "97 -> 96, tighten the baseline" (Ca); naming an uncalled function
    (`RequestUGCDetails`) in a doc comment left the number at 97 (Cb), because comments are stripped
    before matching -- without that strip, writing "we never call X" would *buy* coverage; putting
    the same call inside a `#[cfg(test)] mod` made the ruler refuse with
    "calls 1 interface function(s) only from test code" (Cc), since silently dropping it would be
    the same blind spot pointed the other way; and a missing bindings file exits 2 with "not a
    verdict" instead of printing zero coverage as a fact (Cd -- a first version let the
    `FileNotFoundError` traceback escape as exit 1).
    Building this ledger surfaced three prose defects in the doc it replaced: a ruler that matched
    *header* names against binding names had reported `CreateQueryAllUGCRequest` as never called,
    when the bindings reach it through `..._RequestPage` / `..._RequestCursor`; and four zero rows
    had been explained as "structural" without evidence. The note now separates what a header says
    (`isteamcontroller.h:2`: "The older ISteamController interface has been deprecated in favor of
    ISteamInput") from what is simply unwrapped (`ISteamMatchmaking{Ping,Players,Rules,
    ServerList}Response` and `ISteamNetworkingFakeUDPPort` -- zero name references under `src/`,
    measured), and records that the server-list vtable the crate does hand-write
    (`matchmaking_servers_callback!`, instantiated at lines 148/449/467/480) never calls those
    interfaces' binding entry points.
  - `feature matrix` builds each optional feature alone (`raw-bindings`, `serde`,
    `image`, `futures`) plus the feature-free build, because `--all-features` hides a
    feature whose own `cfg` block rotted -- and the first person to hit that is a user
    who only enabled one feature. All five pass; the job runs as its own gate.
- The first draft of `test_floor.sh` invoked `"${args[@]}"` without the `cargo` word,
  so bash ran the `test` builtin with cargo's arguments and the gate "passed" while
  running nothing. Caught by running the positive case locally rather than trusting the
  exit code; recorded here because it is the exact failure mode the gate exists to catch.
- `src/ugc.rs` (2714 lines) is split into `src/ugc/mod.rs` (the `UGC` accessor),
  `src/ugc/types.rs` (the value types and their Steam-side mappings) and
  `src/ugc/ugc_tests.rs` (the measurement guards). Public paths are unchanged --
  `crate::ugc::*` still re-exports everything, so no caller sees a difference. This is
  what makes the size ceiling a real number rather than a frozen complaint.
- The same rule then cut `src/ugc/mod.rs` further into `query.rs` (487), `results.rs` (473),
  `update.rs` (371) and `read.rs` (142), and split the chat cluster out of `src/friends.rs`
  into `src/friends/chat.rs` (216), leaving `friends/mod.rs` at 1299 (`0654e78`).
- `src/networking_types.rs` (2271) is split into `src/networking_types/mod.rs` (1733) and
  `src/networking_types/config.rs` (551) -- the config dictionary, its two `bitflags` sets and
  the `NetworkingConfigValue` -> Steam-enum mapping. The cut is along *definitions*, not callers:
  `mod.rs` still names those types 22 times through the re-export, which is fine -- the point was
  to move the 500-line enum and its mapping impl out of the file everybody has to scroll through,
  not to pretend the two halves are unrelated. Public paths are unchanged for the same reason as
  above.
- Each of those cuts is checked by two rulers with different jobs: `module_sizes.py` re-measures
  every file, so the baseline has to be re-recorded by hand for the shrink to be a deliberate
  act rather than a silent side effect, and `type_surface.py` must report the *same* registered
  totals (79 keys, 777 inherent methods) -- that one is the evidence that no method moved,
  changed owner, or vanished during the cut. The size gate going red first is the point: an
  unregistered new file (`config.rs=551`, `mod.rs=1733`) and a stale key
  (`src/networking_types.rs`) were both named before anything was recorded.
  At that moment three files were above 1300 lines -- `networking_types/mod.rs` (1733), `lib.rs`
  (1355) and `networking_sockets.rs` (1314) -- and the first two are since gone below it, in the
  two entries further down; `networking_sockets.rs` (1314) and `friends/mod.rs` (1321, grown by a
  test since) are what the same rule has left to do.
- `src/networking_sockets.rs` (1314) became a directory: `networking_sockets/connection.rs` (536)
  holds `NetConnection` -- the per-peer handle, its send/query/close methods and its RAII drop --
  and `networking_sockets/mod.rs` (789) keeps `NetworkingSockets`, `ListenSocket`, the inner
  socket guard and `NetPollGroup`. Public paths are unchanged again (`pub use connection::*`),
  with the same two witnesses as the other cuts: `type_surface` unmoved at 80 keys / 798 methods,
  and the lib suite still reaching 118 tests.
- `src/lib.rs` (1355) is split three ways: the identity types (`SteamId`, `Universe`,
  `AccountType`, `AccountId`, `GameId`) move to `src/identity.rs` (198) and are re-exported from
  the crate root, so every public path -- `steamworks::SteamId` included -- is unchanged; the
  root's own tests move to `src/lib_tests.rs` (354); and what is left in `lib.rs` (814) is the
  client, the interface getters and the callback dispatcher. Two independent checks say nothing
  was lost in the move: `type_surface` still reports the same 79 keys and 786 inherent methods, and the
  lib suite still reaches exactly 109 tests -- the same count the floor was tightened to before
  the split, which is the evidence that no test was orphaned by the cut.
- `src/networking_types/mod.rs` (1735) is cut twice more: the connection-end taxonomy --
  `AppNetConnectionEnd`, `NetConnectionEnd` and the three mapping impls between them and raw
  `i32` (384 lines) -- goes to `src/networking_types/end.rs`, and the read side of a live
  connection (`NetConnectionInfo`, `NetConnectionRealTimeInfo`, `NetConnectionRealTimeLaneStatus`
  with their `Debug`/`From` impls, 271 lines) goes to `src/networking_types/connection_info.rs`.
  `mod.rs` is now 1100 and every public path is still `steamworks::networking_types::*`, because
  both modules are re-exported with `pub use`. The same two checks carry the claim as before:
  `type_surface` unchanged at 79 keys / 786 methods, and the lib suite still reaching 109 tests.
- Building that split found a hole in the coverage ledger shipped in the same session: the
  `#[cfg(test)]` guard only recognised test modules written *inside* a file, so a test moved to
  `lib_tests.rs` would have had its Steam calls counted as wrapper coverage. Test-named files
  (`*_tests.rs`, `tests.rs`) are now skipped as a whole, and a test file that calls an interface
  function makes the ruler refuse instead of shrugging -- measured today at the same 576 / 913,
  so the rule changes no number that CI has already registered.

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
- **Workflow self-check** (`workflow_inputs.py`, plus a shrink-only count of shell escape
  hatches registered at the measured 10). Three invariants this repository has each violated
  once: a run step pointing at a file that is not tracked (the `*.txt` and clone-before-commit
  incidents -- green on the writing machine, red everywhere else), `permissions:` interpolating
  `${{ ... }}` (it zeroed every non-dispatch run of `rebuild-bindings.yml`), and a job without
  `timeout-minutes`. Building the gate exposed a hole in the gate: YAML parses
  `contents: ${{ github.ref_type == 'tag' && 'write' || 'read' }}` down to the string `write`,
  so the first version -- written against the parsed document -- passed a control that should
  have failed it. It now scans raw text and names file plus line; malformed YAML is reported as
  "does not parse" rather than as a traceback, because a crash is not a verdict. The gate caught
  its own new scripts while they were still untracked, before the commit landed.

- **Packaging gate** (`package_inventory.py` + `compare_paths.py`, registered list in
  `scripts/ci/baselines/package_inventory.txt`): what actually goes into each `.crate` is now
  compared against a reviewed file set, and a mismatch blocks both CI and a release. The
  release job used to `cargo package --list` and print it, which is how a real defect survived
  review: `steam_appid.txt` -- a local convenience file so the examples run against a Steam
  client -- was inside the published crate. It stays tracked in the repository, and is now in
  `[package] exclude`. Controls: dropping `steamworks/LICENSE-MIT` from the set reports
  "stopped shipping" (rc=1) -- a missing licence file is a hole no test can see; a fabricated
  extra file reports "ships now, did not before" (rc=1); a missing registered list names its
  two causes (rc=1); a five-entry list is refused as theatre (rc=2). Registered inventory:
  143 files (steamworks 69, steamworks-sys 74).

- Two further gates, both verifying something no test can see:
  - **Vendored SDK manifest** -- 45 headers plus `steam_api.json` under
    `steamworks-sys/lib/steam/public/steam/` are hashed and compared against
    `scripts/ci/baselines/sdk_vendored.txt`. Editing a Valve header reddens no test: the
    generated bindings are checked in, so the change only surfaces the next time somebody
    regenerates them, which silently changes what this wrapper is a wrapper *of*. Controls,
    each with the exit code read from the gate rather than from a pipeline's tail: one
    appended line reports `changed` (rc=1), a fabricated record reports `added` (rc=1), a
    missing manifest names its two possible causes (rc=1), a five-record manifest and a
    missing SDK directory are treated as tooling failures (rc=2), and renaming a header is
    caught by the producer's own floor (45 < 46, rc=2). Restores verified by byte-compare.
    Corrected a claim of mine in `.gitattributes` along the way: the vendored CRLF set is 46
    files = 45 headers + the API json, not "46 headers".
  - **Compile coverage** -- cargo skips a target whose `required-features` are not enabled and
    `--all-targets` does not override that, so a green job can have never looked at
    `examples/stream_events.rs`. The gate asserts both halves from cargo's own JSON: with
    `--features steamworks/futures` the target is built; without it the target is *not* built.
    The second half checks the premise the first half stands on, so if cargo ever stops
    skipping, the gate says so instead of quietly becoming meaningless. Measured:
    `['http', 'inventory', 'stream_events']` with the feature, `['http', 'inventory']` without.
- These two were then run from a clean `git clone` -- the step that was missing when the first
  batch of gates shipped with their baselines git-ignored.

- Two in-repo gates now watch the callback surface (`master`, this release).
  `every_callback_is_streamable_or_listed_as_debt` parses every `impl_callback!` in the crate
  (recursively -- the first version of the ruler looked only at `src/*.rs` and missed
  `src/user_stats/stat_callback.rs`) and requires each one to have a `CallbackResult` variant or a
  registered reason, so the 24 callbacks that `CallbackStream` still cannot deliver may only shrink.
  `debt_entries_point_at_callbacks_that_are_still_unstreamed` checks the other direction: a debt
  entry whose callback was renamed, deleted, or wired up in the meantime fails the test rather than
  rotting. Both are backed by `the_two_parsers_see_the_whole_crate`, which fails if either parser
  sees almost nothing -- a count of zero is not a clean bill of health.
  Controls: deleting a `from_raw` arm, renaming a debt entry, and adding an unwired `impl_callback!`
  each turn exactly one of the three red. Each mutation was then undone before the next one, with
  the restore verified by byte-compare for the files it edited and the probe file deleted.


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
- `QueryResults::content_descriptor` used the count Steam reported as a slice bound
  on its own 10-entry array, so an answer of more than ten descriptors panicked;
  the count is now clamped to the buffer. The same array was built with
  `mem::zeroed()`, and Steam's descriptor codes start at 1, so an all-zero value is
  not one this enum may hold -- it is initialised to a real variant now. Both
  findings came from writing the sibling reader
  (`UGC::user_content_descriptor_preferences`) and reading what it had to be careful
  about.
- The four gates above shipped broken the first time: `.gitignore`'s `*.txt` rule excluded
  `scripts/ci/baselines/`, so every fresh checkout was missing the ceilings and all six
  `quality` jobs died with `FileNotFoundError` (run `cc348a8`), while the same gates were
  green on the machine that wrote them. A gate whose inputs are not in the repository
  constrains nobody. Fixed by excepting the baselines from `*.txt`, pinning them to LF, and
  making a missing baseline report which of the two causes applies instead of raising a
  traceback. The verification that was missing the first time is now the rule: run the gates
  from a clean `git clone`, and run the missing-input path deliberately.
- Doc comments: `src/networking.rs` had a malformed intra-doc link (`[../networking_sockets]`)
  and still claimed the `ISteamNetworkingSockets` wrapper was "unfinished" — it is not. Six other
  doc comments carried redundant explicit link targets. Rustdoc now runs with `-D warnings` in
  CI, measured clean on both the default and `--all-features` sets before tightening.
- `CallbackResult` claimed to be "a sum type over all possible callback results" while 28 of the
  66 callbacks the crate defines had no variant, so `CallbackStream` dropped those events without
  saying so. Seven now reach the stream -- the three chat announcements, `FriendRichPresenceUpdate`,
  both `Timeline…RecordingExists` answers and `GlobalStatsReceived` -- and the doc says what the
  type actually covers, pointing at the registered remainder. `set_listen_for_friends_messages`
  also lost its "this wrapper does not yet expose the incoming-message callback" caveat, which was
  true when written and stopped being true in this batch.

### Changed


- `FriendState` gained an `Unknown` variant, and the new `FriendRelationship` enum has one too.
  Adding a variant to a public enum breaks callers who `match` it exhaustively, so this is called
  out rather than buried: the alternative was keeping `Friend::state()`'s `unreachable!()`, which
  is reachable in practice -- `EPersonaState::k_EPersonaStateMax` is Steam's answer for a state the
  binding does not name, and `EFriendRelationship` carries both `Max` and Valve's retired
  `Suggested_DEPRECATED`. Panicking on a documented enum value is the worse defect.
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
