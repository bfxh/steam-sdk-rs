# Changelog

Notable changes to this repository. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the crate version in
`Cargo.toml` tracks the Steamworks SDK release it bundles.

Entries below cite the commit that made the change, so each one is checkable against
`git log`. Everything under **Unreleased** is on `master` but not yet tagged; the crate
still declares version `0.13.1`.

## [Unreleased]

### Added — interface wrappers that upstream never bound

- **Overlay protocol registration (`src/friends/overlay.rs`)**: `register_protocol_in_overlay_browser`,
  the last synchronous entry `ISteamFriends` had left unwrapped -- 75 -> 76 of 78, project total 694 of
  913. Steam's line above the declaration is "Call this before calling ActivateGameOverlayToWebPage()
  to have the Steam Overlay Browser block navigations" (`isteamfriends.h:419`), and the next says what
  arrives instead of a navigation (`:420`), with the declaration itself at `:422`.
  The choice worth recording is the string. The four void overlay setters beside this one cut a value
  at its first nul, because nothing past a nul could ever reach a `const char *` and there is no
  return channel to report anything. This call returns `bool`, so it has a channel, and the two
  refusals are kept apart: `Err(InvalidParameter)` is this wrapper saying the scheme could not be
  expressed, `Ok(false)` is Steam declining a scheme it could have taken. Truncating here would
  register a *different* scheme than the caller named and still report having done it -- the failure
  the CString debt work spent this year eliminating.
  1 test, 1 mutation, red on the assertion message that says so ("an interior nul is this crate's
  refusal, not a truncation and not a panic"). A second test written first was deleted before
  committing: its two assertions restated the first one's, so it could not have judged anything new.
  Registered unverified: whether a running client accepts a given scheme, and the ordering the header
  insists on, which needs the overlay browser to actually exist.

- **The older persona names (`src/friends/history.rs`)**: `persona_name_history`, over the
  declaration Steam introduces as "accesses old friends names - returns an empty string when their are
  no more items in the history" (`isteamfriends.h:219-220`). `ISteamFriends` moves 74 -> 75 of 78 and
  the project total to 693 of 913.
  The header puts the end of the iterator *inside* the call, so there is no count method to pair with
  it and the walk stops at the first `None`. `history_name` answers `None` for three different things
  and the doc says which is whose: the empty string is Steam's declared marker, a null pointer is not
  offered by the header at all, and a buffer that is not UTF-8 is refused rather than converted
  lossily -- replacement characters would look like a handle a caller could compare against.
  `history_index` refuses an entry number that cannot be handed over as Steam's `int`.
  3 tests, 3 mutations, each killed by its own assertion message (empty marker forwarded as a name,
  lossy conversion, cast instead of refusal). Two of this file's citing blocks first landed in
  `~undeclared` for the same reason as the last unit: the phrase has to be at least
  `MIN_PHRASE = 10` characters, so paraphrase-with-quotes does not anchor a block.
  Registered unverified: whether Steam ever returns the null -- there is no client here, and the
  header does not offer that answer either way.

- **The equipped profile items (`src/friends/profile_items.rs`)**: `has_equipped_profile_item`,
  `profile_item_property_string` and `profile_item_property_uint` (`isteamfriends.h:431-433`), with
  `ProfileItemType` (five slots, `:154-158`) and `ProfileItemProperty` (twelve properties, `:162-173`).
  `ISteamFriends` moves 71 -> 74 of 78 and the project total to 692 of 913.
  The property enum is annotated one entry at a time with the reader it belongs to -- "string" or
  "uint32" in the comment column -- and the C interface does nothing to stop a caller asking
  `GetProfileItemPropertyString` for `AppID`, which is a `uint32` property. `property_kind` turns that
  comment column into a table the wrappers enforce before any call, so a mismatched reader is answered
  with `InvalidParameter` rather than with whatever Steam's storage happens to hold. `Unrecognized(i32)`
  on either enum cannot be sent at all: the generated layer declares no out-of-range name for these
  enums, so there is no legal value to build (the same hard limit that shaped the UGC enums earlier).
  `text_or_nothing` returns a string whole or not at all -- a lossy conversion would rewrite an item's
  title into replacement characters that compare against nothing.
  4 tests, 4 mutations, each killed by its own assertion message (AppID moved into the string block,
  an unnamed number given a kind, lossy conversion, an unnamed item type sent as slot zero). Registered
  unverified: what the three calls return for a user whose items were never requested -- that needs
  `RequestEquippedProfileItems`, which is a `STEAM_CALL_RESULT` face and belongs to the callback work.
  `MIN_PHRASE = 10` bit on this file first: two citing blocks quoted only "string"/"uint32", which is
  six characters, so they landed in `~undeclared` (2 spans). They were anchored with sentences from the
  header itself (`:150`, `:427`), not by dropping the citations.

- **The source iterators (`src/friends/sources.rs`)** close the three calls Steam groups under
  "iterators for getting users in a chat room, lobby, game server or clan" (`isteamfriends.h:256-264`):
  `friend_count_from_source`, `friend_from_source` and `is_user_in_source`. `ISteamFriends` moves
  68 -> 71 of 78 and the project total to 689 of 913.
  Two of the three decisions are this crate's, one is Steam's. `source_index` refuses a seat number
  that cannot be handed over as C's `int` (a cast would have asked Steam about a different member than
  the caller named). `source_count` clamps a negative at zero instead of wrapping it into a loop bound
  of 18446744073709551615. `id_or_unset` answers `None` for the value `0` because that is what a
  `CSteamID` holds when nobody wrote into it: Steam's own default constructor zeroes all four of its
  components (`steamclientpublic.h:491-497`) and the two enums among them are declared as `0`
  (`k_EAccountTypeInvalid`, `steamclientpublic.h:230`; `k_EUniverseInvalid`, `steamuniverse.h:17`), so
  an all-zero 64-bit value in a packed union is Steam's encoding of "nothing", not a rule invented
  here. Registered as unverified: what Steam actually returns for a clan its own comment says cannot be
  iterated ("note that large clans that cannot be iterated by the local user", `:257`) -- the header
  states the constraint and no out-of-range answer, so this wrapper neither promises 0 nor promises an
  error for that case.
  3 tests, 3 mutations, each killed by its own test (clamping replaced by `as usize`, refusing replaced
  by a cast, the zero check removed). `header_citations` now measures 47 files / 359 spans with the
  three debt columns at 0-0-0.
  One correction to the friend-group bullet below, found while writing this one: it implied a caller
  has no comparison available for the invalid trailing SteamIDs. It does -- this crate's own
  `SteamId::is_invalid` (`src/identity.rs`) decodes that same account-type field -- and the module's doc
  now says so. It is still not Steam's `BValid()`, which remains absent from the vendored SDK.

- **The five friend-group readers (`src/friends/groups.rs`)** close the part of `ISteamFriends` that
  only needs a group number: `friends_group_count`, `friends_group_id`, `friends_group_name`,
  `friends_group_member_count` and `friends_group_members`, over the declarations at
  `isteamfriends.h:228-238`. The mutators are not missing from this wrapper by oversight -- `CreateFriendGroup`,
  `DestroyFriendGroup`, `AddFriendToGroup` and `RemoveFriendFromGroup` have **zero occurrences** in the
  generated layer, so there is nothing to call; only the reads exist in the vendored SDK.
  Every answer that could be nonsense is decided by a pure function so CI can judge it without a client:
  `group_index` refuses an index that does not fit `c_int` rather than truncating it, `group_id_or_none`
  maps Steam's `k_FriendsGroupID_Invalid` to `None` instead of handing out `-1` as if it named a group,
  `count_or_zero` refuses a negative count where a `usize` would have wrapped it to the largest number
  the type holds, and `name_or_none` refuses a null `const char *` instead of dereferencing it.
  What this module deliberately does **not** do is drop the trailing entries that Valve calls invalid: the
  declaration of `GetFriendsGroupMembersList` says that if fewer members exist than requested
  "those positions' SteamIDs will be invalid" (`isteamfriends.h:237`), and the C++ helper that would filter
  them, `CSteamID::BValid()`, is not in the bindings at all (`steamclientpublic.h:796` has only
  `BValidExternalSteamID`), so deciding "is this seat a real friend" would be this crate inventing a
  rule Steam never gave us. The raw `FriendsGroupID` and the raw count are returned instead, with the
  caller able to compare against `SteamId::new` / their own friends list.
  Four tests, four mutations, each killed by a different assertion message (`group_id_or_none` returning
  `Some(-1)`, `count_or_zero` using `as usize`, `name_or_none` skipping the null check, `group_index`
  casting instead of `try_from`). `ISteamFriends` moves 63 -> 68 of 78 and the project total to 686 of
  913. `MIN_BASELINES` was raised to the measured 15 ledgers, and the first attempt at it -- 16, counted
  wrongly by me -- was refused by that same gate.

- **`ISteamMatchmakingServers` is closed at 17 of 17, because the last call needed a handle to exist**
  (`src/matchmaking_servers.rs`). `CancelServerQuery` (`isteammatchmaking.h:516`) was the only
  interface function still unwrapped, and it could not honestly be wrapped as a bare method: the three
  queries it cancels are issued by `ping_server` / `player_details` / `server_rules` (`:504`, `:507`,
  `:510`), and this crate threw the `HServerQuery` they return away -- there was nothing to pass in. So
  those three now return `ServerQuery`, which owns the handle, keeps the client alive beside it
  (the same `Arc<Inner>` the interface wrappers carry), and exposes `cancel()`. Steam's reason for
  insisting on the cancel is on the declaration: you should do it "before destructing a callback object
  that may have been passed" to the query, "to avoid crashing when callbacks occur" (`:514-515`).
  Two decisions are functions and are judged without a client: `query_id` refuses Steam's own
  no-query marker (`HSERVERQUERY_INVALID`, which the generated layer stores as the `c_int` `-1`; a
  premise pin asserts that number, because `0xffffffff` reaching `c_int` is the whole basis of the
  test), and `cancel_guard` refuses a second cancel as `InvalidState`, the same shape
  `ServerListRequest::release` already enforces -- a cancelled query's number may have been handed to
  somebody else by then, so the second call is not a no-op.
  Three tests, three mutations, all three red on an assertion message (letting the marker through, and
  inverting that comparison, and allowing a second cancel). The FFI bodies remain registered as
  unverified: a query needs a real server to talk to.
  Ledgers: `ISteamMatchmakingServers` 16 -> **17 of 17**, repository total 680 -> 681 of 913,
  `ServerQuery` registered at 1 inherent method, lib test floor at the measured 288, 334 header
  citations with all three debt columns at zero.


- **Cancelling a server-list query, the sixth list type, and the filter length rule that was only a
  comment** (`src/matchmaking_servers.rs`). `CancelQuery` (`isteammatchmaking.h:481`) becomes
  `ServerListRequest::cancel_query` and `RequestSpectatorServerList` (`:394`) becomes
  `MatchmakingServers::spectator_server_list` -- both purely additive, no signature touched -- so
  `ISteamMatchmakingServers` goes 14 -> 16 of 17 and the repository total 678 -> 680 of 913.
  `cancel_query` is documented against the block that asks for it: it is what you call "before
  destructing a callback object that may have been passed" to a list request because "Not doing so may
  result in a crash when a callback" lands on the object that is gone (`:476-477`), and it is **not** a
  release -- "Canceling a query does not release the allocated request handle." and the handle "must be
  released using ReleaseRequest( hRequest )" (`:479-480`) -- so the order the header asks for
  (cancel, then `release`) is spelled out, and calling it on a released request errs like its
  neighbours do.
  The filter-pair length check moved out of the request macro into `filter_pair_fits`, because it was
  an inline comparison whose only justification was a comment. Steam's fields are
  `char m_szKey[ 256 ];` and `char m_szValue[ 256 ];` (`matchmakingtypes.h:40-41`) and its own
  constructor copies with "strncpy( m_szKey, pchKey, sizeof(m_szKey) )" (`:35`) then forces a
  terminator into the last byte (`:36`) -- that is Steam truncating an over-long filter silently,
  which this wrapper refuses to do; the bound is `< 256` because the 256th byte is the terminator.
  Behaviour is unchanged for every caller; the rule is now judgeable, and four mutations against it
  (drop the value half, `&&` to `||`, `<= 256`, `< 255`) all went red on an assertion message. A second
  test pins the layout premise, and its red-check was done by moving its own expectation (assertion
  red, with the message shown) rather than by editing the generated layer -- shrinking the field there
  makes this crate fail to **compile** first, because the request builder writes a 256-element array
  into it, so compile-red there would not have been a lock either.
  Two things are registered rather than fixed. `CancelServerQuery` (`:516`) stays unwrapped because
  `ping_server` / `player_details` / `server_rules` (`src/matchmaking_servers.rs:616`, `:639`, `:664`)
  return `()` and **discard the `HServerQuery`** Steam hands back, so there is nothing to cancel with;
  measured by probe, changing one of them to return the handle costs one line in this crate and breaks
  no in-repo or example call site (`cargo check --locked --all-targets` reported exactly
  `expected 'i32', found '()'` at the `;` of the FFI call, then rc=0 again after restore), so the
  question is which shape to expose -- a typed `ServerQuery` with `cancel()` like the existing
  `ServerListRequest::release`, or the raw `i32` -- and that is a public-API decision, not a coding one.
  `HServerQuery` is `typedef int` (`:377`) with `HSERVERQUERY_INVALID = 0xffffffff` (`:378`), i.e. `-1`
  read as `i32`, which is the kind of trap a bare handle invites.
  Second, a gap in the gate itself was measured while doing this: `type_surface.py` counts
  `pub fn <ident>` and so **cannot see the methods generated by `gen_server_list_fn!`** --
  `MatchmakingServers` is registered as 4 inherent methods (the four written out) while it really has
  9 (plus the five from the macro, now six). The ceiling still holds in both directions, but it is
  guarding four of nine; fixing that means teaching the ruler about macro bodies, registered for
  whoever wants it.

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

- `ISteamInput` controller commands and layer enumeration, from 18 called functions to 35 of its
  48 (`src/input.rs`): `trigger_vibration`, `trigger_vibration_extended`,
  `trigger_simple_haptic_event`, `stop_analog_action_momentum`, `set_led_color`,
  `activate_action_set_layer` / `deactivate_action_set_layer` /
  `deactivate_all_action_set_layers`, `active_action_set_layers`, `current_action_set`,
  `remote_play_session_id`, `session_input_configuration_settings`, `device_binding_revision`,
  `new_data_available`, `wait_for_data`, and both directions of the legacy gamepad-index mapping.
  Two of them carry measured bounds rather than assumptions: `active_action_set_layers` allocates
  exactly `STEAM_INPUT_MAX_ACTIVE_LAYERS` (16, Steam's own constant) and clamps Steam's written
  count with `layers_to_take`, so an over-count cannot read past the array and a negative one
  cannot wrap; `device_binding_revision` returns `None` for Steam's false answer instead of a
  `(0, 0)` pair that a caller could misread as "revision zero", and its mapping lives in
  `binding_revision` so the test runs the production function rather than restating
  `bool::then_some`. Controls L1 (drop `.max(0)`), L2 (drop `.min(capacity)`) and L3 (always
  `Some`) each turned the matching test red.
  `wait_for_data` is documented as parking the calling thread, which is what Steam's own note says
  it is for ("games with a dedicated input thread").

- `ISteamInput` glyph, action-name and origin translation, from 35 called functions to 43 of its
  48: `glyph_for_xbox_origin`, `glyph_png_for_action_origin`, `glyph_svg_for_action_origin`,
  `string_for_analog_action_name`, `string_for_digital_action_name`, `string_for_xbox_origin`,
  `translate_action_origin`, `action_origin_from_xbox_origin`.
  **Breaking**: the two pre-existing string getters
  (`get_glyph_for_action_origin`, `get_string_for_action_origin`) now return `Option<String>`.
  Their bodies were a bare `CStr::from_ptr` on a pointer Steam owns and can hand back as null --
  `from_ptr(null)` is immediate undefined behaviour, not a delayed error -- so every one of these
  eight calls now goes through `steam_static_str`, which reports `None` for the null case and
  otherwise returns lossy text (one invalid byte must not delete Steam's whole answer). Control:
  turning the null case into `Some(String::new())` -- exactly the "empty means missing" confusion
  the change removes -- turns the test red.
  `translate_action_origin` and `action_origin_from_xbox_origin` deliberately return the raw
  binding enum rather than a Rust enum of ours: `EInputActionOrigin` is `#[non_exhaustive]`, so a
  newer Steam can name an origin this crate has never heard of, and forwarding it unchanged is the
  only answer that cannot lose or mislabel it. The header's own contract is cited in the docs
  (`isteaminput.h:1003-1005`).
- `ISteamUser` account-state predicates, from 9 called functions to 14 of its 33
  (`src/user.rs`): `is_behind_nat`, `is_phone_verified`, `is_two_factor_enabled`,
  `is_phone_identifying`, `is_phone_requiring_verification`. These are five one-line forwards with
  no branch to test, so no test was invented for them: the lock is the coverage ledger, where
  dropping any call moves `ISteamUser.called` and the gate judges that in both directions. Stated
  plainly rather than buried -- a negated return value would survive today's suite, because there
  is nothing offline that can observe it. What the wrappers add is the header's contract, which is
  not guessable: `BIsBehindNAT` is "only valid once the user has connected to steam (i.e a
  SteamServersConnected_t has been issued)" and "may not catch all forms of NAT"
  (`isteamuser.h:152-153`), so `false` is not evidence of a direct connection.
- `ISteamUser` duration control, badge level and voice sample rate, 14 -> 17 of its 33:
  `set_duration_control_online_state` with a new input-only `DurationControlOnlineState` enum
  (`Invalid` / `Offline` / `Online` / `OnlineHighPriority`, the four codes and their meanings are
  `steamclientpublic.h:462-465`), `game_badge_level(series, foil)` and
  `voice_optimal_sample_rate()`. The duration-control arms are the one part with logic, so that
  mapping is pinned by a test and mutation-checked twice -- Offline sharing Invalid's code, and
  OnlineHighPriority sending plain Online, each turn it red and the file restored byte-for-byte.
  The equality table is what kills both; the pairwise-distinctness loop the test also carries is
  implied by it and cannot fire on its own, kept as a statement of intent rather than counted as a
  second lock. The other three are single-line forwards and got no test, for the reason in the
  entry above. `DurationControlOnlineState` needs no `Unknown` arm: it is input-only, so nothing
  Steam invents can arrive through it.
- Deliberately **not** wrapped on `ISteamUser`, each judged against the header rather than the
  name, with the register in `docs/SuperSteam-Design.md` §6.1d: `GetHSteamUser` ("only used
  internally by the API", `isteamuser.h:22-23`), `InitiateGameConnection_DEPRECATED` and
  `TerminateGameConnection_DEPRECATED` ("will be removed from the SDK in an upcoming version",
  `isteamuser.h:51`/`:58`). The remaining 13 are blocked on contracts the headers show -- out
  parameters and buffer sizes for the voice cluster, `STEAM_CALL_RESULT` dispatch for four async
  calls, and an unrepresented `eAppUsageEvent` -- not on a preference. (Four of those 13 have since
  been wrapped; see the next entry. Count re-measured: 12 un-wrapped today.)
- `ISteamUser`'s four `STEAM_CALL_RESULT` calls, 17 -> 21 of its 33 (total 625 of 913), and the
  split that came with them: `request_store_auth_url`, `request_encrypted_app_ticket`,
  `market_eligibility` and `duration_control` go through the crate's existing
  `register_call_result` dispatch, so a caller gets a handle-scoped one-shot callback instead of a
  `SteamAPICall_t` to match up by hand. `src/user.rs` was 620 lines against a registered ceiling of
  620, so it was split before the surface grew: `src/user/mod.rs` keeps `User` and its methods
  (371), `auth.rs` the ticket vocabulary and the two `#[ignore]`d manual tests (244), `duration.rs`
  the duration-control types together with the tests that pin them (391), `callbacks.rs` the four
  payloads (61), `answers.rs` the licence and market verdicts (62). Public paths unchanged.
  New types: `MarketEligibility`, `DurationControl`, `DurationControlProgress`,
  `DurationControlNotification`. Three decisions came out of the headers rather than the names:
  `EMarketNotAllowedReasonFlags` is forwarded as the raw bitfield Steam declares it --
  `(1 << 0)`..`(1 << 3)`, `steamclientpublic.h:362-377` -- because a Rust mirror would have to drop
  a combination or present two reasons as one; `m_progress` and `m_notification` stay two enums
  because `steamclientpublic.h:426-454` keeps them two, and `isteamuser.h:416` pins what this call
  can ever see ("always k_EDurationControlNotification_None for API calls"); `m_szURL` is cut at the
  first NUL *inside its own* 512-byte buffer (`isteamuser.h:378`) instead of trusting a terminator.
  `request_encrypted_app_ticket` delivers readiness only -- the ticket bytes need
  `GetEncryptedAppTicket`'s two-pass out-parameter protocol (`isteamuser.h:167-173`), which stays
  unwrapped and in the register above. Both read-direction enums carry an `Unknown` arm no test can
  reach without fabricating a discriminant for the C enum behind them (`#[repr(i32)]` on Windows and
  `#[repr(u32)]` on the other three platform bindings -- undefined behaviour either way), which is
  why the arm is
  documented as unreachable-with-today's-headers and left to the panic-site ledger rather than
  faked. Four tests pin what is reachable (142 lib tests, floor raised from 138): every declared
  code lands on its own variant, no two codes collide, and every struct field arrives in the slot
  named for it. Each was mutation-checked -- progress 3h answering FiveHours, notification 1Hour
  collapsing onto `None`, `m_csecsRemaining` copied into `seconds_today`, a dropped timestamp -- and
  each turned its test red, with the file restored byte for byte.
- `ISteamUser`'s voice cluster, 21 -> 26 of its 33 (total 630 of 913), in the new `src/user/voice.rs`:
  `start_voice_recording`, `stop_voice_recording`, `available_compressed_voice`,
  `read_compressed_voice_into` / `read_compressed_voice`, `decode_voice_into` / `decode_voice`. The
  register's precondition for this cluster was a buffer contract, and the headers carry most of it,
  so the shape follows theirs rather than a guess. The deprecated uncompressed arguments are **not
  exposed at all** -- `isteamuser.h:96-97` tells most applications to leave them NULL/zero and
  `:90-92` says fetching uncompressed audio "will cause GetVoice to discard any leftover compressed
  audio", so putting them in the signature would hand callers a way to lose data silently. The two
  grow loops differ on purpose because only one has a number to grow by: `DecompressVoice` promises
  "*nBytesWritten will be set to the required buffer size, and k_EVoiceResultBufferTooSmall is
  returned. It is suggested to start with a 20kb buffer" (`isteamuser.h:111-113`), so `decode_voice`
  starts there and uses that figure as a floor; `GetVoice` documents no such answer, so
  `read_compressed_voice` can only double -- at most four times, and never past `MAX_VOICE_BYTES`
  (1 MiB), where the outcome is a reported `BufferTooSmall` instead of an unbounded allocation.
  Capture is polled per frame ("at least once per frame, and preferably every few milliseconds",
  `isteamuser.h:100-102`), so the primitives take a caller-owned buffer and the `Vec` versions are
  built on them; `decode_voice` re-issues the FFI call rather than sitting on `decode_voice_into`,
  because the helper discards the byte count and on a refusal that count is the only input the next
  attempt has. `stop_voice_recording` returns nothing: "the system will keep recording for a little
  bit after this function is called" (`isteamuser.h:74-76`), so the drain is observed as
  `VoiceError::NotRecording` on the reader instead of being asserted at the call.
  `VoiceError` maps the nine non-`OK` codes of `EVoiceResult` (`steamclientpublic.h:151-163`) -- a
  declaration with no comments, so each variant's sentence is the name read plainly and the file
  says so rather than dressing it up as quoted prose; its `Unknown` arm is the same
  documented-as-unreachable, un-faked case as the two above it. Two trust rules: `voice_len` clamps
  the length Steam is told *down* only (the other direction truncates and advertises more room than
  the buffer holds), and `written_bytes` believes Steam's count only as far as the buffer allows.
  Four tests, five mutation controls -- NoData answering `NotRecording`, truncation instead of
  clamping, the count trusted outright, the capture loop uncapped, and Steam's reported size taken
  as the whole answer, each turned its test red and the file came back byte for byte. One control
  did **not** kill at first, and the fix went to the function rather than the test:
  `written_bytes` also compared against the length we had declared, which is always `<=` the
  capacity, so no input any caller can produce told the two bounds apart -- the redundant term was
  deleted instead of being covered by a case only a >4 GiB buffer could reach.
- `ISteamUser`'s two remaining out-parameter calls, 26 -> 28 of its 33 (total 632 of 913), in the
  new `src/user/buffers.rs`: `user_data_folder()` and `encrypted_app_ticket()`. The register held
  both behind the same phrase -- "caller buffer plus undefined length semantics" -- and the headers
  only settle it for one of them. `GetEncryptedAppTicket` documents its own two-pass protocol
  ("If no ticket is available, or your buffer is too small, returns false. Upon exit, *pcbTicket
  will be either the size of the ticket copied into your buffer (if true was returned), or the size
  needed (if false was returned)... you can pass pTicket=NULL and cbMaxTicket=0",
  `isteamuser.h:167-172`), so `encrypted_app_ticket` follows it literally: probe, allocate exactly
  the reported size, fetch; a probe reporting zero is `InvalidState` ("no ticket yet", which is
  timing and not the caller's fault), a reported size above `MAX_TICKET_BYTES` (1 MiB) is
  `InvalidParameter` rather than a huge allocation, and a second round that still refuses after being handed its own number is
  `InvalidState` -- the protocol not converging is Steam's problem to explain, not a licence to
  retry forever. The fetched count goes through the same clamp the voice cluster uses, shared as
  `voice::written_bytes` rather than written out a second time.
  `GetUserDataFolder` is the opposite case: it returns a bare `bool` and the header never says what
  `false` means (`isteamuser.h:67-69`), so the wrapper does not invent an interpretation. It gives
  Steam a 32 KiB buffer -- the example Steam itself prints, `"C:\Progam Files\Steam\userdata\
  <SteamID>\<AppID>\local"`, is a fraction of that and 32 KiB is already >100x Windows' 260-char
  path limit -- and detects the failure mode Steam cannot be trusted to describe: if the buffer
  comes back full with **no NUL inside it**, the path was cut, and that is an error rather than a
  string. The reasoning is that a truncated path still *reads* like a path, and handing one back
  would let a game write its saves somewhere it did not choose. A `false` from Steam is reported as
  `InvalidState` with nothing inferred beyond "not granted"; `cubBuffer` being an `int` goes through
  `i32::try_from` first (the other direction advertises room the buffer does not have); and the
  bytes are decoded lossily like every other fixed Steam buffer in this crate, with the docs saying
  plainly that an undecodable byte becomes U+FFFD and the result is therefore not byte-round-trip
  exact. `request_encrypted_app_ticket`'s doc, which pointed at an unwrapped getter, now links
  [`User::encrypted_app_ticket`] -- that sentence went stale the moment this unit existed.
  Four tests, four mutation controls (no terminator guard, `int` length truncated instead of
  refused, a zero probe treated as a size, the ticket cap removed), each turning its test red and
  the file restored byte for byte.
- `ISteamUser` is now at 30 of its 33 declared functions (total 634 of 913) with the last two
  parameter-vocabulary cases wrapped in the new `src/user/passthrough.rs`: `advertise_game(SteamId,
  Ipv4Addr, u16)` and `track_app_usage_event(GameId, i32, &str) -> Result<(), SteamError>`.
  `AdvertiseGame`'s `uint32 unIPServer` is the interesting one, because the header describes it as
  "the IP address of the game server" and then says *nothing* about byte order
  (`isteamuser.h:156-158`). Rather than invent a rule, the wrapper takes the one this crate already
  uses at both ends and can be checked against itself: `MatchmakingServers::ping_server` sends
  `ip.into()` outward (`src/matchmaking_servers.rs:616-633`) and the server-list reader turns an
  address Steam reported back with `Ipv4Addr::from(raw.m_NetAdr.m_unIP)` (`:213`). Those round-trip,
  and measured on this host `u32::from(addr) == addr.to_bits()` -- first octet most significant, so
  `192.168.1.1` is `0xC0A80101`, the shape Steam itself hands back. The doc says all of that,
  including that the call returns nothing (`void`) so an unreachable address is still "accepted", and
  that a caller reading the word order differently should take `sys::SteamAPI_ISteamUser_AdvertiseGame`
  on purpose -- a deliberate choice beats a wrapper guessing.
  `TrackAppUsageEvent` is the opposite shape: `eAppUsageEvent` is a bare `int` and this SDK ships no
  `EAppUsageEvent` to name its values, so the number is forwarded unchanged and **no Rust enum is
  invented** -- a table of names this binding cannot check is the same mistake as the
  write-direction placeholders. The header's only remark is "used by only a few games to track usage
  events" under its own `// Legacy functions` banner (h:63-64), and the string argument goes through
  the crate's shared `c_string_arg`, so an interior NUL becomes `InvalidParameter` instead of a panic
  or a truncated note.
  Neither method carries a test, and that is stated rather than hidden: `advertise_game` is one
  forward with no branch to take, and `track_app_usage_event`'s only branch needs a live `Client`, so
  a fake client would produce an assertion no mutation could kill. The lock is the coverage ledger --
  `ISteamUser.called` drops if the FFI call disappears, and that gate judges in both directions --
  which is the same treatment the five account-state predicates got.
- `ISteamNetworkingFakeUDPPort` — the ad-hoc datagram port of the FakeIP system, previously one of
  the four "0 of N" rows the design doc §6.1c called a real gap rather than a structural exemption.
  `NetworkingSockets::create_fake_udp_port(idx) -> Option<FakeUDPPort>` plus `send_to_fake_ip`,
  `receive_messages`, `schedule_cleanup` and `destroy`, in the new `src/networking_sockets/fake_udp.rs`.
  `ISteamNetworkingFakeUDPPort.called` 0 -> 4, `ISteamNetworkingSockets.called` 25 -> 26, total 639 of
  913.
  **No `Drop`, and that is the API decision most worth reading.** Steam's own factory comment says a
  server port obtained twice is the *same* object and "this object is not reference counted"
  (`isteamnetworkingsockets.h:906-908`). An automatic `DestroyFakeUDPPort` in `Drop` would therefore
  kill a handle some other live wrapper is still using -- a use-after-free with a polite RAII face.
  So disposal is `destroy(self)`, taking the value: Rust can then hand out no second destroy from one
  wrapper, and the two-wrappers-for-one-index aliasing stays where it is visible. The same comment is
  why `receive_messages` takes `&mut self`: "this function call is not threadsafe with respect to any
  other method of this interface. (However, in general all other operations are threadsafe...)"
  (`steamnetworkingfakeip.h:33-35`) -- exclusivity is how Rust says that, and `&self` is kept for the
  operations Steam allows to run concurrently.
  Two documented numbers became checked ones: `FAKE_UDP_RECOMMENDED_MTU` (1200, a recommendation and
  deliberately *not* enforced -- refusing above it would invent a rule Steam does not have) and
  `FAKE_UDP_MAX_MESSAGE_BYTES` (4096, "Messages larger than this size are not allowed and cannot be
  sent", `steamnetworkingfakeip.h:17-18`), which the wrapper refuses locally instead of spending a
  call. The first test reads both constants back out of the vendored header at run time, so a future
  SDK bump that changes them turns the test red rather than leaving a stale copy in Rust -- and that
  test earned its place immediately: the parser's first version matched only a line *prefix* and the
  header writes `constexpr int <name> = <value>;`, so it reported "not found" for a header that does
  define them. Reading at run time with `env!("CARGO_MANIFEST_DIR")` and skipping out loud follows the
  existing `generated_bindings_expose_the_same_api_on_every_platform` (`src/lib_tests.rs:288-301`),
  because a packaged crate does not carry the `-sys` headers.
  `receive_messages` clamps Steam's returned count with `received_count` before using it as a length:
  a negative `int` would wrap to an enormous `usize`, and a count above the batch size would claim
  slots nobody offered. **Worth recording as an observation about the existing code**: `NetPollGroup`'s
  sibling did `... as usize; set_len(count)` with no such
  clamp, so the same hazard was live there. It is not fixed in this commit -- it is a different
  surface, and changing a hot path that games already use deserves its own measurement rather than
  riding along as a drive-by.
  **Corrected 2026-10-18: that hazard is no longer live.** `NetPollGroup::receive_messages` now routes
  Steam's return through `message_count` (`src/networking_sockets/poll_group.rs:64`), which reads
  "If the connection handle is invalid, -1 is returned" (`isteamnetworkingsockets.h:321`) as zero and
  truncates a count above the array's capacity; the call site is `src/networking_sockets/poll_group.rs:85`.
  Disclosed in the same breath: `FakeUDPPort::receive_messages` already had exactly this clamp as
  `received_count` (`src/networking_sockets/fake_udp.rs:47`, tested at `:313-320`), so there are now two
  implementations of one rule in this module; consolidating them is queued rather than done here, because
  moving a function two sibling modules share changes files this unit otherwise would not touch.
  Three tests, four mutation controls: the copied ceiling drifting from the header, the maximum not
  enforced, a negative count trusted, and the batch clamp removed -- each turned its test red, files
  restored byte for byte.
- FakeIP allocation, the two calls of `ISteamNetworkingSockets` that the generated layer lets this
  crate reach: `begin_async_request_fake_ip(num_ports) -> bool` and
  `create_listen_socket_p2p_fake_ip(idx_fake_port, options) -> Result<ListenSocket, InvalidHandle>`, in
  the new `src/networking_sockets/fake_ip.rs`. `ISteamNetworkingSockets.called` 26 -> 28 and the total
  to 641 of 913. **This entry arrived late.** The commit that added the code promised, in its own last
  paragraph, that it had updated `docs/SuperSteam-Design.md`'s coverage table row and total; `git show
  --stat 306bbf1` lists six files and no doc, so that paragraph described an intention rather than the
  diff, and this entry -- which the repository's convention requires for any added wrapper and which
  that commit also lacked -- is written now rather than then. The code, the wiring, the ledgers and the
  header citations really were in it; only the prose was missing.
  `begin_async_request_fake_ip` returns Steam's `bool` rather than a `Result` because `false` is not a
  failure: `isteamnetworkingsockets.h:835-836` defines it as "a request was already in progress", and
  both answers are normal. No ceiling is imposed on `num_ports`: the limit exists only as prose
  (":829-830", "At the time of this writing, the limit is 4") and a case-insensitive search of every
  vendored header and all four bindings files finds no constant for it -- only
  `steamnetworkingfakeip.h:4`, the include guard -- so checking it would mean inventing the number.
  ":831-832" also warns the assignments are neither ordered nor contiguous, so nothing pretends index
  `i + 1` follows index `i`. The listen-socket half takes an **index into the reservation**, not a port
  (":873-875"), and the pair is safe to expose without the completion message because ":875-877" allows
  the socket to be created while the allocation is still pending.
  What is still not wrapped, and why each reason is different: `GetFakeIP` (h:867) and
  `ISteamNetworkingUtils::SetGlobalCallback_FakeIPResult` are blocked by the generated layer --
  `SteamNetworkingFakeIPResult_t` is bound as the opaque `windows_bindings.rs:17499-17501` shell
  `_unused: [u8; 0]` while the real definition is `steamnetworkingfakeip.h:93`, so feeding that header
  to bindgen and regenerating is a decision about the frozen generated layer, not this module.
  `GetRemoteFakeIPForConnection` (h:895) is reachable -- `SteamNetworkingIPAddr` is bound concretely and
  `From<sys::SteamNetworkingIPAddr> for SteamIpAddr` already exists at
  `src/networking_types/addresses.rs:183-187` -- but every public address-facing method here speaks
  `std::net::SocketAddr` and keeps `SteamIpAddr` crate-internal, so wrapping it means deciding what a
  FakeIP is in the public API; it is registered, not guessed. Neither new method has a test or a
  mutation control: both need a live `ISteamNetworkingSockets` to answer anything, and there is no
  branch in them for a test to pin.
  *The paragraph above about `GetFakeIP` being blocked by the generated layer is superseded by the next
  entry: `41cf3fb` regenerated the bindings and the struct stopped being a shell.*
- FakeIP read-back, plus its completion message on the callback pipe: `NetworkingSockets::get_fake_ip()
  -> Result<FakeIpAllocation, SteamError>`, `FakeIpAllocation { ip, ports, identity }`, and `FakeIpResult`
  registered in `CallbackResult`, all in `src/networking_sockets/fake_ip.rs`. `ISteamNetworkingSockets.called`
  28 -> 29 and the total to **643 of 913**; `NetworkingSockets`' inherent surface 15 -> 16.
  `idxFirstPort` is deliberately not a parameter: `isteamnetworkingsockets.h:865` says it "is currently
  reserved and must be zero", so the wrapper passes 0 instead of exposing a knob with one legal value, and
  `:866` -- "Make sure and check `SteamNetworkingFakeIPResult_t::m_eResult`" -- is why the answer is a
  `Result` rather than the struct. Both lines are in the file's citation table, so they are re-checked
  against the vendored header on every run rather than trusted from this prose.
  The address is `Ipv4Addr::from(u32)` with no byte swap because `steamnetworkingtypes.h:205` defines the
  integer as `aa.bb.cc.dd`, and `ports` keeps only the leading run of Steam's eight-entry array
  (`steamnetworkingfakeip.h:123-125` for the leading run, `:129-130` for the width), so a zero in the
  middle ends the reservation instead of being skipped. The four refusals Steam names (`:98-102`) keep
  their own `SteamError` variants rather than collapsing into `Generic`, because `:104-105` singles Busy
  out as the only failure a poller may tolerate.
  **Three of those `steamnetworkingfakeip.h` spans were wrong when this entry was written, and each of
  them was two to six lines off** -- I had typed them from a reading rather than re-reading the header
  after laying out the text. So the citations this file makes against that header are now a ruler, not
  prose: `every_fakeip_header_citation_lands_on_the_sentence_it_quotes` checks seven spans against the
  vendored file, and `a_fakeip_citation_point_at_the_wrong_line_is_red` proves that ruler can fail by
  checking a span three lines below the leading-entries note, which does not contain its phrase.
  Control: mutate one phrase in the table -> the new test is the only thing that goes red; restored
  byte-for-byte.
  `SetGlobalCallback_FakeIPResult` is still not wrapped, but the reason changed: not the layout -- which is
  now real, `windows_bindings.rs:27038-27046` with `k_iCallback = 1223` at `:27053` -- but that this payload
  already arrives through the installed-callback stream, while the SDK's global callback is a bare function
  pointer with nowhere to put a capture. Wrapping it would offer a second, weaker route to a message that
  is already delivered.
  Six new tests across the two files -- five in `fake_ip.rs`, which now holds seven, and one in
  `src/callback.rs` -- all offline, plus four mutation controls:
  `take_while` -> `filter` in `allocated_ports` -> `only_the_leading_ports_of_the_reservation_are_reported`
  red; deleting the `FakeIpResult::ID` arm of `CallbackResult::from_raw` -> `the_fake_ip_result_reaches_the_stream`
  red; collapsing every mapped code into `Generic` -> both of `the_four_fake_ip_refusals_keep_their_names`
  and `an_unmapped_result_code_is_an_error_and_not_a_success` red; reading `k_EResultNone` as success ->
  the second of those red. Each file was restored with `cp` and checked byte-for-byte with `cmp`, and the
  suite is green at 146 passed on the restored tree. The wrong half of this work was not caught by any
  mutation but by two tests that already existed; that is recorded under **Fixed**.
  One test that nearly shipped is worth naming as *not* written: `assert_eq!(FakeIpResult::ID,
  sys::SteamNetworkingFakeIPResult_t_k_iCallback)` looks like it pins the callback number, but
  `impl_callback!` *defines* `ID` as that exact constant, so the assertion cannot fail for any reason
  except someone editing the macro. The id is pinned indirectly instead, by the dispatch mutation above.

Ledgers for this commit: `api_coverage` `ISteamNetworkingSockets.called` 28 -> 29 (total 643),
`type_surface` `NetworkingSockets` 15 -> 16, `module_sizes` `src/networking_sockets/fake_ip.rs` 195 -> 395,
`src/callback.rs` 543 -> 560, `src/lib.rs` 814 -> 829, `src/networking_sockets/mod.rs` 260 -> 261, and the
lib test floor 153 -> 161. `package_inventory`, `sdk_vendored`,
`no_panic_sites` and `shell_hatches` compare **unchanged**; the last of those is the interesting one, see
the drift-artifact entry under **Added -- CI and release automation**.
  The floor needs its own sentence, because two rulers disagreed and only one of them was the gate: the
  run reports 161 today, and six tests really are new here, but `src/` held **155** attribute-form
  `#[test]` lines at the previous commit against a registered floor of **153** -- measured both ways with
  `git grep` on the attribute pattern, 155 at `41cf3fb` and 161 at this commit, which is exactly +6. So
  the ledger moved by eight while six tests arrived: the unit before this one added two tests without
  re-recording the floor. That is the same class of miss as a stale size ledger, and the reason the
  register is raised to the measured number rather than left as a comfortably low bound -- a number that
  cannot be exceeded by reality cannot notice reality shrinking either.
- `ISteamNetworkingMessages` is now a **complete** interface: 6 of 6, where it had been 5 of 6. Added
  `close_session_with_user(&NetworkingIdentity) -> bool` and
  `close_channel_with_user(&NetworkingIdentity, i32) -> bool`, which fill the last two entries of
  `steamworks-sys`'s six for this interface (`AcceptSessionWithUser`, `CloseChannelWithUser`,
  `CloseSessionWithUser`, `GetSessionConnectionInfo`, `ReceiveMessagesOnChannel`,
  `SendMessageToUser` -- measured with `grep -o` over the bindings, sorted unique). Until now
  `CloseSessionWithUser` was reachable only as a side effect of `SessionRequest::reject`, i.e. only for
  a session that had never been accepted; a session you finished using had no way to give the
  resources back early. `isteamnetworkingmessages.h:103-108` is the reason both are `bool` and not
  `Result`: the header says what the call does ("immediately free up resources under-the-hood", a
  re-used peer posts a fresh `SteamNetworkingMessagesSessionRequest_t`) and adds that idle sessions
  time out by themselves after a few minutes, but it never defines what `false` means, so turning that
  into an error would be this wrapper's invention. `local_channel` is the same routing number
  `send_message_to_user` takes, and `:110-113` says closing the last channel closes the session.
  Total coverage 641 -> 642 of 913; `type_surface` counts `NetworkingMessages` at 5 -> 7. No test: both
  need a live `ISteamNetworkingMessages` pointer and the methods contain no branch for a test to pin.
- `src/input.rs` (725 lines, three separate `impl Input` blocks and three test modules in one file)
  became `src/input/`: `origins.rs` 178 (the glyph and string reads for an action origin or an
  `EXboxOrigin`, the two glyph formats, the origin translations, the `steam_static_str` helper every
  one of them reads through, and `static_string_tests`), `actions.rs` 104 (handles by name, digital and
  analog state, the origins behind an action, motion), `layers.rs` 96 (set and layer activation, the
  layer listing and its clamp, plus `mod input_tests`), `callbacks.rs` 83 (`DeviceConnected`,
  `DeviceDisconnected` with their `impl_callback!`s, and `device_callback_tests`), `roster.rs` 67 (the
  two roster shapes and the per-handle `InputType`), `effects.rs` 87 (vibration, extended vibration,
  simple haptics, analog momentum, LED colour), `core.rs` 58 (`init`, `run_frame`,
  `enable_device_callbacks`, the manifest path, the binding panel, `shutdown`), `session.rs` 57 (Remote
  Play session id, session configuration, binding revision, new-data and wait, the gamepad index map in
  both directions), and `mod.rs` 53 (imports, the `Input` struct, `InputType`, the two file-level
  helpers and the wiring). The crate's largest file is still `src/lib.rs` at 814; the next is
  `src/utils.rs` at 710.
  48 items, 0 unclaimed lines, witness 658 non-blank before / **0** lost / 48 added and every added line
  wrapper-shaped -- nothing widened, because `Input`'s fields are `pub(crate)` and the free helpers
  landed in the parent where both callers and the test module can reach them. `api_coverage` (642 of 913,
  `ISteamInput` 44) and `type_surface` (`Input` 45) compare green against the pre-split baselines;
  `module_sizes` (115 entries, 27,315 lines) and `package_inventory` (130 shipped files) were
  re-recorded, and `cargo test --lib input` reaches 9 tests, all passing.
  Two things this split got wrong before it got right, both worth recording because they are the same two
  traps the series keeps hitting. I typed the three `impl Input` ranges as `(29, 265), (276, 466), (548,
  640)` -- numbers that are really "next anchor minus one" from the probe, not the impls' own closers --
  and the true spans are `(29, 259), (276, 460), (548, 639)`, so `core.rs` shipped with a stray closing
  brace that only `cargo check` could see. The generator now computes every closer by brace balance from
  its own opener and asserts there are exactly three. Then `pub use origins::*;` and `pub use roster::*;`
  were refused as unused imports for the sixth time in this series, because those children hold only
  extra `impl` blocks and export nothing -- so only `callbacks` is globbed out, and the rest are declared.
- `src/utils.rs` (710 lines: one `impl Utils` of 28 methods spanning lines 130-450, plus 26 other top-level items) became
  `src/utils/`: `text_input.rs` 178 (the three controller-typing vocabularies with their write-side
  `From`s, and show / read / length / dismiss for both the plain and floating box), `mod.rs` 142
  (imports, the `Utils` struct, the wiring, and the 105-line `#[cfg(test)] mod tests` (old lines 567-671) that stayed in
  the column-0 shape `api_coverage.py` strips on), `filtering.rs` 103 (init, the filter call, the
  context vocabulary, the `FilteredText` record and the buffer copy that builds it), `ipv6.rs` 60,
  `identity.rs` 56 (app id, IP country, UI language, server time, the two idle reads, the IPC
  counter), `warning.rs` 54 (the process-wide slot behind an `RwLock`, the `extern "C"` trampoline
  that catches a panicking user callback, and `set_warning_callback`), `overlay.rs` 47 (needs-present
  and the notification position/inset), `param_strings.rs` 47, `client_state.rs` 42 (Big Picture,
  Steam Deck, VR running and the headset-streaming switch, the China launcher), `battery.rs` 34 and
  `callbacks.rs` 29 (the two dismissal payloads and the corner enum). The crate's largest file is
  still `src/lib.rs` at 814; the next two are `src/networking_types/config.rs` 629 and
  `src/error.rs` 603.
  54 items, 0 unclaimed lines, witness 642 non-blank before / **2** lost / 70 added, and every one of
  the five non-wrapper additions is named by this paragraph: the two lost lines are
  `ipv6_state_from_raw` and `filtered_text_from_buffer` becoming `pub(super)`, their two replacements,
  the `pub(crate) use param_strings::*;` that keeps `crate::utils::SteamParamStringArray` resolving for
  `src/ugc/update.rs` and `src/ugc/ugc_tests.rs`, and the two `use super::...` lines the root's test
  module needed once those converters lived a level down. `api_coverage` (642 of 913, `ISteamUtils` 32)
  and `type_surface` (`Utils` 28) compare green against the pre-split baselines; `module_sizes`
  (125 entries, 27,397 lines) and `package_inventory` (140 shipped files) re-recorded; `cargo test
  --lib utils` reaches 10 (9 passed, 1 ignored) and the suite still lands at 155.
  The impl's closer was computed again rather than typed -- `(130, 450)` by brace balance -- and two of
  the eleven anchors were wrong on the first pass: I wrote `static WARNING_CALLBACK` at 101 and
  `c_warning_callback` at 104, but 101 is its doc line and the static sits at 103, so the generator's
  decl-line assert (which checks the *declaration*, not the span start) refused and named line 101 as
  blank. Same class as the friends unit's recalled impl span: the probe catches a range, only an
  assertion on the item line catches an off-by-two anchor.
- **The generated layer can now see the FakeIP result struct**, which is the prerequisite for the two
  FakeIP calls that were un-wrappable for a structural reason rather than a choice.
  `steamworks-sys/build.rs` fed bindgen two headers (`steam_api_flat.h`, `steam_gameserver.h`), and
  nothing in the vendored SDK includes `steamnetworkingfakeip.h` -- `grep -rn "#include.*fakeip"` over
  `steamworks-sys/lib/steam/public/steam` reads zero hits, so the only sighting of
  `SteamNetworkingFakeIPResult_t` was the forward declaration at `isteamnetworkingsockets.h:11`, and
  bindgen turned that into the opaque `pub struct SteamNetworkingFakeIPResult_t { _unused: [u8; 0] }`
  at `windows_bindings.rs:17499-17501` while the real definition sits at `steamnetworkingfakeip.h:93`.
  The input is now one bundle translation unit -- `header_contents("steam_bindings_bundle.h", ...)`
  including `<steam/steam_api_flat.h>`, `<steam/steam_gameserver.h>` and then
  `<steam/steamnetworkingfakeip.h>` -- rather than a third `.header()`, because the fake-IP header
  cannot be parsed alone: it reaches `steamnetworkingtypes.h:47`, which stops with
  `#error "Must define VALVE_CALLBACK_PACK_SMALL or VALVE_CALLBACK_PACK_LARGE"` unless
  `steamclientpublic.h` (the only definer, at its 1149/1151) came earlier in the *same* translation
  unit, and macros do not cross translation units.
  **No binding file is regenerated by this commit, and that is deliberate.** Each of the four
  bindings files is produced by the runner whose target it belongs to -- `Rebuild bindings` runs
  `cargo build --features rebuild-bindings` on windows-latest, ubuntu-latest, macos-latest and
  ubuntu-22.04-arm, then rustfmts the result before diffing, because a bindgen run on the wrong host
  would emit the wrong `#[repr]` width for the same C enums -- and the vendored headers are
  byte-frozen behind `scripts/ci/sdk_vendored.py`'s hash gate, and this box has no libclang on PATH and
  no `C:/Program Files/LLVM` at all (measured), so the change is verified the way the repository
  already verifies generated code: through `Rebuild bindings`, whose `compare` job reports drift and
  whose `apply` job only writes when a human checks `commit_bindings`. This commit therefore changes
  what *can* be generated, not what is checked in; `ISteamNetworkingSockets` stays at 28 of 47 until
  the regenerated bindings land and the wrapper is written against them.
- No API change, one structure change: `src/matchmaking.rs` was the largest file left in the crate
  (1339 lines) and the last big module still in single-file form, so it became a directory module --
  `src/matchmaking/mod.rs` 478 (`Matchmaking`, `LobbyType`, `LobbyId`, the lobby lifecycle and the
  game-server pair, plus the two `#[ignore]`d manual tests), `filters.rs` 314 (the lobby-list filter
  vocabulary and the key-length rule), `chat.rs` 280 (chat send/paging plus the entry and member
  enums with the flag test), `queries.rs` 205 (the `add_*_filter` / `set_*` builder chain), and
  `callbacks.rs` 111 (the five payloads Steam fires). The biggest impl -- `Matchmaking`'s own, 17
  methods -- is now four `impl Matchmaking` blocks, which is the same shape the voice, buffers and
  passthrough files of `src/user/` already use.
  Public paths are unchanged (`mod matchmaking;` at `src/lib.rs:69` reaches the directory
  unchanged, and the existing `pub use crate::matchmaking::*;` at `src/lib.rs:37` keeps every item
  at its old path -- both numbers re-read from the file, because the first draft of this line named a
  line number remembered from an earlier grep and it was wrong), and the type-surface and coverage ledgers
  confirmed it: both compare green against the *previous* values, so no key was added, removed or
  grown. `lib` tests are still 153 (138 passed + 15 ignored) -- nothing was dropped or silently
  renamed by the move.
  The witness for "moved, not lost" is a line-level multiset difference between
  `git show HEAD:src/matchmaking.rs` and the five new files: 1247 non-blank source lines before, and
  exactly 5 lost, all of them the two intended edits -- the four lines of the
  `use std::{ fmt::Display, net::{Ipv4Addr, SocketAddrV4} }` block (now `use std::net::{...}`, since
  `Display` moved to `filters.rs`) and the `fn chat_member_state_change` line, widened to
  `pub(crate)` because `callbacks.rs` needs it and the split put the two in different files. 32 lines
  are new: the five module headers, the `mod`/`pub use` wiring, and the impl braces.
  Two of the gate's findings during this work are worth keeping: `pub use queries::*;` was refused by
  the compiler as an unused import -- that file exports no items, being only a second `impl` block --
  so it is declared without a glob, and the splitter initially leaked `impl Matchmaking {` itself
  into the previous item's body, producing a file with three impl openers and two closers, which is
  what `cargo check` (not the eye) caught.
- `src/networking_types/mod.rs` (1100 lines) became wiring plus topic files: `mod.rs` 36,
  `state.rs` 180 (connection state and the availability ladder), `events.rs` 212 (status-change
  callback and the connect/disconnect structs), `identity.rs` 182, `message.rs` 206,
  `addresses.rs` 246, and the three `NetworkingConfigEntry` items appended to the `config.rs` that
  already existed (551 -> 629). Nothing is left in the parent but its doc line, its imports and the
  child wiring, so the crate's largest file is now `src/inventory.rs` at 966.
  Public paths are unchanged and the behavioural ledgers say so: `api_coverage` and `type_surface`
  both compare green against the pre-split baselines, `lib` tests are still 153 (138 + 15 ignored)
  and `doc` still 16. The moved-not-lost witness is a line-level multiset difference over the whole
  directory against `HEAD`: 2071 non-blank lines before, exactly **2** lost, and both are field
  declarations that had to widen -- `NetworkingIdentity::inner` and `SteamIpAddr::inner` became
  `pub(super)` because the split left their readers in sibling files (`message.rs` fills a Steam
  identity out of the first, `identity.rs` builds a `SteamIpAddr` from the second). 30 lines are
  new: five module headers, the two widened fields with the reason beside them, and one re-export
  note.
  Two lessons here were learned by breaking things rather than by design. The first attempt moved
  the imports into each child and emptied the parent, which broke every child that had been reaching
  `sys`, `Arc` and the serde derives through `use super::*` -- 126 errors, restored and redone with
  the parent's import block left where it was, because it is load-bearing. Then a trim loop deleted
  imports that were not unused, because it acted on rustc's `unused_imports` warnings while errors
  were still standing in the same build; a warning emitted over a broken tree is not a signal, and
  the correction is to trim only after `cargo check` returns without errors. One new warning was
  also answered rather than ignored: `pub use addresses::*;` re-exported nothing public, since that
  child holds only the crate-internal `SteamIpAddr`, so it is now `pub(crate) use`, no broader than
  the item.
- `src/inventory.rs` (966 lines, the largest file left in the crate) became `src/inventory/`:
  `result.rs` 338 (`InventoryResult` with its `Drop`, the batch `InventoryUpdateHandle`, and the one
  private helper every request funnels through), `items.rs` 204 (`ItemDef`, `ItemInstanceId`,
  `ItemDetail`, the flag helper and the flag/id tests), `prices.rs` 132, `actions.rs` 121 (consume,
  exchange, transfer, trade, drop), `definitions.rs` 101, `mod.rs` 83 and `callbacks.rs` 44. The
  22 methods on `Inventory` are now six `impl Inventory` blocks by concern rather than one list.
  Both behavioural ledgers were re-run against the *pre-split* baselines and hold: `api_coverage`
  (639 called of 913, unchanged) and `type_surface`, which counts `Inventory` at 21 inherent `pub fn`
  before and after -- so the ruler aggregates per type across files rather than per file, and a split
  cannot hide surface by spreading it out. `lib` tests are still 153 (138 + 15 ignored).
  Witness: 890 non-blank lines before, 5 lost, and every one of them is a declaration that had to
  widen -- `InventoryResult::{inv, handle}`, `InventoryUpdateHandle::{inv, handle}`,
  `start_result` and `read_property_buffer`, each now `pub(super)` with the reason beside it, because
  the split put a reader in a sibling file (the two handle structs are built by `mod.rs` and
  `definitions.rs`, and all 21 requests go through `start_result` from three different files).
  Nothing else moved: 41 lines are new, all module headers and the widened declarations.
  Two wiring findings repeat earlier ones and are worth having in one place: `pub use actions::*;` and
  `pub use definitions::*;` were refused as unused imports because those files export no items of
  their own (they hold only extra `impl` blocks), so they are declared without a glob; and
  `use std::os::raw::c_char;`, copied into every child by the generator, was dropped from the four
  that do not name `c_char` -- trimmed only after `cargo check` came back without errors, per the
  rule the networking_types split bought.
- `src/user_stats.rs` (868 lines) became `src/user_stats/`, joining the `stat_callback.rs` and
  `stats.rs` that directory already had: `leaderboard.rs` 384 (the eight leaderboard calls, the
  seven leaderboard value types, and the manual leaderboard round-trip), `global_stats.rs` 280
  (`request_global_stats` and the four global-value/history readers, plus the two global-stat tests),
  `stats_io.rs` 127 (the request-store-reset lifecycle and the four single-stat accessors),
  `achievements.rs` 90 (the achievement helper entry point, the count, the name list, and the opt-in
  global-percentage request), and `mod.rs` 20 -- wiring plus the `UserStats` struct and nothing else.
  `UserStats`' 24 methods are now four `impl UserStats` blocks by concern, and the crate's largest
  file is `src/html_surface.rs` at 842.
  No field or method had to widen this time: the witness is a line-level multiset difference against
  the pre-split file, 805 non-blank lines before, **1** lost -- the `/// Access to the steam user interface` doc line this same entry then rewrote --
  and the 28 new lines are otherwise all wrappers
  (four `use super::*;`, three extra `impl UserStats {` and their braces, the four three-line module
  headers, four new `mod` lines and one `pub use leaderboard::*;`). The two behavioural ledgers were
  re-run against the *pre-split* baselines and hold unchanged -- `api_coverage` still counts
  `ISteamUserStats` at 31 and the total at 639 of 913, `type_surface` still counts `UserStats` at 24
  -- and `lib` tests are still 153 (138 passed + 15 ignored), so nothing was added, removed or renamed
  by the move. Public paths are unchanged: `Leaderboard`
  and its types reach the crate root through the parent's re-export exactly as before, and
  `src/lib.rs:51`'s `pub use crate::user_stats::*;` was re-read rather than recalled.
  One ordering bug was caught by reading my own generated file rather than by a gate: the first pass
  emitted a file's plain items, then its impl, and the `#[test]` blocks went in source order with the
  plain items -- which put both manual tests *above* the methods while the `leaderboard.rs` header I
  had just written said the manual test sits "at the bottom". The generator now puts `#[test]` blocks
  last, and the re-run's witness still reads 1 lost / 28 added, so the sentence is true of the file a
  reader actually opens.
  One path reference does move with a split, and only one: `achievement()`'s return type was written
  `stats::AchievementHelper`, which resolved because the parent's `use super::*` makes the sibling
  `stats` module visible to the file it lands in -- so it needed no edit, and the check that proved it
  was `cargo check --locked --all-targets` rather than a reading of the generated text. The struct's
  own doc line was also wrong before this commit and is fixed in it: it read "Access to the steam user
  interface", which is `User`'s interface (`ISteamUser`), not this one's; it now says user stats.
- `src/html_surface.rs` (842 lines) became `src/html_surface/`: `browser.rs` 352 (the `Browser`
  handle, its `Send`/`Sync` claims, its 33 methods as one `impl` block, and the `Drop` that removes
  the browser on the Steam side), `callbacks.rs` 247 (the twelve notification payloads, each moved
  together with the `impl_callback!` that registers it), `mod.rs` 206 (the `HtmlSurface` interface,
  the two shared string helpers `cstr` and `owned`, `impl HtmlSurface`, and the test module), and
  `keys.rs` 63 (the mouse-button and key-modifier vocabularies with their raw mappings). The crate's
  largest file is now `src/server.rs` at 821.
  This split moved whole top-level items and cut nothing open -- every anchor is a `pub struct`,
  `impl`, `unsafe impl`, `fn` or `mod`, so a block and its braces travel together and the generator
  never re-wraps a method list. 25 items, 0 unclaimed lines out of 842, and the witness is again a
  line-level multiset difference: 762 non-blank lines before, **4** lost, and all four are the
  declarations that had to widen -- `Browser::{html, handle}` and both `as_raw` methods became
  `pub(super)`, with the reason written beside each -- and 25 added lines are headers, wiring and
  those four replacements plus their comments. `api_coverage` (639 of 913) and `type_surface`
  (`HtmlSurface` 6, `Browser` 33) both compare green against the *pre-split* baselines.
  Two measured facts about visibility came out of the error list, and both corrected what I had
  assumed. A parent module is *not* allowed to initialise a child's private fields: `mod.rs` builds a
  `Browser` in three places (the handle-0 placeholder in `create_browser`, `browser_from_handle`, and
  the callback that fills it in), and all three went E0451 once the struct moved down a level. And the
  first `cargo check` did not tell me that: it reported 12 `E0624 method as_raw is private` errors and
  **no** field errors, because a struct literal that already failed on another count was not audited
  for field privacy as well. The widening list was therefore incomplete until the first fix landed and
  the same tree was checked again -- so "the errors say what must widen" is only true one fix at a
  time, and the set is not known until a check comes back without errors.
- `src/server.rs` (821 lines) became `src/server/`: `config.rs` 172 (the seventeen announcements a
  server makes about itself -- product, description, map, name, player caps, tags, key/value pairs,
  heartbeat and advertise flags), `lifecycle.rs` 170 (the raw `SteamGameServer_InitEx` behind
  `Server::init`, the two callback pumps, `register_callback` and the server's own `steam_id`),
  `callbacks.rs` 128 (the four game-server notifications, with the `DenyReason` vocabulary and its
  `From` mapping), `interfaces.rs` 130 (the eight sub-interface accessors), `auth.rs` 107 (session
  tickets, cancelling one, opening and closing an authentication session), `mod.rs` 105 (the `Server`
  struct with its `Send`/`Sync` claims, `ServerMode`, `QUERY_PORT_SHARED` and the manual test), and
  `packets.rs` 59 (the GSHS relay traffic in and out). The crate's largest file is now `src/lib.rs`
  at 814.
  This one needed no widening at all: `Server`'s fields are private to the root, and a child may read its parent's private fields, and the two
  comments and the payloads that read them stayed reachable through the parent's `use super::*`, so
  the witness is the cleanest yet -- 743 non-blank lines before, **0** lost, 40 added, every added line
  a module header (17), a `use super::*;` (6), one of the four extra `impl Server {` openers and their
  braces (8), or the wiring (7). `api_coverage` (639 of 913) and `type_surface` (`Server` 38) compare
  green against the pre-split baselines; `module_sizes` and `package_inventory` (102 shipped files in
  `steamworks`) were re-recorded.
  One thing the probe found by refusing to move it: the ninth accessor, `Server::apps()`, is not dead
  code and never was -- lines 633-645 of the old file are a `/* TODO: Buggy currently?` block comment
  that swallows the whole method, so the anchor I had planned for `pub fn apps` at line 635 pointed at
  text inside a comment. The splitter's boundary rule caught it (the line above that block is neither
  blank nor a closer) and the comment now travels with `interfaces.rs` as what it is. Nothing in
  `README.md`, `CHANGELOG.md`, `docs/` or `examples/` claims a `Server::apps` exists -- checked by
  grep, 0 hits -- so no document needed correcting, and deleting the commented block was not done: it
  would be a diagnosis of someone else's "Buggy currently?", not a move.
  This unit also walked into the trap the previous two had named. The first chain replay judged
  `module_sizes` red on a single entry: I re-recorded that ledger and *then* rewrote `interfaces.rs`'s
  module header two lines longer (128 -> 130), so "the chain is green" had been describing the tree
  before my own last edit. The size quoted above is the post-edit one, the ledger was re-recorded from
  the tree that ships, and the replay reported for this unit is the run after the last content change.
  A second self-inflicted trap was worth the detour: the new `interfaces.rs` header first quoted the
  literal `/* TODO` text, which made a block-comment-stripping regex eat everything from that doc line
  to the real `*/` and report **0** live null-checks in a file holding 8. The quote is prose now, and
  the design doc's two accessor tallies were re-measured with the corrected ruler: `debug_assert`
  null-checks read lib.rs 24 sites / 23 methods and `server/interfaces.rs` 9 textually but 8 live, so
  the doc's "33 处" is still true as a text count, its live count is 32, and its "17 + 6 = 23 getters"
  is today 23 + 8 = 31.
- `src/remote_storage.rs` (792 lines) became `src/remote_storage/`: `mod.rs` 247 (the `RemoteStorage`
  struct, its `Clone`, the two argument-validation helpers and the 126-line `#[cfg(test)]
  mod offline_tests`), `files.rs` 247 (`SteamFile`, its writer and reader with `Seek` and `Drop`,
  `FileWriteBatch` and `SteamFileInfo`), `api.rs` 182 (`RemoteStorage`'s ten methods -- quota,
  enumeration, the one-shot `read_file`/`write_file` and the legacy surface it forwards),
  `visibility.rs` 68, `changes.rs` 58 and `platforms.rs` 27 (the `bitflags!` block and its conversion
  onto Steam's raw enum). The crate's largest file is now `src/lib.rs` at 814.
  27 items, 0 unclaimed lines, witness 723 non-blank before / **2** lost (both widenings:
  `SteamFile::name` and `FileWriteBatch::rs`, each constructed by `api.rs`) / 32 added, of which 2 are
  those replacements and 30 are headers, wiring and braces. `api_coverage` (639 of 913, with
  `ISteamRemoteStorage` still at 26 of 59) and `type_surface` (`RemoteStorage` 10 methods) compare
  green against the pre-split baselines.
  Two lessons, both from things that looked like passes. First: "0 holes" proves *coverage*, not topic
  correctness. `cloud_file_name` and `cloud_chunk_len` are file-level functions rather than methods, and
  I had no anchor for them, so the probe handed their lines to the preceding block -- `api.rs` -- and
  reported a clean 0 holes. The root's own test module calls both by name, so it was `cargo check`'s six
  `E0425 cannot find function` errors, not the splitter, that found the mis-assignment. Second: the root
  deliberately keeps the 126-line test module inline instead of giving it a file of its own, because
  `api_coverage.py` strips test code by the shape "`mod name {` at column 0 under `#[cfg(test)]`" and a
  whole-file test module would leave that marker behind for the ruler to miss. On today's content the
  exposure is measured and hypothetical -- that module holds 0 direct `SteamAPI_*` call sites, so
  coverage could not inflate from it -- but the shape is what the gate reads, and `test_only_calls` is
  the tripwire that says so.
- `src/networking_sockets/mod.rs` (791 lines, the largest module root still carrying its own
  implementation) became wiring plus six children beside the `connection.rs` and `fake_udp.rs` that
  were already there: `mod.rs` 259 at that commit -- 260 once the FakeIP wiring line was added, the `NetworkingSockets` struct and its `Send`/`Sync`
  claims, `send_messages`, `InvalidHandle` and the inline test module), `listen.rs` 120 (the three ways
  to open a listening socket), `listen_socket.rs` 122 (the socket handle, its three event reads and its
  accepting send), `poll_group.rs` 102 (`InnerSocket`, `NetPollGroup` with its `Drop`, the batched
  receive, and the method that mints a group), `status.rs` 98 (connection info, realtime status, lane
  configuration), `connect.rs` 92 (IP and P2P connects), `auth.rs` 49 (`init_authentication` and
  `get_authentication_status`). `NetworkingSockets`' 389-line `impl` (38..426 inclusive) is now seven blocks by concern; the
  crate's largest file is `src/lib.rs` at 814.
  28 items, 0 unclaimed lines, witness 730 non-blank before / **1** lost (`NetPollGroup::handle`, now
  `pub(super)` because `connection.rs` attaches a connection to a group through it) / 39 added, 38 of
  them module headers, wiring and impl braces. `api_coverage` (639 of 913) and `type_surface`
  (`NetworkingSockets` 13, `ListenSocket` 4, `NetPollGroup` 1 measured, each at its registered value)
  compare green against the pre-split
  baselines; `module_sizes` (98 entries, 26,953 lines registered) and `package_inventory` (113 shipped
  files) were re-recorded.
  The one number I got wrong here was the impl's closing line: I wrote 435 from the earlier method
  listing instead of reading it, and the truth is 426 -- so the block that should have ended at the
  impl's `}` swallowed it, took `ListenSocket`'s eight doc lines (428-435) with it, and left the root
  with an
  unmatched `}` at line 261. The probe still said "0 holes", because everything had been claimed -- by
  the wrong block. Two fixes followed the incident: the generator now asserts the opener line literally
  reads `impl NetworkingSockets {` and the closer is exactly `}`, so a recalled boundary cannot be used
  again, and the FakeUDPPort unit's citation of the old module root's 588-604 range -- a fourth
  `set_len` pointer, naming a file that this same change shrank to 260 lines -- was re-anchored by
  content to line 67 of `src/networking_sockets/poll_group.rs`, the line the unclamped
  `as usize; set_len(count)` actually sat on that day. The coordinate is written out in prose rather
  than as a live pointer on purpose: re-measured 2026-10-18, that file's line 67 is blank, the clamp
  that replaced the hazard is declared at `src/networking_sockets/poll_group.rs:64` and called at
  `src/networking_sockets/poll_group.rs:85`. A dated coordinate is not a claim about today's tree, and
  `doc_pointers.py` only exempts pointers whose *file* is gone -- so the honest fix is to stop
  presenting a past line number as a present pointer, not to widen the gate.
- `src/friends/mod.rs` (790 lines, the largest module root still holding its own implementation) became
  wiring plus eight children beside the `callbacks.rs`, `chat.rs` and `friend.rs` already there:
  `clans.rs` 174 (count, roster, name, tag, owner, officers, the two public/official flags, chat member
  counts and the activity feed), `clan_chat.rs` 157 (join and leave, the roster, sending and reading,
  and the three window-state calls), `followers.rs` 91 (the three asynchronous asks), `overlay.rs` 88
  (the overlay dialogs and invites, with the `OverlayToStoreFlag` this crate models), `list.rs` 85 (the
  rosters, one lookup by id, the fill-in request, persona state, relationship and Steam level),
  `rich_presence.rs` 77, `messages.rs` 60 and `flags.rs` 50 (the two `bitflags!` vocabularies), with
  `mod.rs` down to 70 lines of header, wiring and the four calls that have no topic of their own.
  `Friends`' single 719-line `impl` held 48 `pub fn` (counted: `sed -n '72,790p' | grep -c '^    pub fn '`),
  which is exactly the `type_surface` entry for `Friends` -- the 14 methods in `friend.rs` belong to the
  `Friend` handle, a different type. Those 48 are now spread over eight `impl Friends` blocks grouped by
  what they ask Steam for.
  52 items, 0 unclaimed lines, and the cleanest witness of the series: 736 non-blank lines before, **0**
  lost, 51 added, and none of those 51 is anything but a module header, a `use super::*;`, a `mod` or
  `pub use` line, or an `impl` brace. Nothing had to widen, because the two things the children needed
  from the root were already reachable: `Friends`' fields are `pub(crate)`, and the root's own
  `use chat::{chat_buffer_len, chat_entry_type_from, chat_text};` hands those helpers down through each
  child's `use super::*`. `api_coverage` (`ISteamFriends` 63, total 639 of 913) and `type_surface`
  (`Friends` 48) compare green against the pre-split baselines; `module_sizes` (106 entries, 27,015
  lines) and `package_inventory` (121 shipped files, 12 of them under `src/friends/`) were re-recorded.
  After seven splits the ranking has changed shape: the only file still above 800 lines is `src/lib.rs`
  (814) -- the crate root, where the module declarations live -- and the next two are `src/input.rs` 725
  and `src/utils.rs` 710, single-file modules rather than module roots, so root-splitting stops here
  unless one of them grows again.
- `ISteamInput` device callbacks: `Input::enable_device_callbacks()` plus the `DeviceConnected`
  and `DeviceDisconnected` payloads, delivered through `CallbackResult` so they reach the callback
  stream as well as `client.register_callback`. `ISteamInput` goes 43 -> 44 and the total to 621
  of 913. `isteaminput.h:852-854` is quoted in the docs because it promises more than the name:
  enabling makes "each controller that is already connected ... generate a device connected
  callback", so the caller receives the current roster as events, not only the plug-ins after it.
  Registering them in `CallbackResult` rather than in `CALLBACKS_NOT_IN_STREAM` was forced by the
  crate's own test `every_callback_is_streamable_or_listed_as_debt`, which refuses a longer debt
  list -- and the way it got my attention is worth recording: I had run
  `cargo test --lib device_callback`, seen three green tests, and treated that as the suite
  passing. It was not. The filter had excluded the debt test from ever running, and the full
  lib-suite floor gate is what turned red. A filtered green says only that the filtered tests
  passed.
  Three tests, shaped by what can actually be observed: the two payloads are one `u64` each, so
  swapping which Steam struct a payload decodes moves the same bytes and reads back the same
  number -- value assertions cannot see that crossing. The callback id can, so
  `the_two_device_callbacks_keep_their_own_steam_callback_ids` pins both ids against the
  `steamworks_sys` constants and asserts they differ. Controls: decode the connected payload from
  the disconnected struct -> the id test goes red and nothing else does, which is the point of
  having it; drop the handle instead of copying it -> the value test goes red. Both files restored
  byte-for-byte.
- The callback-parser floors in `src/callback.rs` were tightened to the measured truth instead of
  left wider than reality: 80 definitions, 56 streamed, still exactly the 24-entry debt list.
  A plain `grep -c impl_callback!` says 82, and the extra two are the macro's own documentation,
  which the matcher skips on purpose -- so the registered numbers are the ruler's, not the
  shallowest count available.

Ledgers for this commit: `Input` 44 -> 45 inherent methods (key count stays 80), `src/input.rs`
636 -> 725, `src/callback.rs` 539 -> 543, lib test floor 130 -> 133.

- **The fourth FakeIP entry point, and the public shape it had been waiting on**
  (`src/networking_sockets/connection.rs`, `src/networking_types/addresses.rs`,
  `src/networking_utils.rs`, `src/networking_sockets/fake_ip.rs`).
  `GetRemoteFakeIPForConnection` (`isteamnetworkingsockets.h:895`) is now
  `NetConnection::remote_fake_ip() -> SteamResult<SocketAddr>`, and
  `ISteamNetworkingUtils::GetIPv4FakeIPType` (`isteamnetworkingutils.h:254`) is now
  `NetworkingUtils::ipv4_fake_ip_type(Ipv4Addr) -> FakeIpType`. What had kept the first one out was not the
  generated layer but a public-type question -- every address-shaped answer in this crate is a
  `std::net::SocketAddr` while `SteamIpAddr` stays crate-internal -- and the answer chosen is to keep that
  shape and wrap Steam's own classifier at the same time. `FakeIpType` is `#[non_exhaustive]`, with
  `Invalid`, `NotFake`, `GlobalIpv4`, `LocalIpv4` and `Unrecognized(i32)`. Unlike the nine fallback arms
  registered in the design doc -- reachable only if Steam ships a value its vendored header never declared
  -- this one has a declared-but-unnamed input today: `k_ESteamNetworkingFakeIPType__Force32Bit`
  (`steamnetworkingtypes.h:190`) is the width sentinel, and
  `the_width_sentinel_lands_on_the_fallback_and_keeps_its_number` builds exactly that, so the arm has a test
  instead of a register entry. The method sits on `NetConnection` rather than beside the other three FakeIP
  calls because `sockets` and `handle` are private to `connection.rs`: a sibling module cannot read them,
  and that is a visibility fact rather than a style preference.
  The address decode is a pure function -- `socket_addr_from_octets` -- because that is the rule this
  repository follows for any decision taken from a Steam value: if CI cannot judge it, it does not belong in
  the FFI edge. It classifies by the mapped prefix (ten zero bytes then `ff ff`, the layout
  "RFC4038, section 4.2" names, `steamnetworkingtypes.h:226-231`), reads the four octets in writing order
  ("As bytes, i.e. network byte order", `:231`), hands the port over as the number it already is
  ("Host byte order", `:239`), and keeps a cleared struct as the IPv6 any-address rather than as
  `0.0.0.0` (`:200`) -- the last of those is the one a shorter prefix test silently gets wrong, and it is why
  the all-zero case has its own test. Eleven new tests, and fourteen mutations against them: every one of
  the fourteen went red on an assertion message, not on a crash or a compile error. The replay also corrected
  two of my own attributions -- a byte-swap at the *boundary* read (`to_socket_addr`) is killed by the
  struct-level round trip, not by the pure port test, so a second mutation at the pure function was added to
  lock that path separately; and the classifier's argument went from `ip.to_bits()` written inline at the FFI
  edge to `fake_ip_probe_argument`, because an assertion that `to_bits()` equals itself tests `std`, not this
  crate. Two refusals in `src/error.rs` were folded into `k_EResultFail` one at a time to check that
  `remote_fake_ip`'s documented distinction between a bad handle and a non-FakeIP connection is real; both
  went red. Everything was restored byte-for-byte, checked against a copy and not by re-running the edit
  backwards.
  What CI still cannot judge is stated rather than counted: `remote_fake_ip` and `ipv4_fake_ip_type` each
  need a live interface, and the first needs a connection actually made through the FakeIP system, so the
  wiring is registered as unverified and only the decode, the table and the argument are locked.
  Ledgers for this commit: `NetConnection` 16 -> 17 and `NetworkingUtils` 16 -> 17 inherent methods with
  `FakeIpType 1` registered new; `src/networking_types/addresses.rs` 246 -> 415, `src/networking_utils.rs`
  510 -> 693, `src/networking_sockets/connection.rs` 677 -> 712, `src/networking_sockets/fake_ip.rs`
  506 -> 524; `ISteamNetworkingSockets.called` 30 -> 31 and `ISteamNetworkingUtils.called` 14 -> 15, so the
  coverage total moves 644 -> 646 of 913; header citations 35 files / 194 spans -> 36 / 222 with all three
  debt columns still at 0 (the citation ruler judged three of my new citations wrong on the first try -- two
  `[::]:0` socket literals it read as a citation of line 0, and one paraphrase of mine wearing quotation
  marks -- and both were reworded rather than exempted). The lib test floor was registered at 165 while the
  suite had actually reached 242, so it is re-recorded at the measured 253 instead of being left 77 tests
  looser than reality.

- **The FakeIP family's last lookup** (`src/networking_utils.rs`, with notes in
  `src/networking_sockets/fake_ip.rs` and `src/networking_sockets/fake_udp.rs`).
  `ISteamNetworkingUtils::GetRealIdentityForFakeIP` (`isteamnetworkingutils.h:268`) is now
  `NetworkingUtils::real_identity_for_fake_ip(SocketAddr) -> Result<NetworkingIdentity, SteamError>`,
  which is also the answer to the sentence `FakeUDPPort::receive_messages` has quoted since it was
  written -- "SteamNetworkingMessage_t::m_identity in the returned message(s) will always contain a
  FakeIP.  See ISteamNetworkingUtils::GetRealIdentityForFakeIP." (`steamnetworkingfakeip.h:61-66`).
  The out parameter is read only after the status answers, because a struct Steam never wrote holds
  zeros and zero *is* `k_ESteamNetworkingIdentityType_Invalid`: the premise is pinned by
  `an_untouched_identity_struct_holds_the_type_that_is_nobody`, which was checked by editing the
  generated binding's discriminant to 7 and watching that test go red (`left: 7 / right: 0`) -- a
  premise nobody can re-check is how the last wrong sentence in this file got published. The two
  documented refusals stay tellable apart (`:259-260`: "the IP is not a FakeIP." versus "we don't
  recognize that FakeIP and don't know the corresponding identity.") and are asserted to differ, not
  just to exist. Three mutations, three red on an assertion message; the error-table ones also turn
  red in the two FakeIP refusal tests written yesterday, which is the redundancy this ledger likes.
  Unverified rather than counted: whether a given address *has* an answer needs a running client and
  an address Steam actually assigned. This is the last FakeIP entry point in either header that this
  crate wraps -- `SetGlobalCallback_FakeIPResult` stays deliberately unwrapped, for the reason in
  `fake_ip.rs`'s module note.
  Ledgers: `NetworkingUtils` 17 -> 18 inherent methods, `ISteamNetworkingUtils.called` 15 -> 16 with
  the coverage total 646 -> 647, header citations 222 -> 227 spans (the ruler judged two of the new
  citations wrong on the first try: both quoted line 258, "On failure, returns:", while the sentences
  they quote sit at 259 and 260 -- re-anchored, not exempted), lib test floor re-recorded at the
  measured 255.

- **The user's own list of servers: `ISteamMatchmaking`'s favorites face**
  (`src/matchmaking/favorites.rs`, new). `GetFavoriteGameCount`, `GetFavoriteGame`, `AddFavoriteGame`
  and `RemoveFavoriteGame` (`isteammatchmaking.h:60-77`) become `favorite_game_count`,
  `favorite_game(index) -> Option<FavoriteGame>`, `add_favorite_game(&FavoriteGame) -> Option<usize>`
  and `remove_favorite_game(&FavoriteGame) -> bool`, with the two list bits exposed as a
  `bitflags`-style `FavoriteFlags` built from Steam's own constants (`:525-527`). Four decisions are
  this crate's, and the docs say so instead of dressing them up as Steam's:
  * `AddFavoriteGame`'s header sentence describes what the call does and never says what a refusal
    looks like, so a negative answer is read as `None` because a slot index cannot be negative --
    anyone wanting Steam's exact int can still take the `sys::` binding.
  * the `uint32` address word has no documented byte order either, so the crate keeps the reading it
    already uses in both directions (`u32::from(addr)` out, `Ipv4Addr::from(bits)` back), and the
    argument for it stays written once, on `User::advertise_game`.
  * an unknown flag bit is retained rather than truncated, so `bits()` can always show the number
    Steam wrote instead of quietly turning it into 0.
  * `last_played_on_server` stays a raw `u32`: the header says the field "is filled in the with the
    Unix time the favorite was added" (`:70`) and names no sentinel, so `0` is not "1970" and not
    "unset".
  Every clamp is a named function and every one has a lock: `favorite_count` (negative answer to 0,
  never to `usize::MAX`), `favorite_index` (an index that cannot be a `c_int` never reaches Steam --
  casting `usize::MAX` would hand over -1), `favorite_entry` (`false` means no entry and none of the
  six out parameters is read), `stored_favorite_slot`, and the address pair
  `favorite_address_word`/`favorite_address_from_word`, which exists as two functions precisely
  because the previous unit's mutation replay showed that asserting `to_bits()` against itself tests
  `std`. Seven tests, eight mutations, eight red on an assertion message. The four methods need a
  running client, so the wiring is registered as unverified.
  Ledgers: `ISteamMatchmaking.called` 28 -> 32 and the coverage total 647 -> 651 of 913, `Matchmaking`
  28 -> 32 inherent methods, `src/matchmaking/favorites.rs` 310 lines with `mod.rs` 490 -> 498, header
  citations 227 -> 239 spans across a 38th citing file, lib test floor re-recorded at the measured 262.

- **The lobby control face, which also closed a table with a missing number**
  (`src/matchmaking/mod.rs`, `src/matchmaking/queries.rs`). `ISteamMatchmaking` reaches **38/38**:
  `SetLobbyType`, `SetLobbyMemberLimit`, `SetLobbyOwner`, `SetLinkedLobby` and `InviteUserToLobby`
  (`isteammatchmaking.h:226-251`) plus `AddRequestLobbyListCompatibleMembersFilter` (`:123`) are now
  `set_lobby_type`, `set_lobby_member_limit`, `set_lobby_owner`, `set_linked_lobby`,
  `invite_user_to_lobby` and `set_request_lobby_list_compatible_members_filter`. Getting there exposed
  the reason one of them had never been wrapped: `LobbyType` had four variants while the header
  declares five -- `k_ELobbyTypePrivateUnique` (`:26`) had no name anywhere in this crate, so the call
  that changes a lobby's type could not express every type. See **Changed** for that enum.
  The mapping that `create_lobby` used to spell inline is now `lobby_type_to_raw`, tested arm by arm
  against the header's own numbering, and `lobby_member_limit` stopped casting: it used to read
  `match count { 0 => None, _ => Some(count as usize) }`, so a negative answer -- a value the header
  declares no meaning for -- became a lobby of 18446744073709551615 seats. It now goes through
  `member_limit`, which refuses both zero and anything negative. Three tests, six mutations, six red
  on an assertion message (three of them against the different ways that cast could come back).
  Registered rather than changed at the time, because each needs a decision of its own:
  `set_request_lobby_list_result_count_filter(count: u64)` hands Steam `count as i32`, so
  4294967296 arrives as 0 -- the mirror image of the limit this unit adopted for
  `set_lobby_member_limit` (take the signed type Steam takes, refuse nothing silently); and
  `create_lobby`'s `assert!(max_members <= 250)` aborts on a caller argument for a ceiling the vendored
  header never states (measured: no `250` anywhere in `isteammatchmaking.h`), so the bound is this
  crate's own claim and its failure mode is an abort in the caller's frame.
  **Both were decided and landed on 2026-10-20** -- see the two `### Changed` entries for them; nothing
  in this paragraph is retracted, it records what was measured then.
  Docs for the five setters cite only their own header lines, and `invite_user_to_lobby` says plainly
  that Steam's promise -- the target "will receive a LobbyInvite_t callback" (`:152`) -- is not
  delivered by this crate today, rather than implying it is.

- **The game server's own state, and an address that carries two byte orders**
  (`src/server/status.rs`, new). `LogOff`, `BLoggedOn`, `BSecure`, `WasRestartRequested`,
  `GetPublicIP`, `UserHasLicenseForApp`, `SetRegion`, `SetSpectatorPort` and
  `SetSpectatorServerName` (`isteamgameserver.h:65-75`, `:105-116`, `:136-137`, `:169-171`,
  `:184-187`) become `log_off`, `is_logged_on`, `is_secure`, `restart_requested`,
  `public_address() -> ServerAddress`, `user_has_license(SteamId, AppId) -> UserHasLicense`,
  `set_region`, `set_spectator_port` and `set_spectator_server_name`. `ISteamGameServer` goes 23 -> 32.
  `ServerAddress` exists because Steam's own struct documents the two families differently --
  `m_unIPv4` as "Host order" and `m_rgubIPv6` as "Network order! Same as inaddr_in6"
  (`steamtypes.h:115-116`) -- so the decode (`address_from_steam_parts`) reads the tag and applies the
  order that field states; swapping the IPv4 word and letting a v6 tag fall into the v4 arm are both
  killed by tests. The same struct has no tag for an unknown family, and `k_ESteamIPTypeIPv4` *is*
  zero, so a struct nobody wrote reads as IPv4 `0.0.0.0` -- the docs say that rather than letting a
  caller read it as "no address".
  `UserLicense::Unrecognized` is registered as what it is: a future-only arm. `ESteamIPType` and
  `EUserHasLicenseForAppResult` declare every value their tables name, so no legal input reaches that
  arm, and this repository's rule is that an unreachable arm gets no test -- unlike the FakeIP
  classifier earlier today, whose fallback is reachable through a declared sentinel and therefore has
  one. That claim was checked rather than assumed: a mutation rewriting the fallback to return an
  address **survived**, which is the reading this entry records.
  Left unwrapped, each with the header's own words as the reason: `GetGameplayStats` and
  `GetServerReputation` sit under "these two functions s are deprecated, and will not return results"
  and "they will be removed in a future version of the SDK" (`isteamgameserver.h:178-179`); the two
  `SendUser…_DEPRECATED` calls carry it in their names (`:236`, `:250`) under a section headed
  "Deprecated functions." (`:259`). And `RequestUserGroupStatus` (`:173`, "results returns async by
  GSUserGroupStatus_t"), `AssociateWithClan` and `ComputeNewPlayerCompatibility` (both returning
  `SteamAPICall_t`) wait on the callback unit rather than being half-wrapped here.
  Four tests, five mutations: four killed by assertion message -- including the premise pin, reddened
  by renumbering `k_EUserHasLicenseResultNoAuth` to 6 in the generated layer -- and the one above
  surviving by design. Restores verified byte-for-byte against a copy.
  Ledgers: `Server` 38 -> 47 inherent methods, coverage 657 -> 666 of 913, header citations 256 -> 275
  spans across a 40th citing file (the ruler judged two of my citations off by one line and flagged two
  of my own sentences wearing quotation marks before it was clean), lib test floor at the measured 269.

- **The two clan-association calls that do answer, and the one whose answer type does not exist**
  (`src/server/status.rs`). `AssociateWithClan` and `ComputeNewPlayerCompatibility`
  (`isteamgameserver.h:215`, `:219`) become `associate_with_clan(SteamId, FnOnce(Result<(),
  SteamError>))` and `compute_new_player_compatibility(SteamId, FnOnce(ClanCompatibility))`, both
  through the crate's existing call-result registration; `ISteamGameServer` goes 32 -> 34. The reply
  structs carry what the header says they carry and nothing more -- `AssociateWithClanResult_t` is one
  field whose comment reads "Result of the call" (`:377`) -- and `ClanCompatibility`'s three counters
  stay `i32`, Steam's declared type, because the header describes no range for them and widening an
  undescribed value into `usize` would be the wrapper inventing a domain. The two decisions that are
  real ones are functions: `clan_association_answer` puts the transport failure ahead of the payload
  (a call that never answered still carries `k_EResultOK` in its struct, and reading the payload first
  would turn "no answer" into "accepted"), and `clan_compatibility` keeps each counter in its own slot.
  Four mutations, four red on an assertion message: ignoring `io_error`, crossing two counters,
  dropping the candidate, and reading only the low half of the 64-bit union. The last one is worth
  recording because it survived its first run: the fixture candidate was `42`, which fits in 32 bits,
  so a narrowing read could not be seen -- the candidate became a real Steam ID (`76561197960287930`)
  and only then did that mutation go red. A fixture that is too small is not a test.
  `RequestUserGroupStatus` is **not** wrapped, and the reason is measured rather than judged from the
  name: its own comment promises the result "returns async by GSUserGroupStatus_t"
  (`isteamgameserver.h:173`), but that struct is declared nowhere in the vendored SDK -- measured: one
  mention, the comment itself, and zero definitions -- so the generated layer has no type to hand back
  and the call would be a question with no way to read an answer. Wrapping the `bool` alone would look
  like coverage and deliver a stub.

- **The out-parameter read face of `ISteamUserStats`: six getters, and the two tables behind them**
  (`src/user_stats/user_reads.rs`, new). `GetUserStatInt32` (`isteamuserstats.h:165-166`),
  `GetUserStatFloat` (`:168-169`), `GetUserAchievement` (`:171`), `GetUserAchievementAndUnlockTime`
  (`:173`), `GetAchievementProgressLimitsInt32` (`:300-301`) and `GetAchievementProgressLimitsFloat`
  (`:303-304`) become `user_stat_i32`, `user_stat_f32`, `user_achievement`,
  `user_achievement_and_unlock_time`, `achievement_progress_limits_i32` and
  `achievement_progress_limits_f32`. `ISteamUserStats` goes 31 -> 37 of 44, and the repository total
  668 -> 674 of 913. All six share one shape -- Steam returns a `bool` and writes an out-parameter --
  so the two decisions in that shape became functions instead of six copies of an `if`:
  `read_answer` refuses to return a value on `false` (the headers describe no value for the
  out-parameter of a declined call, so neither does this crate), and `unlock_state` names all four
  cells of (achieved, time). `AchievementUnlock` exists because Steam's own note changes what a
  *number* means: "If the return value is true, but the unlock time is zero, that means it was
  unlocked before Steam" began tracking (`:119`), so a `0` there is not 1970-01-01, and the fourth
  cell -- a time with an unachieved flag -- is the combination the note does not describe and is
  forwarded as a number rather than folded into `NotAchieved`. The seconds stay Steam's `uint32`:
  converting them into a `SystemTime` would be this wrapper choosing a calendar reading the header
  does not ask for, and a caller can add that reading back in one line.
  The declined answer is `SteamError::InvalidParameter`, which is what the sibling interface
  (`src/server_stats.rs`) already answers for the identical shape, chosen to keep two interfaces that
  decline for the same reasons from reporting different errors.
  Progress bounds are forwarded as `(min, max)` with no `min <= max` check, because the header states
  no such constraint (`:297-299`) and enforcing one would report this wrapper's opinion about Steam's
  data as an error.
  Four tests, four mutations, all four red on an assertion message: trusting the value when Steam
  declined, losing the undocumented cell, reading the zero time as a date, and crossing two of the
  four cells. Registered as unverified: the six FFI bodies -- every one needs stats that were actually
  requested for a user, so no input here can reach them. Ledgers: header citations 287 -> 304 spans
  across a 41st citing file, `UserStats` 24 -> 30 inherent methods, lib test floor raised to the
  measured 275, and the citation ruler judged five of my new citations red before it was clean -- two
  quoted phrases sitting outside the spans they cited (`isteamuserstats.h:297`/`:298`, which I had
  written one line low) and three citation blocks carrying line numbers with no checkable phrase. All
  five were re-anchored, none exempted.

- **The global achievement percentages, which this crate could request but never read**
  (`src/user_stats/achievement_percents.rs`, new, plus two writes in `src/user_stats/stats_io.rs`).
  `GetMostAchievedAchievementInfo` (`isteamuserstats.h:262`) and
  `GetNextMostAchievedAchievementInfo` (`:267`) become `most_achieved_achievement_info(name_capacity)`
  and `next_most_achieved_achievement_info(previous_iterator, name_capacity)`, returning
  `Option<GlobalAchievementInfo>`; `IndicateAchievementProgress` (`:147`) and `UpdateAvgRateStat`
  (`:111`) become `indicate_achievement_progress` and `update_avg_rate_stat`. `ISteamUserStats` goes
  37 -> 41 of 44, the repository total 674 -> 678 of 913, and only the three `SteamAPICall_t` forms
  are left unwrapped on that interface. The ask already existed
  (`UserStats::request_global_achievement_percentages`) with no reader, which is why `-1` matters
  twice: Steam's header uses the same value for "no data on achievement percentages" (`:260`) and for
  the end of the iteration (`:265-266`), so `next_iterator: None` says the walk stopped and cannot say
  which ending it was -- registered rather than guessed. Three decisions are functions, not inline
  branches: `checked_capacity` refuses a zero-length buffer (it could never hold a terminator),
  `name_from_buffer` returns a name only if it is whole and valid UTF-8 (a cut-off token is not an
  achievement's name, and lossy conversion would invent one the caller cannot compare against), and
  `iteration` treats only the negative side as the stop signal so **zero stays a legal iterator** --
  the off-by-one this defends against is `<= 0`, which would end the walk before the first row.
  The buffer capacity is a caller argument because the header states no maximum achievement name;
  that is this wrapper's choice, and it is documented as such at both methods.
  `read_answer` became `pub(crate)` so the two writes use the same rule as the six reads: one place
  decides what a `false` from Steam means, across all eight paths.
  Six tests. Seven mutation replays, and the count is stated that way on purpose: the first attempt at
  the lossy-conversion mutant **did not compile**, which proves nothing, so it was rewritten as a
  compiling mutant and then went red on an assertion message like the other six. Restores verified
  byte-for-byte. Registered as unverified: all eight FFI bodies -- a row of the global table needs a
  live session that requested and received it, so no input here can reach them.
  Ledgers: 314 header-citation spans across a 42nd and 43rd citing file, `UserStats` 30 -> 34 inherent
  methods, lib test floor raised to the measured 281, package inventory +1 shipped source. This time
  the ruler judged nothing red, which is the result of reading each header line number out of the file
  with the number printed rather than counting a window by eye.

### Added — CI and release automation

- **`type_surface.py` now sees methods a macro generates, which closes a blind spot it registered
  against itself** (`scripts/ci/type_surface.py`). The ruler counted `pub fn <identifier>` and a
  generated declaration is written `pub fn $name` -- a `$` is not `[A-Za-z_]` -- so `MatchmakingServers`
  sat in the ledger at **4** while the type really had **9**, and the gate that is supposed to catch a
  type accreting responsibility was watching 4 of the 9. It is fixed by counting **call sites**: every
  `macro_rules!` whose body writes a `pub fn $` is collected first, and each of its invocations inside
  an inherent `impl` adds one method to that type. Attribution stays honest because a macro declared and
  never expanded adds nothing, and gaining a generated method means writing a new invocation line -- the
  line a reviewer has to touch.
  Four canary assertions pin it, including the negative case that matters most: a macro invoked inside
  an impl whose body generates no method is *not* surface. And the regression was proven able to go
  red -- restoring the old one-line matcher makes a canary fail with exactly `exit=1 want=4`, the
  value of the blind spot. Ledger re-recorded to `MatchmakingServers 9`; the measured surface is
  83 types / 912 methods, up from 907, which is the honest number rather than growth.

- **`Result<T, ()>` is a counted gate now, and it ships at zero, not at a ceiling**
  (`scripts/ci/erased_errors.py`, `scripts/ci/baselines/erased_errors.txt`, one step in
  `.github/workflows/ci.yml`). Three shapes are counted in code the caller can reach -- a public
  signature returning `Result<_, ()>`, an `Err(())` written anywhere, and the combinator forms
  `.ok_or(())?` / `.map_err(|_| ())?` -- and rows are `<path> <signatures> <erasures>` so a file that
  only erases reasons and a file that also publishes them are distinguishable in the report. It reuses
  the crate's one test-scoping rule (`is_test_material`, `strip_test_modules`, `strip_test_fns`) and
  the one comment-blanking helper, both shared with `cstring_unwraps.py` rather than reimplemented, so
  a changelog sentence about the old shape is not a violation and a test asserting the failure arm is
  not debt.
  The gate ships at zero because the debt it guards was just paid out: a ceiling that allows 26 lets
  the next one through. Nine canary assertions say what it does and does not see (104 in the suite, now
  113): the three counted shapes each emitted as a record, prose about the old shape **not** emitted, a
  `#[cfg(test)]` arm **not** emitted, the ruler red while sites exist, the comparator turning its own
  output red against an empty baseline, the `#scanned <N>` marker present, and -- the one that makes
  all the others meaningful -- a tree with none of the shapes passing, so "0 sites" cannot be a ruler
  that matched nothing. The first run of those assertions caught two bugs of mine: `measure()`
  substituted the matched text before counting it, so it could only ever report zero, and two of the
  "not emitted" expectations were written with inverted logic, which is how I know the assertions
  actually judge something. `scripts/ci/baseline_files.py` registers the ledger's reader and
  `workflow_inputs.py` requires both new paths to be tracked -- which is the exact failure this
  repository has been bitten by once (a ledger present only on the machine that wrote it).
  The local replay is now **29 verdicts** rather than 28 (measured: `steps=29`).

- **`CString::new(x).unwrap()` is a counted debt ledger now, and the classification is the whole gate**
  (new `scripts/ci/cstring_unwraps.py`, `scripts/ci/baselines/cstring_unwraps.txt`, a `quality` step, and
  `canary_cstring_unwraps`). Measured, by two independent implementations that agreed before anything was
  written down (the ruler, and a throwaway paren-matching script): **66 fallible `CString::new`
  conversions in the hand-written sources, 10 of them on literals that contain no nul escape and so
  cannot fail, leaving 56 caller-triggerable panic sites across 23 files** -- worst are
  `src/server/config.rs` (10), `src/matchmaking/mod.rs` (7), then `friends/overlay.rs`,
  `matchmaking/queries.rs`, `timeline.rs` and `user_stats/stats_io.rs` (4 each). The ledger is the 56 and
  may only shrink in both directions, so a new site is red *and* fixing one stays red until the register
  is re-recorded downward. Four classification decisions are what make the number mean something, and
  each has a fixture behind it (73 canary assertions, up from 64): `.expect("…")` counts the same as
  `.unwrap()` (a message does not make a panic cheaper); a string literal counts **only when it carries a
  `\0` escape**, because that is precisely the input `CString::new` rejects and it looks safe at a glance;
  `.unwrap_or_default()` does **not** count -- it substitutes instead of failing, and calling that a
  panic site would inflate the debt with code that cannot crash (this correction landed before the
  ledger was recorded, and the real corpus turned out to contain no such site, so the 56 did not move);
  and comments are blanked line-and-all, not stripped of their `//` marker -- the same failure mode that
  blinds a struct-body ruler, and here the changelog itself is full of `CString::new(..).unwrap()`
  written as prose.
  The gate caught its first real error before it was pushed, and the error was mine: writing the step's
  Chinese comment through a shell heredoc turned the `\0` in "无 `\0` 的字面量" into an actual NUL byte
  inside `.github/workflows/ci.yml`, and `workflow_inputs.py` went red with "only 5 referenced paths
  found" instead of 45 -- a broken YAML does not look like a violation, it looks like an empty scan,
  which is exactly the failure that gate exists to refuse. After the repair the file holds no NUL bytes,
  `yaml.safe_load` parses it, and the `quality` job measures 21 steps (18 named); the local replay is 26
  verdicts and `baseline_files.py` counts 13 ledgers. The local chain then went red on its own wiring,
  which is worth recording next to the gate: both new rulers print their per-site detail on stderr, and
  the replay lines I wrote sent `2>&1` into the measurement file, so `compare_baseline.py` choked on
  `value is not an integer: 'CString::new(key).unwrap()'`. The CI steps were never wrong (they redirect
  stdout only); the local `EXIT=0`-per-last-step habit is what let the mistake sit in the script at all,
  and the `RED_VERDICTS` counter added earlier in the day is what caught it.

- **The bindings-drift classifier is a script now, and it can see the case it used to miss**
  (`scripts/ci/bindings_drift.py`, four new canaries -- `canaries.py` runs 33 assertions where it ran
  29). `Rebuild bindings` has to answer "is this drift a reason to commit, or just the runner's C++
  standard library?", and the inline `grep -cE '^[+-][^+-].*(SteamAPI_|ISteam)'` it used to ask with had
  a one-character hole: the `[^+-]` that keeps `+++`/`---` headers out also *eats the first character of
  the payload*, so a line whose matched token begins right after the diff sign can never match. Proven
  against a fixture, in both directions -- the old pattern counted 2 lines of a diff that materialised
  `SteamNetworkingFakeIPResult_t` and skipped `+pub struct SteamNetworkingFakeIPResult_t {` itself,
  while the two-stage form (select content lines, then match inside them) counts 3 and still counts 0
  for a pure `_LIBCPP_*`/`__MAC_26_*`/ucrt diff. Getting this wrong is not cosmetic: the noise-only case
  is the one the classifier exists to distinguish, and an under-reading classifier reports "environment
  noise" about exactly the drift a human needs to see. The workflow now pipes `git diff -U0` into the
  script and takes its two `$GITHUB_OUTPUT` keys from stdout, and the four fixtures -- noise-only,
  struct materialised, token at column one, empty diff -- are pinned as canaries, so a future edit that
  reintroduces the hole turns the gate red instead of quietly narrowing it.
- **Every SDK-header line citation in `src/` is judged by a gate now**
  (`scripts/ci/header_citations.py`, a new ledger `scripts/ci/baselines/header_citations.txt`, 10 new
  canary assertions -- `canaries.py` runs 43 where it ran 33). The reason it exists is the two units
  immediately before it: `fake_ip.rs` shipped three citations 2-6 lines off and `fake_udp.rs` two
  citations one line off, and the only reason those were caught at all is that those two files bother
  to carry a hand-written citation table. Measured first: **124 citation spans across 17 files and 99
  comment blocks**, of which exactly 2 files were checked by anything.
  The corpus has three citation forms and all three are parsed -- `isteamuser.h:168-172`, the short
  `h:910-911`, and the bare continuation `:866` -- the last two inheriting the header named earlier in
  the same block, which is how prose writes them. That was a measured decision, not a nicety: a
  prototype that matched only the named form counted 119 spans and could not tell "no citation here"
  apart from "a citation written the way people write".
  Judging happens per comment block, over the union of the block's spans, not per citation. Pairing each
  citation with its nearest quoted phrase -- what the prototype did -- reported one phrase against all
  three citations of a block that held three citations and one quote, i.e. a heuristic failure
  pretending to be a documentation failure. Four verdicts are red: a span past the end of the header, a
  span whose lines are all blank, a phrase that is verbatim in a cited header but outside every span
  cited for it, and a phrase that is verbatim in some *other* vendored header the block never cites.
  Everything else -- no phrase, or a phrase no header contains verbatim because it is this crate's own
  summary written inside quotation marks -- is registered per file as `~unjudged` debt that may only
  shrink, because scoring those as verified is the same lie as a green exit from a ruler that read
  nothing. One canary assertion counts that debt literally -- a fixture paraphrase must register as
  exactly one unit, not zero and not two.
  **Scope boundary, registered rather than glossed.** The gate reads comment blocks, so the
  hand-written citation tables -- 26 tuples across `fake_ip.rs` (`CITATIONS` 11 + `FAKEIP_CITATIONS` 7)
  and `fake_udp.rs` (8) -- are *not* among its 124 spans: they are code, and they are pinned by their own
  unit tests instead, which is exactly how both files' bad spans got caught this week. What the gate does
  see in those two files is their prose: 5 + 12 = 17 of the 44 unjudged entries are paraphrases in
  quotation marks, which is why the debt column exists at all.
  Control on the real corpus, not just the fixtures: `steamclientpublic.h:425-435` moved to `427-435`
  in place -- a two-line drift of exactly the kind that shipped twice this week -- and the gate went
  0 → 1 findings naming that citation and that header, then back to 0 with the file restored
  byte-identical. The span was chosen so the quoted phrase still matches nothing in the new range;
  a mutation that only trips the blank or out-of-range rules would have proved the wrong thing.
  Building that control turned up a brittle canary: `canary_baseline_files` set its fixture floor by
  replacing the literal `MIN_BASELINES = 10`, and the day the ledger inventory reached eleven the
  pattern stopped matching, so the fixture silently ran against the repository's real floor and two
  of its assertions exited 2 -- a canary failing because an unrelated constant moved is not testing
  the rule it claims. Both floor edits in that canary are pattern-based now (`^MIN_BASELINES = \d+$`).
  One rule was added after it produced a false red: a fragment several headers share verbatim
  ("not allowed" turns up in four) attributes to none of them, so it is debt, not a finding. The first
  version called it red, and one such fragment was a quarter of everything it reported -- a gate that
  cries wolf on boilerplate teaches everyone to ignore its output.
- **The gate's first run found nine findings over seven sites, and six of them were my prose being
  wrong about Steam's text.** Fixed at the source, each verified against the header rather than against
  memory: `isteaminput.h` 1003 → **1003-1005** (the Playstation-5 example it quotes is on 1005);
  `steamnetworkingfakeip.h:17-18` was **never cited** by the `send_to_fake_ip` doc that quotes "not
  allowed and cannot be sent"; `isteamugc.h:606-607` was missing for the two field comments
  `GetAppDependenciesResult_t` quotes; `isteamuser.h:172` → **168-172** (the "no ticket is available"
  sentence is 168); `steamclientpublic.h:426-435` → **425-435** (the `WARNING: DO NOT RENUMBER` line
  sits above the enum it describes). The seventh site was worse than a drift: `src/matchmaking/chat.rs`
  cited `isteammatchmaking.h:961` and put "Flags describing how a users lobby state has changed" in
  quotation marks as Steam's words -- **that header is 886 lines long**, and the quoted sentence is
  nowhere in the vendored SDK. It was the crate's own doc comment on `ChatMemberStateChange` wearing
  quotation marks and a fabricated number; the entry now cites 531-541 and quotes the line that is
  actually there ("fields specific to a chat member").
  Two more sites were this crate paraphrasing inside quotation marks: `3..5 are "please exit"`, which
  Steam writes as "game should exit - steam will terminate the game", and one generic fragment. The
  paraphrases are now written as prose rather than as quotes, because a quotation mark is a claim that
  the words are somebody else's.
- **A third ledger column makes the gate's own blind spot a number, and the fifth criterion that was
  measured and refused** (`header_citations.py`, `baselines/header_citations.txt`). Of 124 spans, 39 sat
  in comment blocks that carry no quotation marks at all -- not judged, not debt, just unverified, and
  therefore invisible in a way that reads like coverage. They are now `<path>~undeclared`, shrink-only
  like the other two columns, so "go read the header and quote the real sentence" is a direction the gate
  rewards instead of a chore nobody can see.
  The tempting way to convert those 39 automatically was: if a block names a Steam symbol, the symbol
  must appear inside the span it cites. Tried on the corpus, twice -- loose (CamelCase names, substring
  membership: 67 claims, 20 flags) and tight (whole-identifier vocabulary, only `k_E*` / `ISteam*` /
  `SteamAPI_*` / `*_t`: 26 claims, 7 flags). Every one of the seven tight flags was read and **none was
  a defect**: this crate cites a header's *doc comment* rather than its declaration line as a matter of
  course, and the neighbouring prose routinely names a symbol belonging to the next sentence
  (`user_stats/stats.rs` cites `isteamuserstats.h:134` for the icon-handle sentence while
  `UserAchievementIconFetched_t` comes up because the following sentence explains how the crate resolves
  the handle -- the identifier is at 135 and 428). Precision 0 out of 7, so the rule is recorded as a
  negative result in the ruler's own docstring instead of shipping, and the coverage work is done the
  slow way: **fourteen spans in `src/user/voice.rs` and `src/user/mod.rs` now carry the sentences they
  cite** (`"It is suggested to start with a 20kb buffer and reallocate as necessary."`,
  `"GetVoice() should continue to be called until it returns k_eVoiceResultNotRecording"`,
  `"gets whether the users phone number is verified"`, and eleven more), quoted as Steam wrote them --
  including their grammar, which the entries now say out loud rather than repair.
  Ledger after this unit: **125 spans, 0 judged wrong, 21 unjudged phrases, 25 undeclared spans**
  (from 44 and 39; the joiner fix accounts for most of the first drop and the fourteen anchors for the
  second). `module_sizes`: `src/user/voice.rs` 437 -> 445, `src/user/mod.rs` 375 -> 381,
  `src/user_stats/stats.rs` 276 -> 277 (registered total 27838 -> 27853, the +15 being exactly those
  three), and canaries 43 -> **46**.
- **Batch 2 of the anchoring, and a second rule measured and refused.** Nine more spans in
  `src/networking_sockets/fake_ip.rs` and `fake_udp.rs` now quote the sentence they cite
  (`"Port number(s) assigned to us. Only the first entries will contain nonzero values"`,
  `"The port assignments are *not* guaranteed to have any particular order or relationship"`,
  `"If you call this function multiple times and pass the same (nonnegative) fake port index, the same
  object will be returned, and this object is not reference counted"`, and the rest), replacing prose
  that had *paraphrased* Steam -- "warns they are neither ordered nor contiguous", "explains is needed
  because" -- which is exactly the shape no ruler can check. `~undeclared` 25 -> **16**, still 0
  findings, and the span count moved 125 -> 127 because writing out a bare `:123-125` in full makes it
  countable: coverage appearing, not drift.
  The refused rule: some of those blocks carry verbatim header text inside **backticks** rather than
  quotation marks (`fake_udp.rs` quotes the `ReceiveMessages` doc exactly, as code), so the tempting
  generalisation was to accept a multi-word backtick phrase as an anchor too. Measured over the
  unanchored corpus: **1 phrase would have matched a cited span, 22 would not** -- and the 22 are the
  ruler mis-reading its own syntax, because a backtick pair that opens in one code span and closes in
  the next swallows the prose between them (`"says to pass 0 for the first port of the block. call ["`
  quotes nothing). One true case does not pay for a class of fabricated debt, so quotation marks remain
  the only anchor, and what those two files needed is the thing actually done: put the sentence in
  quotes where the doc claims it is Steam's.
  Ledger after batch 2: 127 spans, 0 findings, 21 unjudged, **16 undeclared**; `module_sizes`
  `fake_ip.rs` 493 -> 500 and `fake_udp.rs` 425 -> 427 (registered total 27853 -> 27862).
- **Batch 3: the blind spot is closed, and closing it turned up a citation naming the wrong file.**
  Fourteen edits across `src/input/{effects,session}.rs`,
  `src/networking_sockets/fake_ip.rs`, `src/user/{answers,duration,passthrough,buffers}.rs`,
  `src/networking_messages.rs` and `src/networking_utils.rs` anchored the last **16** spans, so
  **`~undeclared` is 0**: every citing comment block in `src/` is either judged or carries a phrase the
  gate attempted. That is the column's purpose paid off -- while writing the quotes for it,
  `src/networking_utils.rs` turned out to attribute the ping sentinels to
  `isteamnetworkingutils.h:139-146`, which is `EstimatePingTimeFromLocalHost`'s doc comment; the two
  constants are declared in a *different file*, `steamnetworkingtypes.h:1089-1090`
  ("const int k_nSteamNetworkingPing_Failed = -1;" and the -2 below it). Wrong file, wrong lines, and
  undetectable by any rule keyed on quotation marks -- the block simply had none. Same shape as the
  `chat.rs` fabrication two commits back, arrived by a different route, and the only reason either was
  caught is that the invisible set got a number.
  Where Steam's own text is malformed the quote keeps it: `isteaminput.h:829` says "Returns true when
  data was recievied before the timeout expires", and the doc now carries the misspelling with a note
  that it is theirs, because silently correcting a quotation is the exact move that made the last two
  findings possible.
  Ledger: **129 spans, 0 findings, 21 unjudged, 0 undeclared** (the span count rose twice because
  naming a shorthand `:873-875` in full makes it countable), `module_sizes` registered total
  27862 -> 27874. What is left on this ledger is 21 quoted phrases that no header contains verbatim --
  paraphrases wearing quotation marks, which is the next thing to pay down, not coverage to leave.
- **Batch 4: both debt columns are empty, and the last two fixes were about this tool's own
  approximations.** Splitting those 21 by what they actually were: **8 are emphasis** -- one- or
  two-word quotes (`"nil value"`, `"not yet"`, `"UDP port"`) that neither claim a sentence nor locate a
  line -- so `header_citations.py` now treats a quote below `MIN_PHRASE` as neither judged nor debt,
  which is also what makes `~unjudged` mean "a claim that failed" instead of "a quotation mark".
  **4 were Steam's own words the ruler could not see**, because their comments carry inner quotation
  marks (`// deprecated - "you've been playing for 3 hours; take a break"`) and a quote delimited by
  `"..."` cannot hold them: `norm` now drops `"` on both sides, so those four moved from debt to judged
  instead of being deleted from the prose. The remaining 9 were this crate labelling something and
  dressing the label as a quotation; each either got the header's real sentence -- `src/input/effects.rs`
  now quotes "nFlags is a bitmask of values from ESteamInputLEDFlag - 0 will default to setting a color"
  -- or had its marks taken off.
  Two mistakes made *while* fixing this, both worth keeping:
  * writing "colour" where the header writes "color" while turning a paraphrase into a quotation -- the
    exact smoothing this exercise exists to stop, committed by the person doing the stopping. The gate
    reported it as the last debt entry, which is the only reason it was noticed.
  * taking quotation marks off four blocks moved their citations from `~unjudged` into `~undeclared`
    (9 -> 1 while 0 -> 4): honest, but the wrong direction for a two-column debt ledger. Each of those
    four then got a real quote in its place, so the marks came off *and* the citation stayed checked.
  Final state: **130 spans, 0 findings, 0 unjudged, 0 undeclared** -- every SDK line citation in `src/`
  is checked against the header it names, and an empty debt column is now the expected state rather than
  a ceiling. `module_sizes` registered total 27874 -> 27882.
- **The `Rebuild bindings` workflow can no longer commit its own diagnostic into the source tree.**
  `compare` uploads a drift diff as an artifact, and `compare`/`apply` both collect the regenerated
  bindings with `pattern: bindings-*` and `path: steamworks-sys/src/`. The artifact was named
  `bindings-drift`, so it matched its own glob: `apply` downloaded the diff *into the crate's source
  directory*, ran `git add steamworks-sys/src`, and committed `steamworks-sys/src/bindings.diff` --
  667 lines, `git cat-file -s` says 32,970 bytes for the blob (33,637 on disk where this entry was
  drafted, one byte per line more for CRLF; my first draft quoted the checkout size as though it were the
  object size, and the independent re-measurement of this sentence is what caught it). Present in
  `41cf3fb`, listed by `git ls-files`, removed here.
  Two changes, and neither one alone is enough: the artifact is now `drift-diff`, which cannot match the
  collection glob, *and* `apply` stages `steamworks-sys/src/*_bindings.rs` by name, then refuses the
  commit outright if `git diff --cached --name-only` shows any path that is not one of the four. The
  rename only fixes this run's name; the staged-by-name guard is what makes the class of mistake
  unreachable -- a future artifact, or anything else that lands in that directory, now stops the job
  instead of riding into a commit that a human has to notice.
  The guard filters with `awk` rather than `grep -v`, and that is not a style choice: `grep` exits 1 when
  it prints nothing, so under `set -euo pipefail` the *passing* path -- nothing extra staged -- aborts,
  and the only way to stop that is a fail-open fallback. `shell_hatches.py` counts exactly that fallback,
  and it counts it **inside comments too**: the first draft merely mentioned the token in prose and the
  ledger went 9 -> 10 for a comment. That is the ruler telling you the literal belongs nowhere in a
  workflow file, so the reworded version keeps the ledger where it was. Every `run:` body in the file was
  then syntax-checked with `bash -n` -- 8 bodies, 0 failures.
- **The FakeUDP datagram port's header citations are a ruler too, and measuring them found two off by
  one** (`every_fake_udp_header_citation_lands_on_the_sentence_it_quotes` and
  `a_fake_udp_citation_point_at_the_wrong_line_is_red`, both in `src/networking_sockets/fake_udp.rs`).
  That unit cites `steamnetworkingfakeip.h` eight times. Its two *constants* have been read out of the
  header on every run since the day it landed, but its prose was never re-measured -- and two of the
  eight spans were wrong. The recommended-MTU sentence begins on line 10, so the cited 11-15 opened one
  line late; the hard-rule sentence is 17-18, and the cited 18-19 landed on the constant's own line,
  which is the worse of the two shapes: a citation that describes a rule while pointing at a number reads
  as verified to anyone who glances at it. Both are corrected here, and all eight are now checked against
  the vendored file, each with one phrase chosen to sit inside a *single* header line -- the ruler joins
  lines but keeps their `///` prefixes, so a phrase running across a break could never match and the
  check would fail for the wrong reason.
  Control: re-applying the original 18-19 span to the table makes the new test fail by name and print what
  those two lines really say; the file was then restored with `cp` and verified byte-for-byte with `cmp`.
  Same commit as the FakeIP citation ruler, same reason: prose corrections do not survive the next edit,
  so every one of these findings is turned into something that judges.
  Ledgers this moves: `module_sizes` `src/networking_sockets/fake_udp.rs` 317 -> 424 (registered total
  27728 -> 27835, and the +107 is that one file) and the lib test floor 163 -> 165, both re-recorded
  from the run; `api_coverage`, `type_surface`, `package_inventory`,
  `sdk_vendored`, `no_panic_sites` and `shell_hatches` compare unchanged.
- **`ledgers are tracked, populated and read`** (`scripts/ci/baseline_files.py`). This repo's gates
  *are* files, and two failure modes had already happened once each: a shipped gate whose baseline
  was swallowed by a `*.txt` ignore rule (so CI judged against a file that existed only on the
  machine that wrote it), and a ledger that legitimately went empty (the panic-site target is zero),
  after which "0 violations" and "the measurement never landed" print the same thing. The gate
  checks four properties per ledger: tracked by git; non-empty unless it carries a declared excuse
  with a reason; a declared reader whose own text names it (literally or through the parameterised
  prefix it builds); and a non-vacuous inventory (floor of 10 ledgers, measured today -- 10 files,
  1 excuse, 10 readers). An excuse attached to a ledger that has since gained content is red, so the
  exemption cannot be kept alive by habit, and a reader entry naming a ledger that no longer exists
  is red in the other direction.
- **`every gate can still judge red`** (`scripts/ci/canaries.py`, 29 assertions today). A gate that
  cannot fail is a log line. Each canary copies a ruler into a throwaway temp tree at the same
  relative position it occupies in the repository -- so the ruler's own root discovery points at the
  fixture, not the checkout -- builds the violating case, and asserts the exit code it should give.
  The real working tree is never mutated. This was not written out of purity: the documentation
  pointer ruler reported four *correct* citations as dead paths, because it matched a suffix inside a
  longer correct path -- an error that presents as the gate finding things, never as the gate being
  broken, and therefore invisible to every other step. Coverage: the comparator's both-direction
  judgement plus the `#scanned` marker rule (growth, shrink, new key, vanished key, matching pass,
  empty-with-marker pass, empty-without-marker refused); the panic ruler end-to-end (a fixture
  `unreachable!()` becomes a record, the comparator turns it red, and too-few-scanned-sources
  exits 2 rather than reporting a clean tree); the pointer ruler's four violation classes plus its
  vacuous-scan floor; the ledger gate's five violation classes plus its floor; and `api_coverage.py`
  refusing a wrong input path with exit 2 instead of announcing zero coverage. What it does not
  cover is listed in the script's own docstring rather than counted as green: `clippy_gate.py` and
  `test_floor.sh` need a real cargo build to fail meaningfully, and `workflow_inputs.py`,
  `package_inventory.py` and `sdk_vendored.py` would need a fixture the size of the repo.
  Building it caught two bugs in the canary itself, both of which had it passing for the wrong
  reason: injecting the fixture's reader map with `+=` instead of replacing the block left the ten
  real ledgers judged as "no longer exists", and appending the ghost-reader statement after
  `sys.exit(main())` meant it could never run.

- **`quality-extra` no longer cancels itself.** The advisory job (udeps / geiger / audit) moved to
  its own file, `.github/workflows/quality-extra.yml`, with `concurrency.cancel-in-progress: false`.
  The reason is measured, not assumed: on `ci.yml` the workflow-level concurrency cancelled every
  superseded run, and the three cancelled copies of this job had lived **5m14s, 7m46s and 10m17s** --
  all far below its own `timeout-minutes: 30`, so a timeout was never the cause -- each cancellation
  landing within seconds of its successor starting. A run that finishes takes ~14 minutes, so the
  verdicts could not survive a second push: across four consecutive SHAs udeps, geiger and audit
  reported nothing at all. Stated cost: pushes in quick succession now queue and complete instead of
  being killed, so runner minutes rise during bursts. `CI` keeps `cancel-in-progress: true` for
  everything that answers in minutes. The shell-hatch ledger records the move rather than growth --
  total 10 before and after, `ci.yml` 6 -> 0 and `quality-extra.yml` 0 -> 6. One commit is now two
  runs (13 jobs in `CI`, 1 in `Quality extra`), so "this SHA landed" means both of them.

- **`documented source pointers resolve`** (`scripts/ci/doc_pointers.py`, one step, one verdict).
  This repo's docs are evidence files: claims carry `src/<path>.rs:line` citations so a reader can
  check them. Three earlier restructures (`src/friends.rs` and `src/ugc.rs` becoming directory
  modules, `src/lib.rs` thinned) silently broke a chunk of those citations and no step went red --
  a doc whose pointers rot reads as a doc that was invented. The gate judges that a cited path is
  still tracked, that a cited line is inside the file, and that it is not a blank line (a pointer
  landing on whitespace cannot be what the sentence cites; a pointer that is in range but off by
  forty lines is *not* mechanically detectable, and the gate says so rather than claiming more).
  First run caught 7 drifted citations, all re-anchored by reading the code again rather than
  guessing: Pointers
  that name a file a document legitimately recorded as same-day evidence (the `defect-0N` logs, the
  CI evidence note) are **not rewritten** -- replacing a measurement with today's coordinates would
  be a guess -- so they go into `baselines/doc_pointers_historical.txt`, one `<doc> <token>` pair
  each, and that list is judged like a debt register: an entry no document cites any more is red.
  Measured on first run: 128 pointer tokens across 10 docs, 19 registered as historical.
  Controls, each fired red and then restored byte-for-byte: rename a path by one character; push a
  line past EOF; point a line at a blank line; delete a register entry that is still cited; add a
  register entry nobody cites. The vacuous-scan floor was proved with a throwaway copy of the
  script (`MIN_TOKENS` raised out of reach) which exits 2 rather than reporting a clean doc set.
  The ruler needed one fix of its own before it could be trusted: without a leading path boundary,
  the tail of the *correct* citation `steamworks-sys/src/windows_bindings.rs` was matched as a
  second, bare pointer of its own, so four good citations were registered as dead -- which is the
  familiar failure mode of a parser reporting violations because it is looking at the wrong string.
- **An empty measurement can no longer pass `compare_baseline.py`.** The panic-site ledger became
  genuinely empty this session, and with an empty baseline an empty current file used to print
  `baseline holds: 0 entries` -- indistinguishable from the ruler having read nothing (which is
  exactly what happened once here, when `api_coverage.py` was invoked with `src` in place of the
  bindings path and produced an empty file). The rulers now emit `#scanned <files seen>`
  (`no_panic_on_unknown_enum.py` does) and the comparator refuses any zero-record measurement that
  carries no such marker, exiting 2 as "not judged". Proved both ways: an empty file without the
  marker exits 2, the same file with `#scanned 51` passes.

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
- `src/friends/mod.rs` (1321) closes the last of those three: `friends/callbacks.rs` (155) takes
  the ten callback payloads and their `impl_callback!` bridges, `friends/friend.rs` (401) takes
  `Friend` together with the `FriendState` / `FriendRelationship` vocabularies and the mapping
  tests that pin them, and `friends/mod.rs` (790) is left with the `ISteamFriends` accessor.
  Public paths are unchanged again (`pub use callbacks::*`, `pub use friend::*`); the only shapes
  that had to move are `Friend`'s construction, which goes through a new `pub(crate) fn new`
  because its fields stay private, and three module-internal helpers (`steam_string`,
  `persona_state_from`, `relationship_from`) that became `pub(crate)` so the parent can still
  reach them -- none of the three is reachable from outside the crate, so `type_surface` is
  byte-identical at 80 keys / 823 methods and the lib suite still reaches 123 tests
  (108 passed + 15 ignored). The coverage ledger is unchanged at 612 of 913, the size ruler named
  all 3 of its disagreements before anything was recorded (two unregistered new keys plus the
  stale 1321), and packaging gained exactly the 2 new sources.
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

### Fixed

- **Retracted, one commit later: the lock policy was not 22 requests and one rule, and the gate that
  said so was blind to the difference** (`src/lib.rs`, `src/utils/locks.rs`, `scripts/ci/lock_poisons.py`).
  `a5ac8ed` recorded "22 guard requests, 18 aborted, 4 recovered inline -- one policy" and shipped a gate
  that judged exactly those two shapes. Checking the sentence with `grep` rather than trusting the gate
  found a **23rd**: `Callbacks::remove_handler` did `match self.callbacks.lock() { Ok(..) => ..,
  Err(..) => eprintln!(..) }`. That is neither of the two policies the changelog named -- it is a third
  (report, then skip), it was taken outside the policy file, and the gate printed 0 the whole time it
  was in the tree. Two sentences from that same commit also need correcting: I wrote that the two
  remaining `expect()` calls in Drop paths panicked on any reading, and measured now, one of them is
  `CString::new("Handle was dropped").unwrap()` -- a literal with no nul, so it cannot fail -- and the
  other, `receive_event`, is not a `Drop` at all. The third one I left alone
  (`.expect("internal connection was removed before being dropped")` in `NetConnection::drop`) is a real
  hazard *shape* -- panicking in a drop can abort the process mid-unwind -- but I have not shown it
  reachable: the entry is inserted when the independent connection is created and no other code removes
  it, so today it is an undocumented-unreachable arm and it is registered as such rather than given a
  test it could not pass honestly.
  What changed: `try_mutex_guard` in `src/utils/locks.rs` is now where "recover, report, never abort in a
  drop" is decided, and `remove_handler` calls it (same behaviour it had -- the registry is skipped and a
  line is printed -- but the decision lives in the one file). And the gate judges five things instead of
  two: an abort, an inspection of the lock's own `Result`, a `match` that decides, a re-implemented
  recovery, and -- the new part -- **a guard request outside the policy file that none of those explains
  stops the verdict entirely (exit 2)**. That last arm is the self-correction generalised: the tree has
  zero requests in it today, so the arm cannot be proved by the real corpus, only by a fixture, which is
  what a canary is for.
  104 canary assertions, up from 98. Three of the new ones are the lock shapes (match, inspect,
  unclassifiable); each was disproved by disabling its pattern in a throwaway copy -- the canary goes red
  either by losing the named message or by falling through to the refusal, so none of the three is
  carried by an assertion that would pass anyway. Two more report honestly rather than flattering: the
  dedup rule "one violation per site" was caught by its own assertion double-counting
  the `second_policy.rs` canary fixture (it exists only in the ruler's temp tree, hence no `src/` prefix here), and the ordering of this canary was wrong in the first draft -- the
  unclassifiable fixture was written *before* the site assertions, so with the whole tree refusing to
  answer, nine assertions compared against an empty stdout and failed for a reason unrelated to what
  they tested.
  Numbers, re-measured after the change: 23 guard requests, **0 outside the policy file**; the ledger is
  still 0 sites; production panic-capable expressions are unchanged at **13** (this unit moved a
  decision, not a panic).


### Fixed

- **Asking for a network resource's status no longer aborts on a number that is not a status**
  (`src/networking_types/state.rs`). `TryFrom<ESteamNetworkingAvailability> for NetworkingAvailability`
  ended in `panic!("invalid networking availability {:?}", value)`. Reading the header against the table:
  Steam declares nine values and all nine were mapped -- "Negative values indicate a problem."
  (`steamnetworkingtypes.h:103`) is the ladder's own rule, and the four rungs that are not problems are
  `NeverTried`, `Waiting`, `Attempting`, `Current`. What reached the panic was
  "k_ESteamNetworkingAvailability__Force32bit = 0x7fffffff," (`steamnetworkingtypes.h:124`): the width
  sentinel C enums carry so the compiler sizes them, which is not a status at all. So the function that
  already returns `Result<_, NetworkingAvailabilityError>` answered a harmless number by aborting the
  caller, and a future SDK adding a rung would have done the same. The arm is now
  `NetworkingAvailabilityError::Unrecognized(i32)`, which keeps the number instead of naming it wrongly;
  unlike the connection-state table fixed in `836222f`, **no declared state was missing here** -- that
  difference is what the table test now pins.
  Three tests: the nine-row ladder (four positive rungs, five named problems, and an assertion that the
  two groups really total nine, so the table cannot silently drop a rung), the width sentinel arriving as
  `Unrecognized(0x7fffffff)` with its number intact, and pairwise distinctness among the named answers.
  One of my first three drafts was thrown out before it shipped: a loop that asserted
  `n == code || n == 0x7fffffff` passed for every input because of the `||` -- a green that judged
  nothing, which is the same class of defect this repository has now caught in a `matches!` binding, a
  comment-scoped ruler and a line-based panic count. Five mutants, all caught: restoring the `panic!` died
  by aborting (the behaviour the test is named for), dropping the number, reading `Current` as
  `NeverTried`, treating `Unknown` as available, and swapping two failure rungs each died on an
  `assert_eq!`.
  Production panic-capable expressions: **14 -> 13**. `header_citations` picked up the two spans above
  (194 total, all three debt columns 0) after my first draft carried a scare-quoted paraphrase and a bare
  citation with no phrase next to it -- both paid back by rewording, not by registering the growth.


### Fixed

- **Lock poisoning stopped being re-decided line by line: 22 guard requests, one policy**
  (`src/utils/locks.rs`, and the call sites in `src/lib.rs`, `src/callback.rs`, `src/matchmaking_servers.rs`,
  `src/networking_sockets_callback.rs`, `src/utils/warning.rs`, `src/networking_sockets/{connection,
  listen_socket,poll_group,fake_udp}.rs`). Every lock here is taken inside callback dispatch, which runs
  user code, so a user callback that panics poisons the lock and the *next* guard request pays for it --
  in a different caller, much later. Measured on the tree as it was: 22 requests, **18 of them aborted**
  and **4 already recovering inline** in `src/networking_sockets/fake_udp.rs`, i.e. the repository was
  running two policies at once. Two of the 18 were the worst-placed of all: `.expect("warning func lock
  poisoned")` on `RwLock`s reached from an `extern "C"` callback, where a panic does not unwind into Rust
  at all. The 14 conversions in the first pass were written `x.lock().unwrap()` on one line; the other 4
  were a chain with `.unwrap()` on its own line, which is why a line-based count reported 12 and was wrong
  -- the same observation-layer trap as the 81-vs-43 panic figure, met again from the other side.
  `mutex_guard`, `read_guard` and `write_guard` in `src/utils/locks.rs` now hold the decision, with the
  trade written where it is taken: `into_inner` hands over a table a panicking thread may have left
  mid-update, which is acceptable for `handle -> sender` maps and boxed-closure queues and is not
  presented as free. Recovery is a maintainer's call, made explicitly, not a default I picked.
  **New gate** `scripts/ci/lock_poisons.py`, zero-tolerance ledger `lock_poisons.txt` (14th ledger,
  `MIN_BASELINES` 13 -> 14, one CI step, one replay step), judging two halves: no guard request may abort
  on poison, and nothing outside the policy file may re-implement the recovery -- because a rule that only
  counts aborts would have gone green on the day someone finished this job by inverting it. It also
  refuses to answer (rc=2) if `src/utils/locks.rs` is missing or no longer contains all three guard kinds
  plus the recovery, so "0 violations" can never come from a deleted policy. 15 new canary assertions
  (98 total), each disproved: neutralising the test-body exclusion moves a fixture file from 1 site to 2,
  neutralising the second-policy half makes that site disappear, and the first version of one assertion
  passed for the wrong reason -- I had named the fixture `bare_test.rs`, which `TEST_MATERIAL` already
  treats as a whole test file, so nothing in it was ever judged.
  `api_coverage.py` gained `strip_test_fns`, the third scoping rule (bare `#[test]` fns written at file
  scope, which rustc builds only for the harness and neither existing rule could see); `cstring_unwraps.py`
  now uses the same three, and its `#scanned` total moved 29 -> 28 occurrences with exactly one site
  changing -- `src/user_stats/global_stats.rs`, a fixture assertion inside a test fn -- so the shared rule
  neither hid a production conversion nor invented one.
  Production panic-capable expressions: **32 -> 14**; the lock gate reports 0 policy breaks across 4 guard
  requests in 126 hand-written sources. Two side effects of shortening those chains: the shipped `.crate`
  gains `src/utils/locks.rs` (`package_inventory` 142 -> 143 files, re-recorded), and `src/lib.rs` lost
  enough lines that a pointer in `docs/SuperSteam-Design.md` moved two lines off its target -- fixed by
  re-anchoring it to `766-775`/`:772`, and the same drift inside a *published* CHANGELOG sentence was
  answered by naming the `timeline` accessor rather than by registering a line number nobody can follow
  any more (`doc_pointers_historical.txt` only exempts paths that no longer exist, and it stays
  byte-identical here).


- **Steam declares nine connection states; this crate had names for six**
  (`src/networking_types/state.rs`, `src/networking_types/events.rs`). The table on
  `NetworkingConnectionState` stopped at `ProblemDetectedLocally`, while the SDK header continues with
  three negative ones: "k_ESteamNetworkingConnectionState_FinWait = -1," (`steamnetworkingtypes.h:430`),
  "k_ESteamNetworkingConnectionState_Linger = -2" (`steamnetworkingtypes.h:446`) and
  "Connection is completely inactive and ready to be destroyed" /
  "k_ESteamNetworkingConnectionState_Dead = -3," (`steamnetworkingtypes.h:448-449`). Those are states a
  connection passes through, and `SteamNetConnectionStatusChangedCallback_t` carries one of them in
  `m_eOldState` -- which the status callback filled with `cb.m_eOldState.try_into().unwrap()`. So the
  crate's answer to a value its own header declares was, in one path, an abort; and in two others a
  lie-shaped fallback, because `NetConnectionInfo::state()` returned `Err(InvalidConnectionState)` for
  them, which made `independent_connection_callback` return early and drop the event, and made
  `into_listen_socket_event` report `UnknownType` where the state was known to Steam.
  Now: `#[non_exhaustive]` on the enum, a variant for each of the three states, and `Unknown(i32)` that
  keeps any other discriminant verbatim -- including the `__Force32Bit` width filler, which is not a state
  and is not guessed at. One total table, `state_from_raw`, is what both directions read, and the callback
  calls it directly, so the `unwrap` is gone rather than wrapped.
  Shape, stated plainly because the maintainer's pick caused it: adding variants plus
  `#[non_exhaustive]` stops a downstream exhaustive `match` compiling, and the outgoing
  `From<NetworkingConnectionState> for sys::ESteamNetworkingConnectionState` became `TryFrom` -- an
  unnamed number has no constant to carry it back, so refusing is the only honest answer, and the old
  infallible `From` could never have been asked. Nothing in this repository used that direction: the
  generated bindings mention the state enum only as three struct fields (`m_eState` twice, `m_eOldState`
  once) and never as a call parameter, so no Steam API consumes it. `TryFrom<sys::E...> for
  NetworkingConnectionState` keeps `Error = InvalidConnectionState` although its arm is now unreachable,
  because `state()`'s public signature and `NetConnectionError::UnknownType` name that type; retiring it
  would be a wider break than this one, and the impl's doc says so.
  Five tests: the nine-row table, the regression itself (a teardown previous state is a state, not an
  error, and not the dummy `None`), `__Force32Bit` arriving as `Unknown(2147483647)` with its number
  intact, a round trip through the raw enum for all nine, and the one genuinely reachable failure in the
  outgoing direction. Five mutants, all caught by `assert_eq!` rather than by aborting: dropping the
  `FinWait` arm and mapping `Connected` to `FindingRoute` are each caught by two tests, while losing
  `Unknown`'s number, sending `Unknown` back as `None`, and swapping `Linger` for `Dead` on the way out are
  each caught by exactly one -- which is how those three were checked to be load-bearing, not incidental.
  Not claimed: that a live client was observed sending one of these states. That needs a session and is
  registered as uncovered. What is claimed is narrower and checkable in the tree: the header declares
  them, the callback carries a previous state, and this crate had no name for them.
  Ledger trail: `header_citations` 188 -> 192 spans across 35 files, all three debt columns 0 (the four new
  spans are the declarations quoted above); `type_surface` unchanged at 857 -- no new public item;
  production panic-capable expressions **33 -> 32**; `module_sizes` took the table, its docs and the tests
  (`src/networking_types/state.rs` 180 -> 361, `src/networking_types/events.rs` 217 -> 224), registered
  total 29534 -> 29722.



- **A listen-socket dispatch stops aborting on a state number it cannot name**
  (`src/networking_sockets_callback.rs`). `listen_socket_callback` read the connection's state with
  `event.connection_info.state().expect("invalid state")` -- and `state()` is exactly the fallible one of
  this file's two end-side reads, `Result<NetworkingConnectionState, InvalidConnectionState>`, because
  bindgen's `ENetConnection` is non-exhaustive and Steam can answer with a discriminant the crate has no
  variant for. That value had one use: deciding whether to reject a connection nobody is left to accept.
  So an unnameable state now takes the same path as any other non-`Connecting` state (`.ok()`, compared
  as `Some(NetworkingConnectionState::Connecting)`), which is what the sibling method in the same file
  already did with the same answer -- `independent_connection_callback` reads it as
  `let Ok(new_state) = .. else { return; }`. The panic was this file disagreeing with itself, not a
  precondition anyone wrote down.
  No public shape changed and no new test is claimed: the call site needs an `Arc<InnerSocket>`, i.e. a
  running client, so it is registered as uncovered -- the judgment it depends on
  (`NetworkingConnectionState`'s conversion being fallible) is the crate's own declared signature, and the
  unknown-answer handling now matches the other path that has to make the same decision.
  Production panic-capable expressions: **34 -> 33**.


- **The end-reason table got its first tests, and the two `expect(..)` calls that pretended it could
  fail are gone** (`src/networking_types/end.rs`, `src/networking_types/connection_info.rs`,
  `src/networking_types/events.rs`). `NetConnectionEnd` is built from Steam's plain `int` by
  `impl From<i32>`, and that table -- sentinel, the two application ranges, every named system code,
  `end => Self::Other(end)` for anything else -- had never been judged by anything in this repository.
  Two of its readers claimed otherwise: `NetConnectionInfo::end_reason` finished with
  `.expect("Unknown end reason could not be converted")` on a conversion whose error type is
  `Infallible` (a dead arm that read like a failure mode), and `into_listen_socket_event` finished a
  disconnect with `.expect("disconnect event received, but no valid end reason was given")` -- reachable,
  because `end_reason` returns `None` for exactly one input, the code Steam itself spells
  `k_ESteamNetConnectionEnd_Invalid`. The first is now `.into()` and its doc says the conversion cannot
  fail; the second delivers the event carrying `NetConnectionEnd::Invalid`, since the disconnection is the
  fact the listener needs and aborting the callback that was asked to route it hides that fact too.
  Six tests cover the table: the sentinel, both application ranges keeping their own number at all four
  boundaries, a named system code, `Other` for an undefined and a negative and a maximum code, a
  round-trip through `i32` for a member of every family, and an anti-vacuity read that builds the value
  with `4242.into()` so a mutant that made the conversion partial could not hide. Six mutants: the
  sentinel decoding as `Other(0)`, the app range stopping before the exception family, `Other` losing its
  number on the way back, a system code mapping to the wrong variant, and an unknown code aborting -- each
  died on an `assert_eq!` except the last, which died by aborting (the behaviour under test) and was
  caught by three tests including the anti-vacuity one. One mutant of mine was not a measurement at all:
  the first attempt replaced the sentinel arm with `None =>`, which does not compile in that match, so it
  reported a compile error rather than a killed mutant and was re-run as the `Other(0)` variant above.
  Neither call site itself is CI-judged -- `into_listen_socket_event` needs an `Arc<InnerSocket>`, i.e. a
  live client -- so that half is registered as uncovered rather than tested with a fixture.
  Production panic-capable expressions: **34** (was 36). `NetConnectionEnd` is public; nothing here
  changed its shape or any signature.



- **`get_achievement_names` panicked on the two ordinary states of an achievement list**
  (`src/user_stats/achievements.rs`). `get_num_achievements` answers `Err(())` whenever Steam's count is
  zero -- a mapping this file documents itself ("Returns 0 if the current App ID has no achievements",
  plus the note that AppId 480 returns an error) -- and the names getter read that answer with
  `.expect("Failed to get number of achievements")`. So asking a game for its achievement names before
  `request_current_stats()` had landed, or asking one that defines none at all, aborted the caller's
  frame from a function whose return type is already `Option`. It propagates now (`.ok()?`), and the
  count decision moved into `achievement_count`, a pure function three tests judge: positive counts come
  through as themselves, zero is the documented error case rather than a panic, and an anti-vacuity test
  asserts `achievement_count(0)` really is an `Err` -- because without it, mutating the mapping to be
  total would have left the getter's own tests green. Four mutants, all caught: `> 1`, `Ok(0)`, off-by-one
  each died on an `assert_eq!`, and replacing the error with `panic!` died by aborting, which is the
  behaviour the two tests name; the anti-vacuity test is what caught the `Ok(0)` and `panic!` pair rather
  than leaving them to the value assertions.
- **A stray byte from the Big Picture keyboard no longer discards a submission**
  (`src/utils/text_input.rs`). `get_entered_gamepad_text_input` filled its buffer and then ran
  `String::from_utf8(buf).expect("Steamworks returned invalid UTF-8 string")`. Steam's own contract for
  the fill is "virtual bool GetEnteredGamepadTextInput( char *pchText, uint32 cchText ) = 0;"
  (`isteamutils.h:153`): a boolean for whether there was an answer, and nothing about encoding. The
  bytes are whatever the overlay produced, so the read is lossy now -- `None` would have been
  indistinguishable from the user cancelling, while the boolean had already said there was an answer.
  No lock is claimed for this one: the change is a single standard-library call inside an `unsafe` block
  that needs a live client to reach, and the crate's lossy-reading convention is tested where it lives
  (`src/utils/strings.rs`).
  Both drafts earned the same lesson twice more: my prose inside double quotes -- three paraphrases and a
  hedged restatement -- was counted as 4 then 3 `~unjudged` spans, and the debt was paid by rewriting the
  sentences in the crate's own voice rather than by registering the growth. `header_citations` now reads
  188 spans across 34 files with all three columns at 0. Production panic-capable expressions:
  **38 -> 36**. `module_sizes` moved where the docs and tests landed and nowhere else --
  `src/user_stats/achievements.rs` 91 -> 138, `src/utils/text_input.rs` 187 -> 195, registered total
  29370 -> 29425.


- **Retracted: "the verdict always completes" for `quality-extra` -- `cancel-in-progress: false` only
  protects the run that is already running** (`.github/workflows/quality-extra.yml`). The comment written
  when the advisory trio was split out of `ci.yml` explained, correctly, that three ~14-minute jobs had
  been killed by *my own* pushes under the CI workflow's `cancel-in-progress: true` (measured: 5m14s,
  7m46s, 10m17s, all far below the job's own `timeout-minutes: 30`), and set the new workflow's flag to
  `false` so a later run queues instead of killing the one in flight -- "and the verdict always
  completes". The first half holds; the second was read off a run that falsifies it: run 39158786643
  (SHA `3c59bee`) was created 19:26:36Z, judged `cancelled` at 19:32:26Z, and carried **zero jobs** -- it
  never started one. At that moment `772c811`'s run was still in progress and `f7091b5`'s was queued,
  which is the shape of the limit: one run in flight plus one queued per group, and a third push replaces
  the queued one. Under a burst, therefore, the middle SHAs get no `udeps` / `geiger` / `audit` answer at
  all, and a recent history that looks covered can hide several SHAs with no advisory verdict.
  The design is not changed, for a measured reason rather than an assumption: `master` has no branch
  protection at all (`GET /repos/bfxh/steam-sdk-rs/branches/master/protection` answers 404 `Branch not
  protected`), so neither workflow blocks anything mechanically -- the 13 jobs of `ci.yml` are verdicts
  that are read, not gates that stop a push, and the trio here is advisory on top of that. Queueing every
  SHA through another ~14 minutes would buy runner minutes, not safety. What changed is the claim. The comment now says the trio is not
  coverage, and that when a specific SHA's answer is actually wanted it is fetched with
  `gh workflow run quality-extra.yml --ref <sha>` rather than presumed from the push history.

- **The rich-presence join callback stops aborting on the payload it was asked to decode**
  (`src/friends/callbacks.rs`). `GameRichPresenceJoinRequested_t` carries
  "char m_rgchConnect[k_cchMaxRichPresenceValueLength];" (`isteamfriends.h:570`, with
  "enum { k_cchMaxRichPresenceValueLength = 256 };" at `:125`) -- an array, with no terminator promised
  and no encoding declared. The bridge read it with `CStr::from_bytes_until_nul(..).expect(..)` and then
  `.to_str().expect(..)`, and the comment sitting above those two lines already conceded the case it
  panicked on: "Technically, this C string does not have to be UTF-8, but I think for all realistic uses
  it will be." This crate has since grown the total reader for exactly that shape -- `fixed_string` in
  `src/utils/strings.rs`, whose module doc names this failure mode ("a reader that demands UTF-8 can
  abort the callback it is running in") -- and this site was simply left behind. It now uses that helper:
  valid UTF-8 decodes identically, an unterminated array reads as the array's own text instead of
  unwinding through dispatch, and stray bytes become U+FFFD.
  No new test is claimed here, and the reason is stated rather than papered over: the two panics lived
  inside an `impl_callback!` block, which expands into the callback dispatch path, so the only thing a
  fixture could assert without a running client is `fixed_string` itself -- already judged by
  `stops_at_the_first_terminator`, `an_unterminated_array_is_bounded_by_the_array` and
  `invalid_utf8_is_replaced_not_fatal` in its own module. What
  remains unjudged is the one-line wiring claim "this callback uses that helper", registered as uncovered
  like every other macro-body wiring in this crate.
  Panic-capable expressions in production sources: **40 -> 38**; `module_sizes` registered total
  29373 -> 29370. One process note, because it is the kind of thing that gets quietly re-recorded: my
  first version of that citation wrapped both header lines in backticks, and `header_citations.py`'s
  phrase matcher looks for double quotes -- so the two new spans arrived as `~undeclared` debt, 0 -> 2.
  The ledger was re-recorded back out (`git checkout`) and the debt paid in the currency the rule wants,
  by quoting the lines verbatim in the form the rest of the crate uses; 186 spans now, all three columns
  0.


- **A server-list ping that is not a `u64` no longer aborts the list** (`src/matchmaking_servers.rs`).
  `GameServerItem::from_ptr` built its `Duration` field with `raw.m_nPing.try_into().unwrap()` -- a
  fallible conversion of an `int` Steam wrote, whose failure mode was a panic while decoding a query
  answer, inside a callback frame. The header types the field and seeds it but never bounds its sign:
  "current ping time in milliseconds" (`matchmakingtypes.h:177`) and the constructor line
  "m_nPing = m_nAppID = m_nPlayers = m_nMaxPlayers = m_nBotPlayers = m_ulTimeLastPlayed = m_nServerVersion = 0"
  (`matchmakingtypes.h:211`). `ping_duration` now reads a negative as no measured delay and says so in
  the doc -- the crate's own choice about an undocumented sign, not a claim about Steam.
  What it deliberately does *not* do is borrow the networking layer's sentinels: `PingEstimate` keeps
  `-1` and `-2` as `Failed` and `Unknown` because `steamnetworkingtypes.h:1089-1090` declares those two
  constants for *route estimates*, and treating one field's named values as another field's would be a
  claim no header supports. The sharper public answer is a three-state `ping` (or `Option<Duration>`),
  which changes a field type on a struct users read directly, so it is offered rather than taken.
  Three tests, including the anti-vacuity one that asserts `u64::try_from(-1).is_err()` -- so the
  fallback arm is proved live rather than decorated. Four mutants, all caught: `unwrap()` back in place
  died by aborting (the behaviour under test), `unwrap_or(1)`, `from_secs`, and flooring positives at 1
  each died on an `assert_eq!`, so those are locks. Ledger: `module_sizes` 587 -> 641 for this file
  (40 of the 54 lines are the doc and the three tests; the helper is inline in the module that owns the
  struct, the way `messages_received` and `name_answer` are), registered total 29319 -> 29373.
  `header_citations` picked up the two new spans: 184 spans, 31 files citing, all three debt columns 0
  (three zero rows registered for `src/matchmaking_servers.rs`, which cited no header before).
  Inserting 54 lines also moved two pointers in `docs/SuperSteam-Design.md` off their targets: the
  inbound `Ipv4Addr::from(m_unIP)` reading drifted 214 -> 268, and the outbound `ping_server` body
  454-465 -> 508-519. `doc_pointers.py` went red on the first and *stayed green on the second* -- it
  rejects a blank or missing line, not a line that moved to different code, which is why both spans
  were re-anchored by reading the file rather than by trusting the ruler's verdict.
  Panic-capable expressions in production sources: **41 -> 40** (`.unwrap()` 21 -> 20).

- **Both `send_messages` sites stop panicking on an error code this crate has never seen**
  (`src/networking_sockets/mod.rs`, `src/networking_sockets/listen_socket.rs`). The out-array of
  `ISteamNetworkingSockets::SendMessages` is `int64` -- `pOutMessageNumberOrResult: *mut int64` in the
  generated binding, and "If sending failed, then a negative EResult value is placed into the array.
  For example, the array will hold -k_EResultInvalidState if the connection was in an invalid state."
  (`isteamnetworkingsockets.h:297-299`) -- and both callers read it with
  `Err((-x).try_into().expect("invalid error code"))`. `SteamError::try_from` answers
  `InvalidSteamError::Unknown(raw)` for a code outside the mapped table, which is precisely the case
  `expect` turned into an abort inside the caller's frame, on a value Steam chose, in a function whose
  own return type is `Vec<SteamResult<MessageNumber>>`. It now maps through `send_outcome`, a pure
  function next to `messages_received`, and an unmapped code becomes `SteamError::Generic` -- the
  convention this crate already set for the same shape of answer when it fixed the FakeIP result status
  (`src/lib.rs`, "An answer this enum has no variant for becomes `Fail` rather than a panic"; the exact
  number stays reachable through `SteamError::try_from`). No public type or signature changed.
  The negation is `checked_neg`, not `-x`: `-i64::MIN` is not representable, and the old form panicked
  in a debug build while silently wrapping in a release one. Four tests cover the slot (non-negative
  is a message number; a known negative decodes to the variant it names -- chosen to be `InvalidState`
  so it cannot be confused with the fallback; `-4242` and `i64::MIN` are failures, not aborts).
  Five mutants, all caught, with the judging mechanism named rather than assumed: flipping `>= 0` to
  `> 0`, collapsing every failure to `Generic`, and never flipping the sign each went red on an
  `assert`, so those three are locks; making the fallback `panic!` and un-checking the negation went
  red because the function itself aborted -- which is the behaviour the two tests are named after, but
  note the second of those only dies because the test profile has overflow checks on, and in a release
  build the unchecked mutant would have wrapped into the same `Generic` answer by luck rather than by
  contract.
  Measured with the same ruler before and after: **43 -> 41** caller-visible panic-capable
  expressions in production sources. The citation ruler caught one of my own sentences on its first run
  (1 `~unjudged`): I had put the phrase `"a failure we cannot name"` in quotation marks in the same
  block as two genuine ones, and a quote this repository wrote is debt, not a verdict. Paid by taking
  the marks off, not by re-recording the debt column -- the ruler now reports 181 spans with all three debt columns at
  0, the two `isteamnetworkingsockets.h` quotes matching verbatim.
  Ledger moved deliberately, in both directions:
  `module_sizes` `src/networking_sockets/mod.rs` 315 -> 383 (the pure function, its doc and its four
  tests) and `src/networking_sockets/listen_socket.rs` 122 -> 113 (the caller it no longer duplicates);
  registered total 29260 -> 29319. The first 380 was measured before `cargo fmt` re-wrapped the
  `checked_neg` chain, and the entry quotes the number the ledger actually holds.

- **The count of production panic sites was 43, not 81: the ruler could not see a bare `#[test]`
  function living in a production file** (`scripts/ci/`, measurement only -- no gate was built on the
  wrong number). Two scoping rules already exist here: `is_test_material` drops whole test files
  (`src/lib_tests.rs` and friends) and `strip_test_modules` drops `#[cfg(test)] mod name { ... }`
  blocks. Neither sees the third form, which this repository also uses: `#[test]` fns written at module
  level, gated by nothing at all, because rustc only builds them for the test harness. 38 of the 79
  sites found by the two existing rules sat in that form -- `src/remote_storage/mod.rs` alone accounts
  for 12 of them (`test_one_shot_cloud_io`, `test_cloud`), and `matchmaking_servers.rs` another 7.
  The number the next unit had been handed was therefore 1.9x its real target. The fix is in the
  measuring tool, not in the tree: the triage script now blanks every `#[test]`-attributed function
  body by brace counting, and re-running it against the tree is what produced the 43 above.
  That scratch script is deliberately not shipped as a gate: "is this expression caller-triggerable"
  needs a reading of the enclosing call, not a pattern count, and a ledger that pretends otherwise
  would be the same vocabulary test `shell_hatches.py` had to stop being. The 43 split as 14 lock
  poisoning (`Mutex`/`RwLock::lock().unwrap()`, reachable only after a holder already panicked),
  ~10 decisions made from a Steam-supplied value or enum, 6 caller preconditions asserted in the
  crate's documented idiom (one of which, `src/ugc/read.rs:110`, is a `const _: () = assert!(..)` and
  never runs at all), and the rest internal invariants and un-failable literals.


- **The CString ledger could have counted a test's own fixture as user-facing debt, and the rule that
  stops it was already in this repo under a different name** (`scripts/ci/api_coverage.py`,
  `scripts/ci/cstring_unwraps.py`, `canaries.py`). This crate keeps its bigger suites out of line --
  `src/lib_tests.rs` and `src/ugc/ugc_tests.rs` are `#[cfg(test)] mod name;`, so nothing *inside* those
  files is marked as test code and `strip_test_modules` cannot see them. Measured while sizing the next
  class of work: 173 panic-capable expressions in "non-test" sources, of which **92 sat in exactly those
  two files** -- a 1.1x overcount that would have sent the next unit after the wrong number.
  The fix is not a new rule: `api_coverage.main` already refused to treat such files as wrappers, via an
  inline name regex plus a hard error if one turned out to call a Steam interface. That predicate is now
  `api_coverage.is_test_material`, named once and reused, so the debt ledger skips the same files the
  coverage ledger does -- after I had already written a second, weaker mechanism (`out_of_line_test_mods`,
  which inferred test files from `mod` declarations and silently skipped them); the duplicate is removed
  rather than kept alongside, and the surviving rule is the stricter of the two because it *refuses*
  instead of quietly excluding.
  Two honesty details that came out of the same change. `#scanned` now counts the files the ruler
  actually read (125, not 127) -- a marker proving "I looked" must not count files it skipped. And the
  new canary assertion ("a whole-file test module is not debt") was disproved rather than trusted: with
  the predicate disabled in a throwaway copy, `fixture_tests.rs` does appear as a site, so the
  assertion passes for a reason (74 canary assertions, up from 73).

- **Retracted a second time: the `-p steamworks` rescope did not fix the warnings gate either, and
  neither did the crate-level `#![deny(warnings)]` that replaced it** (`.github/workflows/ci.yml`,
  `steamworks-sys/src/lib.rs`, `scripts/ci/workflow_inputs.py`, `scripts/ci/canaries.py`).
  `8f59be4` claimed its package filter "keeps the assertion actually being made". It did not: cargo
  applies `RUSTFLAGS` to every crate it builds in that invocation, and package selection decides who
  gets *built*, not who gets the *flags*. Same three `1.80.0` jobs, same `dead_code` error.
  The third attempt moved the ban into the crate and deleted the `RUSTFLAGS`. It compiles clean on
  both toolchains, and it is still the wrong mechanism: a clippy lint *is* a rustc lint, so a crate
  attribute is honoured by the clippy step too. Measured with `#![deny(warnings)]` in `src/lib.rs`,
  `cargo clippy --locked --all-targets --all-features` failed with **99 errors** in this crate -- the
  style backlog (`result_unit_err`, `type_complexity`, doc-tabs) that the repo deliberately keeps
  advisory, with only `clippy::correctness` and `clippy::suspicious` denied by `clippy_gate.py` and no
  baseline to slide on. A gate that silently re-tieres the tool next to it is not the same guarantee.
  So the fix is the one line the first two attempts were working around: `#![allow(dead_code)]` in
  `steamworks-sys/src/lib.rs`, with the reason written next to it. That file is hand-written (three
  `allow` attributes and four `include!`s), so it survives a bindgen rerun; the generated
  `*_bindings.rs` is not touched and its warnings stay honest. The A/B is one command, one toolchain,
  one line: `RUSTFLAGS="-D warnings" cargo +1.80.0 check --locked --all-targets` exits 101 on
  "struct `ISteamContentServer` is never constructed" without the allow and 0 with it (this platform
  reproduces one of the two errors the CI matrix reported -- `obstack` does not fire here), and stable
  `--all-features` is 0 either way, which is exactly why only MSRV jobs ever saw it. The ban is now a
  property of the one `check (all targets, rustc warnings denied)` step rather than of two steps: a
  second `cargo check -p steamworks` after it would be a cache hit, i.e. a step that prints `Finished`
  and decides nothing.
  Two things pin the shape so it cannot regress quietly. `workflow_inputs.py` gained a fourth fact --
  the ban must exist, must be on a `cargo check` step, must not be on a `cargo clippy` step, and a scan
  that saw no clippy step at all is refused rather than passed -- and `workflow_inputs.py` is no longer
  on this file's un-canariable list: the fixture it was said to need turned out to be one workflow file
  and a `git add`. 83 canary assertions, up from 74. Two of them passed for the wrong reason at first
  and were rebuilt: the "ban moved onto clippy" fixture also tripped the exists-floor, so the ban now
  stays on the check step too and only the tier rule can fire (proved by neutralising that rule in a
  throwaway copy -- the fixture goes green). And a `python … && cargo …` chain whose python half died on
  an assertion printed `rc=1` as though cargo had run; the without-allow number above is from the
  re-run, not from that line.
  The offer in the entry below -- install the pinned MSRV toolchain so MSRV-only behaviour is visible
  before a push -- is taken: `1.80.0-x86_64-pc-windows-gnu` is installed here and the local replay runs
  both `-D warnings` shapes. Both mistakes reached CI for the same reason: the chain had no 1.80.0
  compile in it at all.
  Registered rather than widened: `$EXAMPLES` in those two command lines has never been assigned in
  either workflow, so it expands to empty and the quality step compiles the root package plus its
  same-invocation dependencies. The four `examples/*` members are compiled by their own
  `examples (workspace members)` job with `--workspace --features steamworks/futures`, and that
  command deliberately carries no `-D warnings` -- measured on stable it is clean (rc=0 with the flag
  set), but the same flag would reach `fontdue` and the other registry dependencies that command pulls
  in, and the same command run on 1.80.0 stops long before any lint matters: `error[E0658]: use of
  unstable library feature 'integer_sign_cast'` in `fontdue` (which is why that job is stable-only).
  Buying four small crates by promising that no dependency will ever emit a new warning is the same
  trade this unit had to make twice, in the other direction.

- **Retracted: the rustc-warnings gate did not only bite hand-written code, and MSRV said so**
  (`.github/workflows/ci.yml`). The step as written in `ee60ab9` denied warnings for the whole workspace
  and its comment asserted that cargo's `--cap-lints allow` would keep generated and third-party code out
  of its reach. That is true of registry dependencies and wrong about `steamworks-sys`, which is a
  *workspace member*: the three `1.80.0` jobs went red on two `dead_code` errors raised inside the bindgen
  output ("struct `ISteamContentServer` is never constructed", "struct `obstack` is never constructed")
  while all three stable jobs were green -- unobservable on the dev box, which has no 1.80 toolchain
  installed and whose newer rustc does not emit those lints. The step now runs
  `cargo check -p steamworks --all-targets` with `-D warnings`, which keeps the assertion actually being
  made ("our sources compile without warnings") and stops pretending about generated code that a bindgen
  rerun would bring back -- the same judgement every other ruler here encodes by skipping `*bindings.rs`.
  The plain workspace-wide `check (all targets)` step stays unchanged, so nothing is silently ungated:
  what moved is only which crate's warnings are verdicts.
  Cost, stated plainly: one red CI run on a SHA I pushed, on exactly the axis I had written down as
  untestable locally. The change that would prevent the class rather than this instance is installing the
  pinned MSRV toolchain so MSRV-only behaviour is visible before a push; that touches this machine, so it
  is offered rather than done.

- **The last thirteen caller-triggerable `CString` panics are gone, and the ledger is now zero-tolerance**
  (`src/app.rs`, `src/input/actions.rs`, `src/matchmaking/queries.rs`, `src/networking_types/config.rs`,
  `src/remote_storage/api.rs`, `src/server/lifecycle.rs`, `src/user/mod.rs`,
  `src/user_stats/achievements.rs`). These were the sites with no failure channel, so the rule was
  already set: cut at the first nul, because Steam's parameter is a `const char *` and nothing past the
  nul could have reached it. Three exceptions are worth naming. `Server::init` *does* have a channel --
  it returns `Result<_, SteamAPIInitError>` -- so a version string that is not a C string is now
  reported with the existing `Generic` variant rather than registering a server under a shortened
  version, which adds no variant and breaks no match. `NetworkingConfigValue::new_string` carried the
  last `.expect(..)` in the crate, with a comment saying it was kept because changing the shape was worse
  than the panic; the shape still does not change, but the panic goes, because the cut is exactly what
  `SetString` would have read. And the three `ISteamInput` handle lookups were left as truncation rather
  than answering with a zero handle, because the vendored header never says what `GetActionSetHandle`
  returns for an unknown name -- inventing a not-found sentinel here would be a claim about Steam, not
  about this crate.
  What "empty ledger" means here is machine-checked, and three probes say so on the real tree: restoring
  one `unwrap` yields 1 record and `compare_baseline.py` exits 1; after the restore it is 0 records and
  rc=0; and a measurement whose `#scanned` marker is removed exits 2 -- refused, not passed -- which is
  the difference between "clean" and "did not look". `baseline_files.py` enforces the other half: it
  refuses a ledger listed as excused-empty the moment it holds bytes again (4094 in my first attempt,
  which is why the payment history lives in the ruler's docstring instead), so 13 ledgers now, 3 of them
  legitimately empty.
  Ledger: `cstring_unwraps` 13 -> **0**, the day's five payments read 56 -> 44 -> 31 -> 22 -> 13 -> 0;
  `module_sizes` and the rest re-recorded from the final tree; lib suite 200 passed / 15 ignored; rustc
  warnings still 0 under the new `-D warnings` check step.

- **A test that asserted nothing about its error type, and the gate that now catches its kind**
  (`src/networking_sockets/connection.rs`, `.github/workflows/ci.yml`). `a_refused_fetch_is_an_invalid_handle`
  was `matches!(name_answer(..), Err(InvalidHandle))` -- and because the test module never imported
  `InvalidHandle`, that pattern is not a type check but a *binding*: rustc read it as "bind whatever the
  error is to a variable named InvalidHandle", warned it was unused, and the assertion passed for any `Err`
  at all. The name had to be imported before `{ .. }` could even compile, which is how the vacuous form
  became visible. The repo's clippy step deliberately runs without `-D warnings` (style lints are many and
  low-value), and nothing else looked at rustc's diagnostics, so the only signal available was a warning
  nobody was required to read.
  The `check (all targets)` step now sets `RUSTFLAGS: -D warnings`, which costs no extra build (it *is*
  the build) and is measured to be empty: rustc emits 2 warnings across all targets before this fix and 0
  after. Dependency crates are safe from it -- cargo caps their lints -- so the flag only ever bites this
  workspace's code. Proved on the real tree rather than asserted: injecting `let probe_warning_that_nothing_gated_before = 5;`
  into a test made the step exit 101 with `error: unused variable`, and removing it returned rc=0. Clippy's
  own style categories stay undeclared-by-design, unchanged: measured 130 clippy warnings, top items
  `Result<_, ()>` style (23), needless `return` (9), bare `#[must_use]` (9).

- **Nine readers with a failure channel answer "no such thing" instead of crashing on a nul byte**
  (`src/friends/friend.rs`, `src/matchmaking/mod.rs`, `src/screenshots.rs`,
  `src/user_stats/{stats,stats_io}.rs`). `rich_presence`, `lobby_data` and `get_lobby_member_data`
  return `Option<String>`, the four stat get/set methods return `Result<_, ()>`,
  `get_achievement_display_attribute` returns `Result<&str, ()>`, and `path_to_absolute_cstring` already
  returned `Option<CString>` -- every one of them had a way to say no, and all of them used
  `CString::new(x).unwrap()` (or `.expect("Failed to create c_str from key parameter")`) instead. They
  go through `try_c_string` now, so a key or stat name containing a nul is refused rather than looked up
  under its own prefix: `None`/`Err(())` is the answer those types already mean for "not found", which
  makes this a narrower claim than inventing a new error. No signature moved. The asymmetry this leaves
  is deliberate and documented in both helpers: the same input in a *setter with no channel* is cut at
  the nul, because there the alternative to acting is panicking.
  Measured honesty about coverage: the rule is tested
  (`an_interior_nul_is_refused_rather_than_acted_on`, which a mutation making `try_c_string` infallible
  turned red by assertion message), the per-site choice is not -- the probe that replaced one call
  site's refusal with the truncating helper left the suite green (rc=0), since all nine need a running
  client, and that verdict carries over to these nine by the same argument.
  Ledger: `cstring_unwraps` 22 -> **13 across 8 files** -- the day's payments read 56 -> 44 -> 31 -> 22
  -> 13 -- `module_sizes` 29225 -> 29239, lib suite 200 passed / 15 ignored, `type_surface` 857
  unchanged. What remains is 13 sites in methods with no failure channel at all.

- **Nine setters with a failure channel now refuse instead of quietly acting on different text**
  (`src/friends/rich_presence.rs`, `src/input/core.rs`, `src/matchmaking/mod.rs`, `src/ugc/mod.rs`,
  `src/utils/text_input.rs`, new helper in `src/utils/strings.rs`). These are the sites where the
  previous two payments' answer would have been wrong: `set_rich_presence`, `set_lobby_data`,
  `delete_lobby_data`, `set_input_action_manifest_file_path`, `init_for_game_server` and
  `show_gamepad_text_input` all return `bool`, so a string containing a nul can be *refused* -- and
  truncating one would set the key `a` when the caller named `a\0b`, load a different manifest, or open
  a different workshop folder, then report success. `try_c_string` is the refusing sibling of
  `c_string_prefix`, and both docs state the split: no channel -> cut at the nul; a channel -> say no.
  `set_rich_presence`'s `/// # Panics` section became a sentence about returning `false`.
  `show_gamepad_text_input` checks its strings **before** `register_callback`, because a refusal after
  registering would leave a handler the caller never asked for.
  Mutation replay: making the rule never fail (`Some(..)`) turned
  `an_interior_nul_is_refused_rather_than_acted_on` red by assertion message. A second probe swapped one
  call site's refusal for the truncating helper -- semantically the wrong choice, exactly as described
  above -- and the suite stayed green (rc=0), because reaching any of these needs a running client. So
  the rule is locked and the per-site *choice* of rule is not; recorded rather than papered over. Two
  tests cover the helper; lib suite 200 passed, 15 ignored.
  Ledger: `cstring_unwraps` 31 -> **22 across 13 files** (44 -> 31 -> 22 in three payments the same
  day), `module_sizes` 29139 -> 29225, `type_surface` 857 unchanged, `header_citations` unchanged at 179
  spans with all three debt columns 0.

- **Thirteen more fire-and-forget setters stopped panicking on a caller's nul byte**
  (`src/friends/friend.rs`, `src/friends/overlay.rs`, `src/matchmaking/mod.rs`, `src/timeline.rs`,
  `src/user_stats/leaderboard.rs`, `scripts/ci/api_coverage.py`). `invite_user_to_game`, the four
  `activate_game_overlay*` / invite-dialog methods, `set_lobby_member_data` (key and value),
  `set_timeline_state_description`, `add_timeline_event` (icon, title, description) and
  `find_leaderboard` / `find_or_create_leaderboard` all took `CString::new(x).unwrap()` on a
  caller-supplied string and return nothing but a callback registration, so the panic had no exit. They
  share `c_string_prefix` now. `activate_invite_dialog_connect_string` carried a
  `/// # Panics` / "Panics if the `connect` str contains a null byte." section -- a doc that promises a
  panic is a claim about the public contract, so it was rewritten rather than left to rot; two such
  sections remain live in `src/friends/rich_presence.rs` and `src/matchmaking/filters.rs`, which are
  still-unpaid debt rows, not stale docs.
  Paying them exposed a defect in the tooling rather than the code, and it is the kind that quietly
  ruins a report: `cstring_unwraps.py` skips test modules by importing
  `api_coverage.strip_test_modules`, and that function **dropped** lines. Line numbers were then counted
  on text that was shorter than the file, so every `path:line` it printed was wrong in exactly the files
  that have a `#[cfg(test)]` module -- it pointed me at line 280 of `screenshots.rs` as a production setter
  when the real site was a test, and that misdirection was the only reason I found it. It now blanks
  those lines instead of dropping them (`api_coverage`'s own numbers did not move: 66 entries, total
  1557, baseline holds), and the debt register re-measured to **31 sites across 17 files** from 44/20.
  Importing that helper rather than copying it is what the canary suite caught next: `fixture()` copies
  only the ruler under test into a temp tree, so the new sibling import made the copy fail to start, and
  the canary reported "too little scanned is refused rather than passed" as a failure -- a crash wearing
  a verdict's clothes. The fixture now carries `api_coverage.py` alongside, and 73 assertions pass.
  Gate probe on the real tree, with the mtime discipline the previous unit established: restoring
  `CString::new(description).unwrap()` at the range-event site made the ruler emit `src/timeline.rs 1`
  (32 sites) and `compare_baseline.py` exit 1; restoring the file gave 31 sites and rc=0, and the suite
  was green (198 passed). Ledger: `cstring_unwraps` 44 -> 31, `module_sizes` 29135 -> 29139,
  `type_surface` 857 unchanged.

- **Eleven setters panicked on a caller's nul byte, and they now share one rule**
  (`src/server/config.rs`, `src/networking_sockets/connection.rs`, new helper in
  `src/utils/strings.rs`). All ten `Server::set_*` string setters (`set_product`,
  `set_game_description`, `set_game_data`, `log_on`, `set_mod_dir`, `set_map_name`,
  `set_server_name`, `set_tags`, and `set_key_value`'s pair) plus `set_connection_name` were
  `CString::new(x).unwrap()` on an argument the caller chooses, and none of them returns anything --
  there is no channel to report a failure through, so `unwrap` was a crash with no exit. They now build
  the string with `c_string_prefix`, which cuts at the first NUL: every one of those parameters is a
  `const char *` in the headers, so the NUL already ended the value Steam would read and nothing
  observable is lost. Where a setter *does* have a channel the answer is `Err(InvalidParameter)`
  (`cstr()` in `src/http.rs` and `src/html_surface/mod.rs`), and the helper's doc states that split so
  the next one is not guessed. `debug_string_prefix` in `connection.rs` now delegates to the same helper
  instead of keeping its own copy of the rule.
  Mutation replay on the rule: dropping the cut (`Q1`) and taking the last piece instead of the first
  (`Q2`) each turned tests red by assertion message -- in both files' modules at once, since they now
  share one implementation. On the *gate*: putting `CString::new(product).unwrap()` back into
  `set_product` made `cstring_unwraps.py` emit `src/server/config.rs 1` and `compare_baseline.py` exit 1
  against the freshly re-recorded ledger ("1 of 22 entries disagree"), which is the proof the register
  has teeth instead of being a list. Three tests cover the helper.
  A methodology catch, because it nearly produced a false claim: the probe driver restored each file with
  `shutil.copy2`, which **preserves the source's mtime**, so after the last restore `cargo test` reused
  the binary built from the mutated source and the run's "final check" reported two assertion failures
  that the tree did not contain. Touching the restored files and re-running turned that green (rc=0, 198
  passed); the Q1/Q2 numbers above are from that second, honest run.
  Ledger: `cstring_unwraps` 56 -> 45 across 21 files (both paid files leave the register),
  `module_sizes` 29087 -> 29135, `type_surface` 857 and `api_coverage` 1557 unchanged -- no public
  signature moved. The ruler's denominator reads 60 fallible conversions now (11 removed, plus 5 that
  this unit's own test literals contributed), of which 15 are provably-safe literals.

- **Seven public wrappers could outlive `SteamAPI_Shutdown`, because none of them held the client**
  (`src/html_surface/browser.rs`, `src/html_surface/mod.rs`, `src/http.rs`,
  `src/inventory/{mod,definitions,result}.rs`, `src/matchmaking_servers.rs`,
  `src/remote_storage/{api,files}.rs`, new `scripts/ci/handle_owners.py`). `Inner` owns the `Manager`
  and `Manager::drop` is what calls `SteamAPI_Shutdown()`, so a wrapper that carries a
  `*mut sys::ISteam*` and no `Arc<Inner>` is safe code holding a pointer to an interface whose client
  may already be gone -- and four of these seven release their Steam handle in `Drop`, so the very
  cleanup is one of the calls that can land after shutdown. Measured before the change: 169 `pub
  struct` shapes in `src/`, 44 of them holding a raw interface pointer, **7** without an owner
  (`Browser`, `CookieContainer`, `HttpResponse`, `InventoryResult`, `InventoryUpdateHandle`,
  `ServerListRequest`, `FileWriteBatch`). Each now owns an `Arc<Inner>` declared *last*, so it is
  released last and the wrapper's own `Drop` body still runs against a live client. No public signature
  changed; `ServerListRequest` holds `Option<Arc<Inner>>` because its callback object -- and with it the
  request -- is built by the caller before any interface is in hand, and the issuing method fills it in
  where it already fills in `mms`, `real` and `h_req`.
  Two consequences worth naming, both found by the compiler rather than by reading. `HttpResponse` is
  built inside the `register_call_result` closure, and the closure must stay `Send`: `..response` used
  to make that a whole-`HttpResponse` capture, and once the struct owns an `Arc` the functional update
  becomes a partial move *and* a separate capture of the raw `*mut sys::ISteamHTTP`, which is not
  `Send`. The closure now fills in `status` and moves the whole response out. And the `Browser`
  placeholder can no longer be `mem::forget`-ed, because forgetting it would strand the `Arc<Inner>` it
  now carries -- which only works because `removes_on_drop` refuses handle 0, the guard added in the
  entry below.
  The gate is the second half. `handle_owners.py` counts `pub struct` bodies that hold a raw interface
  pointer without `Arc<Inner>`, one per wrapper, keyed by file, and its ledger target is 0 --
  `baselines/handle_owners.txt` is empty and excused as such, with the measurement proving it scanned
  (`#scanned 127`). It is wired into the `quality` job (19 -> 20 steps, 17 named) and the local replay
  (24 -> 25 verdicts), and registered in `baseline_files.py` (11 -> 12 ledgers). Its canary adds 7
  assertions (57 -> 64): a bare wrapper is an offender, one that owns the client is not, a non-`pub`
  struct does not pad the count, the comparator turns a record red, and a scan that looked at too
  little is refused with exit 2. The canary exists because the first version of the ruler was wrong in
  a way only a mutation can show: its comment stripper removed the `//` marker but kept the prose, so
  the owner field's own comment -- which says `Arc<Inner>` -- read as an owner, and **deleting the
  field from `Browser` on the real tree still reported 0 offenders**. Blanking whole comment lines fixes
  it, the same probe then reports `src/html_surface/browser.rs 1`, and the fixture carries that comment
  shape inside the body so the bug cannot come back. Ledger: `module_sizes` total 29047 -> 29087.
  And the pointer gate earned its keep again: adding seven lines to `src/matchmaking_servers.rs` pushed
  two spans `docs/SuperSteam-Design.md` cites for the `Ipv4Addr` reading off `m_unIP` out of place -- one
  landed on a blank line, which is how the gate reads a stale line number -- and both were re-measured
  to `:454-465` and `:214`.

- **Breaking: `connection_name()` was a panic with a signature, and it now returns the name**
  (`src/networking_sockets/connection.rs`). Its whole body was `unimplemented!()`, so the one name a
  caller could set with `set_connection_name` could never be read back -- and calling it from safe
  code aborted the caller's frame. It is `Result<String, InvalidHandle>` now, the shape its sibling
  `connection_user_data` already uses. `GetConnectionName` takes the buffer *and* its size from the
  caller ("char *pszName, int nMaxLen", `isteamnetworkingsockets.h:229`) and the vendored headers give
  no limit for the name itself -- the two comparable ceilings are for other strings,
  "k_cchSteamNetworkingMaxConnectionAppName = 32" (`steamnetworkingtypes.h:663`) and
  "k_cbMaxGameServerName = 64" (`matchmakingtypes.h:21`) -- so `CONNECTION_NAME_BYTES = 256` is stated
  as this crate's choice rather than dressed up as Steam's rule, and a longer name comes back cut with
  no signal that anything was lost. The decision half is one pure `name_answer(ok, buffer)`, which
  reads the buffer through the shared `fixed_string` rather than a fourth copy of the cut-at-nul rule.
  Mutation replay: refusing to report the invalid handle turned
  `a_refused_fetch_is_an_invalid_handle` red by assertion message, and emptying the buffer turned two
  tests red; a third probe that replaced the *call* inside `connection_name` with a constant string
  stayed green (rc=0) because reaching the FFI needs a live connection -- registered, not counted as
  coverage.
  Widening the gate is the second half of this unit: `no_panic_on_unknown_enum.py` judged
  `unreachable!()` alone, so the stub this file carried was invisible to it. Its pattern is now
  `unreachable! | unimplemented! | todo!` and the corpus is 0 across 127 scanned sources. That is only
  worth saying because the new canary `canary_panic_stub` puts a stubbed method and a `todo!()` into a
  fixture tree, shows both reach the comparator as records, and shows that a *comment* about an
  `unimplemented!()` which used to be there does not (52 -> 57 canary assertions). As a direct check,
  putting the old stub back made the ruler emit `src/networking_sockets/connection.rs 1` for it.
  Ledger effects: `api_coverage` `ISteamNetworkingSockets.called` 29 -> 30 (declared 913 unchanged,
  total 1556 -> 1557), `module_sizes` `connection.rs` 600 -> 680 and total 28933 -> 29047,
  `header_citations` 172 -> 179 spans with all three debt columns 0 -- and the ruler judged two of the
  new citations wrong on the first try (a quote that sits at line 228 while the span said 229, and
  four cited spans in a block carrying no phrase at all), paid back by quoting rather than by
  re-recording the debt. Three new tests; lib test count 192 -> 195.

- **Dropping a browser placeholder asked Steam to remove handle 0** (`src/html_surface/browser.rs`,
  `src/html_surface/mod.rs`). `create_browser` plants a `Browser { handle: 0 }` before Steam answers
  and forgets it on the failure path, so the one path that was never covered is the answer that does
  not arrive: the callback closure -- holding the placeholder -- is dropped with the callback map, and
  `Drop` called `RemoveBrowser` on a handle Steam never handed out. `RemoveBrowser` takes
  "HHTMLBrowser unBrowserHandle" (`isteamhtmlsurface.h:47`) and documents no value meaning "none", so
  0 is this crate's own sentinel and the sentinel now refuses to make the call: `removes_on_drop` is
  one pure predicate with two tests, and both flips of it (always remove, never remove) went red by
  assertion message. The `mem::forget` stays, and not for the reason its comment used to give:
  removing it changed *how* the closure captures -- `..placeholder` moves the fields, so the closure
  captured a bare `*mut ISteamHTMLSurface` and stopped being `Send`, which
  `register_call_result` requires. That is why the comment now says which of the two guards covers
  which path.

- **Six audit claims checked: three were real and are fixed above, two were not defects, and one is
  a gap that is measured but not yet closed.** The two retracted: (a) *`matchmaking_servers.rs` frees
  a vtable Steam may still call back into* -- it does not, because `release()` asks
  `IsRefreshing` first and returns `Err(ReleaseError::Refreshing)` while a query is live, so
  `free_serverlist` only ever runs when Steam says nothing is in flight; what survives the check is
  that forgetting to call `release()` leaks the three `Box` allocations (no `Drop`, and one is not
  possible: dropping the callback object is exactly what Steam forbids while a request is live) and
  that `unsafe impl Send for ServerListRequest` asserts across `Rc` refcounts, which is a threading
  contract to change on purpose rather than a bug to patch quietly. (b) *`html_surface` double
  release* -- no path in the crate removes the same handle twice; `browser_from_handle` and
  `remove_browser` are documented alternatives for one handle, and the reachable half of the worry was
  the handle-0 case, which is the entry above. The gap that is real and not yet fixed: seven public
  wrappers hold a raw `*mut sys::ISteam*` and no `Arc<Inner>` -- `Browser`, `CookieContainer`,
  `HttpResponse`, `InventoryResult`, `InventoryUpdateHandle`, `ServerListRequest`, `FileWriteBatch` --
  so `Inner`'s `Manager` can reach its `Drop` (which calls `SteamAPI_Shutdown`) while one of them is
  still alive and about to call through the pointer. Measured with a structural scan of every
  `pub struct` shape in `src/` (169 scanned, 44 hold a raw interface pointer, 7 of those carry no
  `Arc<Inner>`), not by eye. Closing it means giving each type the
  `Arc<Inner>` the poll group and the Fake UDP port already carry and then a ruler that keeps the
  count at zero, which is a unit of its own rather than a line in this one.

- **`SteamFileReader` sought to the wrong place, and Steam's own byte count could push it past the
  end** (`src/remote_storage/files.rs`, new `src/utils/counts.rs`, `src/user/voice.rs`,
  `src/user/buffers.rs`). `SeekFrom::End(o)` was computed as `size - 1 - o`: the sign of `o` inverted
  and one subtracted, so `End(0)` landed a byte short of EOF and `End(-4)` landed *past* the end at
  `size + 3`, while `Current(o)` far enough negative produced a negative number that cast to a
  `usize` of ~1.8e19. The reader's next `read` then evaluated `size - offset` on that -- a panic in a
  debug build, and in a release build a length of ~1.8e19 handed to `FileReadAsync`. Seek now answers
  through one pure `seek_target(pos, offset, size)` using checked arithmetic against the same two
  bounds, and the end itself is addressable (reading there returns zero bytes, which is what EOF
  means). The count path had the same shape of hole: `m_cubRead` was trusted as a `usize` and then
  passed to `FileReadAsyncComplete`, whose parameter list is "void *pvBuffer, uint32 cubToRead"
  (`isteamremotestorage.h:206`) -- the reported number is also the size of the copy Steam is about to
  make into the caller's buffer. It goes through `written_bytes`, which had been the voice capture's
  private rule for exactly that shape and is now shared from `src/utils/counts.rs` (4 call sites: 3
  in `voice.rs`, 1 in `buffers.rs`, this one). Four tests cover the arithmetic, and mutation replay
  on `seek_target` turned three separate rules red by assertion message -- upper bound removed
  (4 tests), `End`'s sign flipped back (1), negatives clamped to zero instead of refused (3). A
  fourth probe replaced the *clamp call* in `read()` with the old `m_cubRead as usize` and every test
  stayed green (rc=0): that path needs a real cloud file, so the rule is locked and the wiring is
  stated, not tested.
  Ledger effects: `module_sizes` total 28842 -> 28933 (`files.rs` +81, `utils/counts.rs` +17 new,
  `utils/mod.rs` +2, `voice.rs` -9 for the moved function), `package_inventory` 215 -> 216 (the new
  file ships), `header_citations` 168 -> 172 spans and 27 -> 29 files, all three debt columns 0.

- **`close()` panicked on a caller's nul byte, and its debug string was freed before Steam read
  it** (`src/networking_sockets/connection.rs`). The first half is `CString::new(x).unwrap()` on an
  argument the caller chooses: paste a log line with a nul in it and safe code crashes the frame
  closing the connection. The second half is quieter -- `match debug_string { Some(s) => s.as_ptr() }`
  moves the `CString` into the arm, drops it when the arm's value is produced, and hands the now
  dangling pointer to `CloseConnection`, whose parameter is "const char *pszDebug"
  (`isteamnetworkingsockets.h:191`). The owner now lives in a local that outlives the call, and the
  byte decision is one pure `debug_string_prefix`: cut at the first nul, which is where a C string
  ends anyway, so nothing Steam could have read is lost. Three tests cover it; the two mutations of
  the rule (no truncation, take the last piece instead of the first) both went red by assertion
  message, while a third probe that restores the old `CString::new(..).unwrap()` plus the dangling
  `match` left every test green (rc=0) -- reaching `close()` needs a live connection, so the dangling
  pointer is proven by reading the temporary's lifetime, not by a test this crate can run.

- **Every HTTP header value lost its last byte** (`src/http.rs`). `header()` sizes its buffer from
  `GetHTTPResponseHeaderSize`, which says only that it "returns the size of the header value if
  present" (`isteamhttp.h:75`) and says nothing about a terminator; `GetHTTPResponseHeaderValue`
  (`:82`) is the same. The wrapper assumed the reported size counts a nul and cut
  `buf.len() - 1` unconditionally, so under the other reading -- the size is the value -- every
  header came back one character short, and the same file's `body()` returns all of the bytes it was
  told, which is how the inconsistency shows. The rule that is true either way is now one pure
  function, `header_value_bytes`: stop at the first nul, keep everything when there is none. A header
  value cannot legally contain a nul, so nothing meant to be read is lost by cutting at one. Three
  tests; both mutations went red by assertion message (never cutting -> the terminator and all-nul
  cases, cutting a last byte when there is no nul -> the exact-size case, which also panics on an
  empty buffer in a debug build).

- **Four batched receives believed Steam's count in two different ways, and two of them not at

  all** (`src/networking_sockets/mod.rs`, `poll_group.rs`, `fake_udp.rs`, `connection.rs`,
  `src/networking_messages.rs`). `ReceiveMessagesOnConnection`, `ReceiveMessagesOnPollGroup`, the
  Fake UDP port's `ReceiveMessages` and `ReceiveMessagesOnChannel` all answer with a signed `int`,
  and the crate had four private answers to that: the poll group and the Fake port each carried their
  own clamp of the same rule (two functions, two test sets, worded differently),
  `receive_messages_internal` cut the negative to `InvalidHandle` but passed an over-large count
  straight to `set_len`, and `receive_messages_on_channel` cast the number through with no check at
  all -- so the documented `-1` became ~1.8e19 slots and `set_len` described a `Vec` longer than the
  `batch_size` pointers that were allocated. All four now call one `messages_received(count,
  offered)` whose doc block carries the two header sentences that license the cut ("If the connection
  handle is invalid, -1 is returned." at `isteamnetworkingsockets.h:321`; the poll group inherits the
  connection's guarantees at `:552-553`). Mutation replay on the shared rule: cutting the negative
  branch turned `the_invalid_handle_minus_one_reads_as_no_messages` red by assertion message, and
  cutting the upper bound turned `an_over_large_count_is_clamped_to_the_offered_slots` red the same
  way. A third probe replaced the *wiring* at the poll group with the old `returned as usize` and
  every test stayed green (rc=0), which is recorded rather than glossed: the four call sites need a
  live client, so what CI judges here is the rule, not the claim that each caller reaches it.
  Ledger effects are a relocation, not growth: `module_sizes` total is unchanged at 28742 (five rows
  moved: +54 in `networking_sockets/mod.rs`, -41 in `poll_group.rs`, -26 in `fake_udp.rs`, +7 in
  `connection.rs`, +6 in `networking_messages.rs`), `header_citations` went 166 -> 168 spans and 26
  -> 27 files with all three debt columns still 0, and the lib test count went 181 -> 180 because one
  duplicate assertion set was deleted when its function was.

- **`create_fake_udp_port` could hand out two owners of one object, so `destroy` was a
  use-after-free** (`src/networking_sockets/fake_udp.rs`, `src/lib.rs`). Steam's own words: "If you
  call this function multiple times and pass the same (nonnegative) fake port index, the same object
  will be returned, and this object is not reference counted" (`isteamnetworkingsockets.h:906-908`).
  The wrapper consumed itself for `destroy` and said in prose that two live wrappers would be a
  caller-side alias, but nothing stopped the second call from succeeding -- and `FakeUDPPort` is
  `Send + Sync`, so `create(2000)` twice, `destroy()` on one, and sending on the other is a free
  read of freed memory in safe code, possibly on another thread. Server indexes now have to be claimed:
  `claim_port` records a nonnegative index and refuses a second claim while a wrapper is alive, while
  ephemeral ports (`-1`) are never recorded because "a distinct object will be returned for each call"
  (`:910-911`) -- deduplicating those would withhold handles Steam documents as unique. The claim
  travels with the value and is released by `destroy` (before the object is destroyed, so no fresh
  wrapper can claim a dead index) and by `Drop` -- which still does not destroy anything, because an
  automatic destroy is the bug this type was written to avoid; without it, dropping a wrapper without
  `destroy` would leave the index un-wrappable forever. `create_fake_udp_port`'s signature and its
  `Option` return are unchanged; a second concurrent wrapper is now `None` instead of an alias.
  Three tests cover the claim (one wrapper per server index, ephemerals never deduplicated, release
  re-opens the index) and three mutations were replayed against them: no-dedupe, dedupe-everything,
  and a release that does nothing. **All three turned their own test red by assertion message, not by
  a crash**, which is the distinction this unit has started recording: a mutation that only fails
  because a fixture dereferences a null pointer proves nothing about the rule.
  Two things the gate caught while the change was being made, both mine: the new comments cited
  `isteamnetworkingsockets.h` twice without quoting it, which pushed the `~undeclared` column from 0
  to 2 -- paid back by quoting Steam's sentence rather than re-recording the growth; and a live pointer
  in `docs/SuperSteam-Design.md` turned out to have been wrong all along, aimed at the region of
  `lib.rs` holding `utils` while claiming to illustrate the `timeline` accessor. The
  7-line addition to `lib.rs` moved that stale span onto a blank line, the gate flagged it, and re-measuring found
  the accessor at the `timeline` method of `src/lib.rs` with the runtime `timeline.is_null()`
  check it was citing.
  A third thing, also mine and worse: this very entry then went red in CI for the commit that shipped it,
  because it narrated the old bad pointer with a literal `lib.rs:512-518` span and `CHANGELOG.md` is one of the ten
  documents the gate reads -- so the write-up of the fix re-introduced the defect it described. The local
  24-verdict replay had been run *before* that prose was written, which is the same ordering rule broken
  twice in one session: the replay must be the last content change, not the last step before it. Fixed by
  describing the old span without a machine-readable pointer; the sentence keeps the numbers for the reader.

- **Three reads of Steam's fixed-size text fields could over-read the array or abort the callback**
  (`src/user_stats/stat_callback.rs`, `src/networking_utils.rs`, new helper in
  `src/utils/strings.rs`). `UserAchievementStored` and `UserAchievementIconFetched` each did
  `CStr::from_ptr(cb.m_rgchAchievementName.as_ptr()).to_owned()` and then `into_string().unwrap()`, and
  `RelayNetworkStatus`'s conversion did `CStr::from_ptr(status.m_debugMsg.as_ptr()).to_str().expect(..)`.
  Those fields are arrays inside a struct Steam fills, not strings Steam promised to terminate or to
  encode: `char m_rgchAchievementName[k_cchStatNameMax];` is "name of the achievement"
  (`isteamuserstats.h:358`, `enum { k_cchStatNameMax = 128 };` at `:17`) and the debug line is
  `char m_debugMsg[ 256 ];` under "Non-localized English language status.  For diagnostic/debugging
  purposes only." (`isteamnetworkingutils.h:417-419`). Neither carries the out-string length annotation
  that `isteammatchmaking.h` uses for its own fields, so `from_ptr` walks to a terminator nobody
  promised -- off the end of the array and into the following fields -- while `unwrap`/`expect` turns
  "this game's achievement name is not UTF-8" into a panic *inside callback dispatch*, which the host
  application cannot catch. All three now go through one `fixed_string`: bounded by the array,
  lossy rather than fatal. Counting the shape before and after, the fatal conversions of this form in
  `src/` went 3 -> 0 (the two remaining grep hits are a commented-out line and this file's own
  documentation of the choice).
  The helper already existed as a private function in `src/parties.rs`; rather than a fourth copy, it
  moved to `src/utils/strings.rs` as `pub(crate)` and `parties.rs` re-exports it -- which is why
  `package_inventory` grew by one file, `steamworks/src/utils/strings.rs`: the published crate now
  ships it, and the ledger was re-recorded against that reason rather than regenerated blindly.
  Three tests travel with the helper: it stops at the first terminator, an entirely full array yields
  exactly the array (the case `from_ptr` gets wrong), and invalid UTF-8 comes back as replacement
  characters instead of a panic. Three mutations replayed -- terminator ignored, UTF-8 demanded, and a
  build-always-empty variant -- and each turned its own test red **by assertion message**, not by a
  crash. Gates after the move: 24/24 green, `module_sizes` 28584, and the citation ruler at **26 files /
  162 spans with all three debt columns 0** -- it caught one of my own new citations on the first pass
  (`isteamnetworkingutils.h:419,417`, which parses as one span and left the quoted comment outside it;
  now `:417-419`).

- **Rejecting a session request closed it twice** (`src/networking_messages.rs`). `SessionRequest` kept
  a flag named `accepted`, which only `accept()` set, so `reject()` -- which calls
  `CloseSessionWithUser` -- left the request looking unanswered and `Drop` closed the same session a
  second time. `CloseSessionWithUser` is "Call this when you're done talking to a user to immediately
  free up resources under-the-hood.  If the remote user tries to send data to you again, another
  SteamNetworkingMessagesSessionRequest_t callback will be posted." (`isteamnetworkingmessages.h:103-105`)
  and the header promises no idempotency, so the second close is not a no-op: a session re-established
  in between -- and sending implicitly accepts a pending one ("Calling SendMessageToUser() will
  implicitly accepts any pending session request to that user.", `:100`) -- is torn down by it.
  The flag is now `answered`, set by one function `mark_answered` that also reports whether Steam is
  still owed a close; `accept`, `reject_inner` and `Drop` all consult it, and `Drop` no longer restates
  the rule from a flag only one path set. Public signatures are unchanged (`accept`/`reject` behave the
  same apart from the missing second close).
  **What the tests pin, and the limit of that, measured rather than assumed.** Two tests assert the
  state machine directly -- the first answer owes exactly one close and no later answer does; an
  ignored request still owes exactly one, so the refactor cannot quietly stop rejecting what Steam
  expects to see closed. Mutation replay: removing the record (`answered = true` never happening) and
  making the guard always owe a close both turn those tests **red by assertion**. The third mutation --
  deleting the guard in `reject_inner`, the historical bug shape -- does **not** fail an assertion: the
  test binary dies with `0xC0000005` (`rc=3221225477`) because the now-unguarded close dereferences the
  null interface pointer the fixture carries. CI therefore notices that mutation loudly but judges
  nothing about it, and the honest statement is that the *wiring* ("does the closer consult the state?")
  has no CI-verifiable coverage: `CloseSessionWithUser` is a Steam call with no seam to count through,
  and this file carries no `#[ignore]`d client-dependent test either -- so the end-to-end behaviour
  (exactly one close observed by Steam) is asserted nowhere, in CI or locally. The correction that
  produces it is not in doubt -- the old code plainly reached the close twice -- but the guarantee is
  registered as unproven rather than dressed up as a lock.

- **`RemoteStorage::files` singled out -1 and cast every other negative into a capacity**
  (`src/remote_storage/api.rs`). `GetFileCount` is "virtual int32 GetFileCount() = 0;"
  (`isteamremotestorage.h:228`), a signed count; the loop guarded `count == -1` and then did
  `Vec::with_capacity(count as usize)`, so -2, `INT_MIN`, or any other negative answer became ~1.8e19
  elements and the allocation aborted the caller's thread -- inside a safe method, on a value the
  caller never supplied. The sibling enumeration in the same file already asked `count <= 0` of the
  identically signed `GetLocalFileChangeCount` (`:315`), so both calls now share one `file_count`
  guard and the asymmetry that let one side be safe and the other not cannot come back. Two tests,
  one per direction: every negative enumerates nothing, and a real count (including `i32::MAX`) passes
  through unrounded -- the second because a guard that quietly swallowed large counts would be a data
  bug dressed as a fix. Two mutations replayed (cast straight through, clamp everything) and both went
  red; file restored byte-identical from a `cp` backup.
  **What this does not close, stated rather than implied:** a *positive* absurd count still reaches
  `with_capacity`, because putting a ceiling on the number of cloud files would be inventing a policy
  Steam never stated (and the guard's own test insists large counts survive). The residual symptom of
  that case is an allocation failure, not out-of-bounds memory, and it needs the same value Steam would
  have to lie about. Also left alone deliberately: `GetFileNameAndSize` reports the size as `int32` and
  the wrapper widens it to `u64`, so a negative size would surface as ~1.8e19 bytes rather than a
  failure -- clamping it to zero would fabricate a file length, so it stays as an observation until
  there is a documented answer to give.

- **A downloaded leaderboard could publish uninitialised heap as player data**
  (`src/user_stats/leaderboard.rs`). Inside the `DownloadLeaderboardEntries` callback, three of Steam's
  own numbers were used as sizes without being checked, and this is the path that fills `Vec<i32>` for
  every caller that reads scores:
  - `Vec::with_capacity(len as usize)` on `m_cEntryCount`, which is "the number of entries downloaded"
    (`isteamuserstats.h:385`) as a plain `int`. A negative answer became ~1.8e19 and an absurd positive
    one a multi-gigabyte reservation -- either aborts, and it aborts *inside the callback Steam is
    dispatching*, where the caller cannot catch it.
  - `details.set_len(min(entry.m_cDetails as usize, capacity))` on a `Vec::with_capacity` buffer.
    `m_cDetails` is "number of int32 details available for this entry" (`:78`), a signed `int32` that the
    backend fills. Cast through, a negative value made `min` select the whole capacity, so `set_len`
    published slots nobody had ever written as leaderboard details: undefined behaviour plus adjacent
    heap disclosure. The buffer is now zero-filled and `truncate`d through a `details_len` helper that
    clamps to the buffer, so no value Steam reports can extend it past what was written.
  - `GetDownloadedLeaderboardEntry`'s `bool` return was discarded. It "Returns data about a single
    leaderboard entry" (`:219`) -- false means it wrote nothing, and the old code pushed the zeroed
    `LeaderboardEntry_t` anyway, so a failed read became a phantom entry with user 0 / rank 0 / score 0
    in the caller's list. Failed reads are now skipped.
  `max_details_len` is caller-supplied as a `usize` and was cast straight into the API's signed `int`
  parameter; it is clamped to `c_int::MAX` before the buffer is built, so the cast cannot hand Steam a
  negative capacity.
  Four tests pin the two pure guards, each from both sides -- what must be refused and what must still
  pass untouched -- and four mutations were replayed against them (cast straight through, and clamp
  harder than warranted, for each guard); all four went red, file restored byte-identical from a `cp`
  backup. **Registered rather than faked: the `continue` on a failed read lives in the FFI closure and
  needs a running client**, so CI cannot judge it; it follows the same header line as the guard beside it
  and is covered by the manual `#[ignore]`d leaderboard test at the bottom of the file.

- **Two unchecked conversions from Steam's own numbers into memory sizes**
  (`src/networking_sockets/poll_group.rs`, `src/user_stats/stats.rs`). Both are the same shape as the
  `message.rs` guards in the commit above -- a value Steam fills in is used as a length or a size with
  no sign or width check -- and both were found by the sandbox audit rather than by reading the diff.
  - `NetPollGroup::receive_messages` did `... ReceiveMessagesOnPollGroup(...) as usize` and
    `set_len(count)`. Steam's contract is that the count "Returns the number of messages returned into
    your array, up to nMaxMessages.  If the connection handle is invalid, -1 is returned"
    (`isteamnetworkingsockets.h:320-321`), and the poll-group entry point claims "the same guarantees
    are present as mentioned in ReceiveMessagesOnConnection" (`:552-553`). `create_poll_group` does not
    reject an invalid handle, so -1 is reachable, and `-1 as usize` handed `set_len` a longer `Vec`
    than any allocation -- UB at the moment the messages were drained. A `message_count` helper now
    reads the negative answer as zero and truncates a count above the capacity to it. Three tests, and
    both halves are pinned: the clamp and the fact that an honest count passes through untouched.
  - `AchievementHelper` sized its RGBA buffer with `(width * height * 4).try_into().unwrap()`, where
    `width`/`height` come from `ISteamUtils::GetImageSize`. The product is computed in `u32`, so an
    image of 32768 on a side overflowed it -- panic in debug, wrap in release -- and the allocation
    ended up smaller than the size passed to `GetImageRGBA`, which then wrote past it; the required
    size is spelled out as "the destination buffer size should be 4 * height * width * sizeof(char)"
    (`isteamutils.h:85`). `rgba_buffer_len` now does `checked_mul` twice and refuses anything that does
    not fit the signed parameter it has to pass, which is also the 2GB boundary
    `docs/defect-03-resize-deep-dive.md` measures on the callback path. Three tests cover the normal
    case, the product leaving `u32`, and a size that fits `u32` but not `i32`.
  Four mutation controls, all red: the wrapping product, the signed parameter ignored, the count cast
  straight through, and the capacity clamp removed. Files restored from `cp` backups and compared
  byte-for-byte.
  Two things this unit also turned up about its own documentation and the audit's claims, recorded here
  rather than quietly fixed:
  **a)** the earlier note that `NetConnection`'s `Drop` would panic on its `.expect` is **wrong** --
  `close()` sets `is_handled = true` at `connection.rs:205` before it hands the connection to Steam, so
  the branch is never reached. It stays as a defensive arm and is registered, not tested, per the rule
  about unreachable fallbacks.
  **b)** `FakeUDPPort::receive_messages` has held this exact clamp as `received_count`
  (`src/networking_sockets/fake_udp.rs:47`, tested at `:313-320`) all along, so `message_count` is a
  second implementation of one rule; consolidating them is queued. And two `CHANGELOG.md` coordinates
  that pointed at `poll_group.rs:67` were re-measured rather than left dangling -- the pointer register
  in `doc_pointers.py` exempts only pointers whose *file* is gone, so a dated line number in a file that
  still exists stays the author's problem, which is the right outcome.

- **Four defects a sandbox audit executed rather than inferred** (`src/networking_types/message.rs`,
  `identity.rs`, `config.rs` and the six call sites that pass options to Steam). Each was reproduced in
  a clone of the tree at `36fc248` before being fixed, and each fix was then replayed backwards -- the
  old expression put back, the specific test run, the file restored from a `cp` backup and compared
  byte-for-byte. **6 of 6 mutations judged red.**
  - `NetworkingMessage::data()` built a slice straight out of `m_pData`/`m_cbSize`. rustc's own
    precondition check aborts on it -- `slice::from_raw_parts requires the pointer to be aligned and
    non-null` -- and the state that triggers it is documented as normal:
    "If cbAllocateBuffer=0, then no buffer is allocated.  m_pData will be NULL, m_cbSize will be zero"
    (`isteamnetworkingutils.h:38-40`). This is a *safe* method, and the crate calls it on its own
    `allocate_message(0)` results. It now reads as empty for a null payload or a non-positive size.
  - `copy_data_into_buffer` asked `((*self.message).m_cbSize as usize) < data.len()`. `m_cbSize` is a
    signed `c_int`, so a negative value casts to ~1.8e19 and the capacity guard *inverts*: an 8-byte
    write into a 4-byte buffer returned `Ok(())` and wrote past the allocation (the sandbox run died
    with `STATUS_ACCESS_VIOLATION`). The capacity is now `usize::try_from`, so a negative record
    refuses the write, and a test pins both halves -- the refusal and the fact that a correct capacity
    still writes.
  - `send_flags()` was `from_bits(..).expect(..)` on a bitmask Steam fills. Measured panic:
    `send flags could not be converted to rust representation`. It is now `from_bits_truncate`, with a
    test that also requires the *named* bits to survive, because truncation that silently dropped known
    flags would be a worse bug than the panic it replaces. Same class as the `to_steam_result` fix
    above: the value is not the caller's.
  - `NetworkingIdentity`'s `GenericString`, `GenericBytes` and `UnknownType` arms in `debug_string` were
    `unimplemented!()`, and `impl Debug` calls it -- so formatting an identity that arrived from a peer
    panicked in the local process. Executed: `not implemented` at `identity.rs:127` and `:130`. All
    three now render (bounded scan of the inline array, `m_cbSize` clamped rather than cast, lossy
    UTF-8), and the header's own reason for `UnknownType` is forward compatibility
    (`steamnetworkingtypes.h:252-256`), so a crash there was the opposite of the intent.
  - `NetworkingConfigEntry::new_string` freed its `CString` on return while the struct kept the pointer
    -- `steamnetworkingtypes.h:1787`: "WARNING - Just saves your pointer.  Does NOT make a copy of the
    string", `:1756`: `const char *m_string; // Points to your '\0'-terminated buffer`. The entry now
    owns an `Arc<CString>` beside the struct, the six option-passing call sites borrow-and-copy
    (`as_raw`) so the owner is alive across the FFI call, and the public `From` conversion -- which
    cannot give the buffer back, since the C struct has no destructor -- takes one extra strong count:
    a deliberate bounded leak replacing a use-after-free. **Registered honestly: this last fix has no
    CI-runnable test**, because its constructor is a call into `libsteam_api64`; the repo's convention
    for that is `#[ignore = "requires a running Steam client"]`, so CI cannot judge it and I did not
    write a test that pretends otherwise.
  The gate chain caught one of *my own* new citations on the first replay
  (`steamnetworkingtypes.h:1785` for a sentence that sits at 1787) and put 17 new spans into the
  ledger, all judged: 130 -> **147 spans, 0 wrong, 0 unjudged, 0 undeclared**.

- **`shell_hatches.py` was counting vocabulary, not holes: 4 of its 9 registered hatches were
  comment prose** (`scripts/ci/shell_hatches.py`). It searched each workflow file's whole text for
  `|| true`, `2>/dev/null`, `--no-verify` &c. -- and this repository's workflows *write those
  literals out to explain why a step no longer uses them*. Measured on the files as they stood:
  `quality-extra.yml` counted 6 where the runner executes 3, `release.yml` counted 3 where it
  executes 2, so the ledger's "may only go down" was a rule about wording: rewording a comment moved
  the number, and a genuine new hatch could hide inside the sentence describing it. The counter now
  skips lines whose first non-space character is `#` and still counts a hatch that has a trailing
  comment, because that line is code the runner executes. The ledger is re-recorded 9 -> 5 with that
  reason in its header -- the shrink is the ruler getting honest, not four holes being repaired.
  Three canaries pin it (49 -> **52**), and the mutation was replayed rather than argued: restoring
  whole-file counting turns two of them red -- the all-prose fixture stops reporting an empty scan
  (exit 2 -> 0, i.e. the green-that-checks-nothing this gate exists to catch) and the one-hatch
  fixture reports 3 instead of 1 -- while the third assertion, that a trailing comment still counts,
  stays green under that mutation by design; it is the guard against the opposite over-stripping.
  The two `cargo package --no-verify` invocations that remain are the release path's own trade, and
  the failure class they could hide was checked rather than assumed: a packaged crate missing what
  it builds from is pinned against by `package_inventory`, which carries 63 `lib/steam` entries and
  all three headers the bundled bindgen translation unit includes.

- **A docs citation two lines off, and the measured reason the ruler stays out of `docs/`.**
  `defect-06-ugc-builder-deep-dive.md` cited `isteamugc.h:29` for
  `k_UGCUpdateHandleInvalid` being all-ones `uint64`; line 29 of that header is blank and the constant
  is at `:31`. Now corrected.
  Why the citation gate does not simply widen to the docs, measured: those files carry **73 numbered
  header citations** (42 in the short `h:910` form), and only RANGE and BLANK are decidable there --
  the docs quote Steam as Chinese translation, so the verbatim-phrase half cannot follow. Running just
  those two verdicts over the docs flags **3**, of which this one is the only defect: the other two are
  sentences *about the ruler*, in `SuperSteam-Design.md`'s record of this gate -- it quotes `h:910-911`
  as the example of the short form and quotes `isteammatchmaking.h:961` precisely to state that the
  header ends at 886. Narrating a wrong citation requires writing the wrong number, so a docs-side
  verdict would go red exactly where the prose is doing its careful work; the precision of such a gate
  would be 1 in 3, which is the third time this unit has been measured down instead of built. The
  convention stays what the docs already do: correct the number in place, keep the wrong one in the
  dated CHANGELOG record.

- **Four pointers into the generated bindings had rotted, and nothing judged them red**
  (`docs/defect-03-resize-deep-dive.md`, `docs/SuperSteam-Design.md`). `header_citations.py` judges
  citations into the SDK headers; `doc_pointers.py` judges that a documented *path* resolves. Neither
  looks at a line number inside `windows_bindings.rs` & friends -- and those move on every
  regeneration. Counting the whole repo with a broad pattern (not the narrow comment-block walk that
  first suggested "there is nothing here"): **10 pointers named a generated-layer line; 1 sits in
  `src/`, 9 in prose** -- and this entry adds three more, by quoting the numbers it replaced. Each was
  judged against the file it names. Four were wrong, and all four sat
  in the living docs: `:4662` and `:1750` both landed on a bare `}`, `linux_bindings.rs:20284` landed
  on `#[repr(C)]`, and `:19611` landed on a parameter of `SteamAPI_ManualDispatch_GetNextCallback` --
  eleven lines from the `ManualDispatch_Init` binding the sentence was pointing at. Repaired to the
  lines that actually hold the claimed item: `windows_bindings.rs:4658-4661` (`pub m_cubParam: uint32`),
  `:1745-1749` (`m_cubParam: c_int`), `linux_bindings.rs:20269-20273` (`cubCallback: c_int`) -- the three
  that make the 2GB/4GB asymmetry visible -- and `:19600` (`SteamAPI_ManualDispatch_Init`). Five spans
  are now written from the item's declaration through the field it quotes, adding
  `windows_bindings.rs:11770-11772` and `:11773-11774`; that widening exposed an off-by-one header
  pointer on the same line: the "lower latency / one callback at a time" sentence
  is at `isteaminput.h:873`, so the citation now reads `872-873`.
  **A gate for this family was measured and refused.** The tempting rule -- the identifier the block
  names must appear inside the cited span -- turned 3 of these red *before* the widening, for the wrong
  reason: a field claim names its enclosing item, which sits up to four lines above the field. That is
  the failure the header-side version of the same rule already measured (7 flags, 0 defects), so the
  drift stays a documented hazard rather than a gate that cries wolf -- but the widening makes the rule
  hold anyway: re-measured after it, **no live pointer fails it** -- 6 pass it on the line that carries
  the claim (`windows_bindings.rs:4658-4661`, `:1745-1749`, `linux_bindings.rs:20269-20273`,
  `:11770-11772`, `:11773-11774`, `:408`), and the 7th (`:19600`) names its symbol on the preceding
  line, so it was read by hand: that line holds `pub fn SteamAPI_ManualDispatch_Init();`. The only
  flag left is CHANGELOG:477, a dated record. The CHANGELOG's two mentions of
  `:17499-17501` are left as dated records of the pre-regeneration state, consistent with the 30
  pointers `doc_pointers.py` already registers as historical.
  Re-running the same check over the crate's *own* files (78 numbered `src/*.rs:NNN` pointers in the
  docs; 63 in range and non-blank, 15 naming a file that no longer exists -- all 15 already in that
  historical register) caught one pointer in the sentence just repaired, in the failure mode
  `doc_pointers.py` says it cannot see: `src/lib.rs:264` exists, is not blank, and does not call
  `SteamAPI_ManualDispatch_Init` -- the call is at `src/lib.rs:431` (and `src/server/lifecycle.rs:100`
  on the game-server side). Both now cited.

- **The citation gate could not see a sentence Steam had wrapped, and one real mis-citation hid behind
  that** (`scripts/ci/header_citations.py`, `src/user_stats/stats.rs`). The ruler joined header lines
  with their `//` markers still attached, so any phrase spanning two or more header lines was
  unmatchable -- the haystack read `... about widgets and gadgets // and this sentence keeps ...` while
  the prose quoted it clean. Measured here: of 93 quotes sitting in single-citation blocks, **25 matched
  their header only once the marker was stripped**. Two damages followed. Verbatim quotes were filed as
  paraphrase debt, inflating `~unjudged`; and every `WRONG-PLACE` verdict whose phrase crossed a line
  break was **suppressed**, which is how `src/user_stats/stats.rs` got away with citing
  `isteamuserstats.h:134` while quoting the sentence that lives at 135-137. That one surfaced the moment
  the joiner was fixed and is now cited at `135-137`.
  How it was found matters more than the fix: I added twelve verbatim quotes to anchor unverified
  citations and the debt column moved the *wrong way* (+5). A ruler that reports more "unverifiable"
  text after the text became more quotable is a broken observation layer, not a corpus that got worse --
  the same shape as the doc-pointer incident where the parser matched a path suffix inside a longer,
  correct path. Two cross-line assertions pin it (canaries reach **46** across this unit, from 43 at the
  previous commit): a phrase spanning header lines
  4-5 matches when cited `4-5`, and is red when cited `5` alone.
  **The bug had already been laundered into a style rule.** The hand-written tables in `fake_ip.rs` and
  `fake_udp.rs` require each phrase to sit inside a *single* header line, their own comment saying a
  phrase across a break "would fail for the wrong reason". That constraint was work around the defect,
  not a property of the problem. It still holds and the tables still pass, but it is no longer
  load-bearing, and the next person to read it should not conclude that cross-line quotes are forbidden.
- **`to_steam_result` no longer panics on a result code this crate has no name for.** It read
  `error => Err(error.try_into().unwrap())`, so any code outside `SteamError`'s mapped list aborted inside
  the *caller's* frame -- on a value Steam chose, not one the caller passed in wrong. Measured rather than
  reasoned: filling a zeroed `SteamNetworkingFakeIPResult_t` and decoding it aborted with
  ``called `Result::unwrap()` on an `Err` value: Unknown(0)``, because `k_EResultNone` is 0 and 0 is exactly
  what a status field holds when Steam never wrote the struct. The FakeIP read-back found it by stepping on
  it; it was not a pre-existing risk someone had noticed.
  An unnamed code now decodes to `SteamError::Generic` -- the coarsest true thing that can be said, and the
  variant Steam's own `k_EResultFail` already maps to (`src/error.rs:24-26`) -- while the exact number stays
  reachable through `SteamError::try_from(code) -> InvalidSteamError::Unknown(code)`. Measured against the
  generated layer, only two of the 130 `k_EResult*` constants have no variant: `k_EResultOK`, which is not
  an error, and `k_EResultNone`.
  **`k_EResultNone` stays an error on purpose, and that answers a question this file carried from upstream.**
  The line above the `Ok` arm was commented out by the original author as
  `// sys::EResult::k_EResultNone => Ok(()), TODO: I'm not sure if this is considered a success`
  (`git show 41cf3fb:src/lib.rs:99`). Resolving it to *no* is a behaviour decision, not a bug fix, so it is
  written down: a status nobody wrote must not read as an operation that worked.
  **The first fix was wrong and the existing tests are what caught it.** Written as
  `.map(|_| ()).map_err(|_| SteamError::Generic)`, it turned *named* failures into `Ok(())` -- the opposite
  bug, and a silent one. Two pre-existing tests went red immediately,
  `ugc::answers::answer_tests::answered_prefers_the_io_failure_over_steam_s_result` and
  `ugc::answers::answer_tests::dependency_answers_decode_both_ids_and_their_own_callback_numbers`; the
  shape became an explicit `match`, and those two names were re-measured by re-applying the wrong body and
  reading the failure list rather than recalled into this sentence. The two arms now have their own mutation
  controls (`the_four_fake_ip_refusals_keep_their_names` for the named-code arm,
  `an_unmapped_result_code_is_an_error_and_not_a_success` for the fallback), and the restored tree is
  byte-identical to the backup under `cmp`.


- **Eleven numeric and attribution claims in this changelog and the design doc were wrong, and are
  corrected in place.** A separate read-only review of the seven most recent commits re-measured every
  number those commits publish. It found: two witness tallies that had gone stale when a file was
  edited *after* the ledger was recorded (`src/user_stats.rs`'s is 805 / **1** lost / **28** added, not
  0 / 27 -- the lost line is the `UserStats` doc line that same entry describes rewriting; `src/server.rs`'s
  is 743 / 0 / **40** added with 19 header lines, not 38 with 17); a `Server` field-visibility claim
  with the right conclusion and the wrong reason (the fields are private to the root, and a *child* may
  read its parent's private fields -- `pub(crate)` was never involved; the reverse case really does
  fail, and `E0451` is what proved it in `html_surface`); `src/server/callbacks.rs` described as "five
  notifications" where the file holds four `pub struct` and four `impl_callback!` (the fifth item is the
  `DenyReason` enum); the `remote_storage` test module called "200-line" where `mod offline_tests` spans
  lines 48-173, i.e. 126; "nine `impl Friends` blocks" where `grep -rc '^impl Friends'` finds eight files
  with one block each; `src/networking_sockets/mod.rs` quoted at 259 lines where it is 260 after the
  FakeIP wiring line landed; `NetworkingSockets`' impl called 398 lines where 38..426 is 389; two
  subjects and two bullets calling a root "the last module root still carrying its own implementation"
  when `ugc/mod.rs` (598 lines, four impls), `matchmaking/mod.rs` (478, three) and `user/mod.rs` (376)
  all still held theirs; a design-doc heading that read "all single-file modules are now directories"
  while `ls src/*.rs | wc -l` measures 24 that are not (biggest: `input.rs` 725, `utils.rs` 710,
  `error.rs` 603); a `src/server/interfaces.rs` header that promised "one more site" for both `pub fn`
  and `debug_assert` when the `debug_assert` half counted the header sentence's own use of the word, so
  it now states only the reproducible `pub fn` figure (9 text / 8 live); and a `steamnetworkingfakeip.h`
  citation off by one line (the "cannot be sent" sentence is at 17-18, not 18-19).
  Two things came out of the pattern rather than the list. First, `scripts/ci/split_witness.py` is
  committed: it re-derives any split's old/new/lost/added multiset from git objects on both sides
  (`python scripts/ci/split_witness.py 295cfbd^ src/user_stats.rs 295cfbd src/user_stats stat_callback.rs
  stats.rs`), so "X lost / Y added" is now a claim anyone can re-check without rewriting a scratch
  script -- all six published witnesses reproduce, and every lost line is a widening or the doc rewrite
  named above. Second, `src/networking_sockets/fake_ip.rs` gained
  `every_header_citation_in_this_file_lands_on_the_sentence_it_quotes`, which reads
  `isteamnetworkingsockets.h` and asserts that each of the nine line spans its docs cite actually
  contains the quoted phrase -- the review had correctly pointed out that the FakeUDPPort constants are
  machine-checked but nothing checked this file's citations. The guard was disproved by mutation: moving
  one citation to lines 20-22 turned that test red with the offending span printed, and the file was
  restored byte for byte.

- **The last two `unreachable!()` in the hand-written sources are gone, and the panic-site ledger
  reaches the zero it was built to measure.** Both sat in the fallback arm of a `match` over a
  `#[non_exhaustive]` Steam enum -- `ERemoteStoragePublishedFileVisibility`, read back while a
  Workshop query result is assembled, and `EUGCContentDescriptorID`, read back from a preferences
  or descriptor query -- so a Steam newer than the vendored SDK could panic inside the caller's
  process. Neither arm is reachable with today's headers, because the bindings name every value
  those two enums can carry; no test can hit them, so none was written for them. The guard is the
  ledger itself: it now measures 0 sites across 51 scanned sources, and its comparator turns red on
  the first new site (an empty ledger is only accepted while the ruler still sees sources, which it
  checks separately). What those enums gained instead is a documented `Unknown` and a write path
  that refuses -- see the breaking entry under **Changed**.
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

- **Breaking: `ping_server` / `player_details` / `server_rules` return `SteamResult<ServerQuery>`
  instead of `()`.** They were already able to fail in two ways a caller could not see -- Steam returns a
  query handle, and it can return its "no query" marker -- and both were discarded with the handle, which
  is what left `CancelServerQuery` unwrappable. `ServerQuery` is opaque: the only thing to do with it is
  `cancel()`, and that is the operation the header asks for before a callback object goes away
  (`isteammatchmaking.h:514-515`). **Downstream**: a call written as a statement still compiles unchanged
  (the returned value is simply ignored); one that bound the result to `let ... : ()` needs updating. No
  call site inside this crate's `src/`, tests or examples did (`cargo check --locked --workspace
  --all-targets` rc=0, no warnings). A caller that ignores the handle outright now gets an
  `unused_must_use` warning, because `Result` is `#[must_use]` -- which is the point of returning one
  rather than the bare handle: dropping the query is a decision, not a default.

- **Breaking, and it is the whole class: no more `Result<T, ()>` in this crate** (six files:
  `src/user_stats/stats_io.rs`, `stats.rs`, `global_stats.rs`, `achievements.rs`,
  `src/matchmaking_servers.rs`, `src/matchmaking/mod.rs`). A `Result<T, ()>` says a call can fail and
  then refuses to say why: the caller cannot branch on a reason, cannot log one, and cannot map it
  without inventing the mapping itself. Measured before this change: **26 signature expressions** of
  that shape and **30 `Err(())` sites** -- and one of those 26 is the shared body of
  `gen_server_list_fn!`, so five generated list-request methods erase the reason through a single line.
  Every one now returns `SteamError`:
  `InvalidParameter` where Steam itself declined, where a name contains an interior nul, or where an
  index is out of range -- the same answer `src/server_stats.rs` and `user_reads.rs` already gave for
  that shape -- and `InvalidState` where the failure is about this wrapper's own object rather than a
  refusal: `ServerListRequest::released` (the handle is gone; there is nothing left to ask) and
  `achievement_count` (Steam's `GetNumAchievements` returns the failure value `0`, which cannot be
  distinguished from "this app has no achievements", as that function's own doc says). The three
  doctests that ended in `# Err(())` end in `# Ok(())` now, because their `?` must convert to
  `SteamError`.
  `join_lobby` was the worst case, so it got a type of its own instead of a substitution: its callback
  took `Result<LobbyId, ()>` and its check was `io_error || response != 1`, which collapsed "the answer
  never arrived" together with "Steam named a reason" -- and Steam *does* name one, in
  `m_EChatRoomEnterResponse`, which this crate already translates into `ChatRoomEnterResponse` for the
  chat path. It now takes `Result<LobbyId, JoinLobbyError>` with
  `Transport(SteamError)` and `Refused(ChatRoomEnterResponse)`, decided by `join_lobby_answer`, so a
  caller can tell a full lobby from a disconnected client. **Downstream**: anything that matched on
  `Err(())`, annotated a `Result<_, ()>`, or passed those to `?` needs updating; nothing in this
  crate's `src/`, tests, or examples did (`cargo check --locked --workspace --all-targets` is rc=0).
  Three mutations against the new decision went red on an assertion message (dropping the transport
  check, treating every reason as success, making the achievement count total). The fourth is recorded
  as what it is: rewriting `store_stats` to answer `Ok` even when Steam returned `false` **survived**,
  because that path needs a live client -- so the conversion is verified at the type level and at the
  decision level, and the FFI bodies remain registered as untested rather than counted as covered.

- **Breaking: the lobby's two number widths now match Steam's `int`, and the 250 abort is gone**
  (`src/matchmaking/queries.rs`, `src/matchmaking/filters.rs`, `src/matchmaking/mod.rs`). This closes
  the two decisions the lobby-control entry registered without acting on.
  `set_request_lobby_list_result_count_filter(count: u64)` takes `i32`, and `LobbyListFilter::count`
  with its `set_count` builder moves `Option<u64>` -> `Option<i32>`, because the call is declared
  "AddRequestLobbyListResultCountFilter( int cMaxResults )" (`isteammatchmaking.h:121`) and the wrapper
  reached it through `count as i32` -- so `4294967296` became `0`, a filter for no results that nobody
  asked for and that no test could see. `create_lobby` takes `max_members: i32` (Steam:
  `CreateLobby( ELobbyType eLobbyType, int cMaxMembers )`, `:138`) and the
  `assert!(max_members <= 250)` that used to run before the call is **removed**: 250 is nowhere in the
  vendored header (re-measured this round: no `250` in `isteammatchmaking.h`), the ceiling was the
  crate's own claim, and its failure mode was an abort in the caller's frame -- while
  `LobbyCreated_t`'s `m_eResult`, already mapped through `to_steam_result` in the same function, is the
  channel Steam uses to say no. An over-large limit now arrives as `Err` in the caller's callback like
  any other refusal; a game that wants a local bound checks one before calling.
  There is deliberately no test for either: the fix is at the type level, so an out-of-`i32` value no
  longer has a type to arrive in, and a test would only assert that Rust's integers are Rust's
  integers. What the change cannot be tested for is also registered: whether Steam really refuses an
  over-large `cMaxMembers` needs a live client, so the *answer* is unverified while the *silent
  narrowing* is gone. Verified instead: `cargo check --locked --workspace --all-targets` with the
  futures feature is rc=0, i.e. no call site in `src/`, the tests, or `examples/` needed a compatibility
  shim -- the breakage is for downstream callers only, which is why this is a `Changed` entry and not a
  quiet edit. `module_sizes` and `header_citations` re-recorded (329 spans, 0/0/0, across two more
  citing files); the same backticks-instead-of-double-quotes mistake the ruler has caught before was
  caught again here and fixed by quoting Steam's declaration properly.

- **Two licence tables became one, and the public-address read became arm-selective**
  (`src/user/answers.rs`, `src/server/status.rs`). Reviewing the entry above turned up two things
  worth fixing before either name ships. First, `server::UserLicense` duplicated
  `user::UserHasLicense`: both interfaces ask the same question, so both now answer it through
  `license_answer` and return the one type, and the table's tests moved with it
  (`every_licence_answer_steam_declares_has_a_name` plus its premise pin). Its fallback variant is
  renamed `Unknown(i32)` -> `Unrecognized(i32)`, the name the rest of the crate gives an
  out-of-table number, and `UserHasLicense` and `ServerAddress` are now `#[non_exhaustive]` for the
  same reason `LobbyType` is: a value Steam adds later should not become this wrapper's breaking
  change. Second, `public_address` copied **both** union arms before consulting the tag. The arms
  are not the same size -- `uint32 m_unIPv4`, "Host order", against `uint8 m_rgubIPv6[16]`,
  "Network order! Same as inaddr_in6." (`steamtypes.h:115-116`) -- so every IPv4 answer read twelve
  bytes nobody had written. The reported address was still the one the tag selects, which is why no
  test caught it: this was a bad read, not a wrong value. It now reads only the selected arm. That
  selection needs a live game server, so it is registered as uncovered rather than tested; the pure
  decode keeps both of its tests. Four claims in documentation were re-anchored in the same pass --
  the arm widths in `src/networking_types/addresses.rs` now cite `steamnetworkingtypes.h:227-238`,
  the favorites entry says "the six values Steam fills in", `FakeIpType::Invalid` no longer asserts
  reachability it has not shown, and two fixture-length messages state what their tables enumerate.
  Ledgers: 287 header-citation spans with all three debt columns at zero, `module_sizes` re-recorded
  -- `src/server/status.rs` 450 -> 377 and `src/user/answers.rs` 66 -> 130, which is the table
  moving rather than growing -- and the lib test floor holding at the measured 271.

- **Breaking, by the same rule as the two state enums: `LobbyType` is `#[non_exhaustive]` and gained
  `PrivateUnique`** (`src/matchmaking/mod.rs`). Steam declares five lobby types
  (`isteammatchmaking.h:18-28`) and this enum named four; the fifth, `k_ELobbyTypePrivateUnique`
  ("private, unique and does not delete when empty", `:26`, with the note that one "can only create
  from webapi", `:27`), could not be asked for at all -- which is also why `SetLobbyType` had gone
  unwrapped. A downstream `match` that lists the four old variants without a wildcard stops compiling;
  that is the point. Nothing in this repository does one (measured: `LobbyType` appears in `src/` and
  in the two examples only as a value passed to `create_lobby`, never as a scrutinee), and each variant
  now carries the header sentence that names it.

- **`NetworkingAvailabilityError` is `#[non_exhaustive]` and gained `Unrecognized(i32)`**
  (`src/networking_types/state.rs`). Same category as the connection-state change in `836222f`: a match
  that lists the five named answers without a wildcard stops compiling, and that is the point -- the
  alternative was to keep answering an unrecognised number by aborting. Nothing in this repository matches
  exhaustively on it (measured: the type appears only as a return type in
  `src/networking_sockets/auth.rs`), and the variant is the one thing the old `panic!` could not express:
  the number itself.

- **Breaking, by the maintainer's pick: `NetworkingConnectionState` is now `#[non_exhaustive]` with four
  more variants, and its outgoing conversion is no longer infallible**
  (`src/networking_types/state.rs`). `FinWait`, `Linger`, `Dead` and `Unknown(i32)` were added, so a
  downstream `match` that listed the six old variants without a wildcard stops compiling -- that is the
  point of the change, since the alternative was a callback that aborted on a state Steam's own header
  declares (see the entry under **Fixed**). `impl From<NetworkingConnectionState> for
  sys::ESteamNetworkingConnectionState` became `impl TryFrom<...> with Error = InvalidConnectionState`: an
  `Unknown` number has no constant in this SDK's header to carry it back, so the direction must be able to
  refuse. Nothing in this repository used it -- the bindings mention the enum only as struct fields, never
  as a call parameter -- and `state()`'s signature still names `InvalidConnectionState`, which is why that
  error type is kept rather than removed.

- **`chat-example` no longer copies the lobby id to the clipboard, and six advisories left with it**
  (`examples/chat-example/Cargo.toml`, `examples/chat-example/src/main.rs`, its `README.md`, and
  `Cargo.lock`). GitHub reported 6
  open Dependabot advisories on `master` (3 critical, 1 high, 1 moderate, 1 low) and all six are the
  same chain: `clipboard 0.5.0` -> `x11-clipboard 0.3.3` -> `xcb 0.8.2`, whose fixes are published as
  1.0.0 and 1.6.0 -- outside `clipboard`'s own semver bound, so no in-range upgrade exists. The
  dependency was one call in one example (`ClipboardProvider::new().unwrap()` + `set_contents` after
  a lobby was created), and the id was already printed on the line above, so the example now prints
  it and the `clipboard` subtree leaves the lock entirely: 77 lines, 8 packages (`clipboard`,
  `clipboard-win`, `x11-clipboard`, `xcb`, `block`, `objc`, `objc-foundation`, `objc_id`). `cargo
  check -p chat-example` passes after the change (rc=0). The library never depended on any of them --
  this only ever reached a Linux user building the example -- but the advisories sit on the default
  branch and were the only six open.

- **Breaking: `QueryHandle`'s builder is fallible too, and the whole file lost its panic surface.**
  All 29 `-> Self` setters now return `Result<Self, SteamError>` (30 do today, counting
  `add_required_tag_group`, which was already fallible), and `fetch`, `fetch_total` and `fetch_ids`
  keep their callback signatures while answering failures **through the callback** -- they have no
  other channel. Measured before/after in `src/ugc/query.rs`: **78 `unwrap()` → 0, 31
  `debug_assert!` → 0, 0 `expect`**. Two of those failure classes were not even panics in
  production: `debug_assert!(ok)` compiles away in release, so Steam refusing a tag, a language or
  a search string was answered with silence and the query went out unconfigured. A handle that
  `fetch` already consumed now yields `SteamError::InvalidState` instead of an `unwrap()`, kept
  distinct from `InvalidParameter` on purpose -- "there is no request left to configure" and "Steam
  said no to this one" are different bugs and the caller cannot fix either one if they arrive as
  the same message.
  **This is wider than the draft in `docs/pr-bodies/6-ugc-builder-result.md`, which named 9 string
  setters.** Converting only those nine would have left twenty setters in the same file still
  doing `self.handle.unwrap()` -- the half-converted shape this changelog criticised three entries
  ago for the update builder, so the file is uniform instead. The other twenty are one-line bool/int
  setters whose only new failure is the consumed handle, so the migration for a caller is the same
  `?` in the same chain. Say the word and I will cut it back to nine.
  Decisiveness: the handle decision was extracted into `taken_from` so it is testable without a
  Steam client, and mutation-checked twice -- answering a consumed handle with a fabricated handle
  0, and reporting it as `InvalidParameter` -- each turning the test red, file restored
  byte-for-byte. The 29 setters themselves are one helper plus one FFI call and got no test: that
  is the thin-shell case, and a `check(ok)?` restated as an assertion would look like coverage
  while killing nothing. `examples/` and the rest of `src/` contain no query-builder chains
  (measured: one doc-link mention), so the break is downstream-only.
- Ledgers re-recorded at measured values: lib test floor 137 -> 138, and `src/ugc/query.rs` 487 ->
  462 lines -- it shrank, because the converted bodies are shorter than the `let ok = ...;
  debug_assert!(ok); self` they replaced. Coverage is unchanged at 621 of 913 and `type_surface`
  holds at 80 keys: no FFI call and no method name was added or removed.

- **Breaking: `UpdateHandle`'s ten remaining setters return `Result<Self, SteamError>`, and the
  builder no longer panics anywhere.** `title`, `description`, `language`, `preview_path`,
  `content_path`, `metadata`, `tags`, `add_key_value_tag`, `remove_key_value_tag` and
  `remove_all_key_value_tags` join the eleven that already answered with a `Result`, so the whole
  update builder is now uniformly fallible. Measured, this removed **21 panic expressions** from
  `src/ugc/update.rs` -- 10 `assert!` on Steam's own `false` answer and 11 `unwrap()` (9
  `CString::new(..)` on a string with an embedded NUL, 2 `canonicalize()` on a path that does not
  exist). `grep` for `assert!`/`unwrap`/`expect` in that file now returns three hits, all inside
  doc comments. Migration: a chain that used to be
  `.title(t).description(d).submit(..)` becomes `.title(t)?.description(d)?.submit(..)`, or a
  refusal is handled where the chain is built; `examples/workshop` shows both shapes by building
  the chain in a fallible function and printing the preparation error.
  Error shapes are Steam's own vocabulary rather than invented ones: an unencodable string is
  `InvalidParameter`; a path that does not exist is `FileNotFound` while any other OS failure
  during resolution is `IOFailure` (they are different operations for a caller -- create the file
  versus fix permissions -- and both used to be the same panic); Steam answering `false` is
  `InvalidParameter`, the value it already reported rather than a crash.
  `#[must_use]` is dropped from the converted methods: `Result` is already `#[must_use]`, and
  keeping both would say nothing.
- **Breaking (internal signature, no public surface): `SteamParamStringArray::new` returns
  `Option`.** It is `pub(crate)`, so this is not a downstream-visible change, but it fixed a
  second defect on the same line: the old `.expect(..)` panicked in the caller's process *and*
  the entries already converted with `into_raw()` had no owner left to free them, so a bad tag in
  position 5 leaked the first four. It now hands back `None` and frees what it took. The freeing
  itself is not observable from safe Rust, so no assertion pretends to cover it -- the
  no-longer-panicking half is tested.
- `SafeUgcUpdate`'s passthroughs map the inner error into `SafeUgcError::Refused(err)` (the
  variant `467ab19` added), which carries the actual `SteamError` rather than flattening it. Its own pre-checks still fire
  first (a NUL in a title is `InteriorNul`, an unresolvable path is `InvalidPath`), so the carried
  error means "Steam said no", and the variant keeps that claim checkable.

- **`PublishedFileVisibility` and `UGCContentDescriptorID` no longer pretend every answer fits.**
  Both gained an `Unknown` variant, and both lost the infallible `Into<sys::E…>` that used to send
  them back: writing one is now `TryFrom<ours> for sys::E…` with `Error = SteamError`, and
  `Unknown` refuses with `InvalidParameter`. Migration: `v.into()` becomes `sys::…::try_from(v)?`
  (or `v.try_into()?`), and the three `UpdateHandle` setters that take these values --
  `visibility`, `add_content_descriptor`, `remove_content_descriptor` -- now return
  `Result<Self, SteamError>`, so a chain needs `?` at those steps. `SafeUgcUpdate::visibility`
  keeps its two failures apart rather than conflating them: a value with no Steam code is
  `SafeUgcError::UnencodableEnum`, a refusal by Steam is the new `SafeUgcError::Refused`.
  Why this is not a repeat of the `FileType` fix: `FileType::Unknown` refuses *by deferral*,
  handing Steam its own `k_EWorkshopFileTypeMax`. Measured against the vendored bindings, neither
  of these two enums declares such a value -- `ERemoteStoragePublishedFileVisibility` is exactly
  0..3, `EUGCContentDescriptorID` exactly 1..5 -- and a `#[non_exhaustive]` enum gives no legal way
  to build an out-of-range discriminant. So the only alternatives were inventing a code or
  refusing: an invented visibility would publish an item the caller meant to keep private, and an
  invented descriptor would tag an item with a mature-content label nobody asked for. Those three
  setters also stop `assert!`-panicking when Steam itself refuses; they return the same
  `InvalidParameter`, which is the first slice of the 22-method builder change drafted in
  `docs/pr-bodies/6-ugc-builder-result.md` -- the other ten steps still panic as before.
  `UGCContentDescriptorID` additionally derives `Clone, Copy, Debug, PartialEq, Eq`: additive, and
  what lets a test compare a descriptor read out of a query with one written back.
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
