use super::*;
use crate::networking_messages::*;
use crate::networking_types::*;
use crate::networking_utils::*;
use crate::screenshots::*;

use crate::sys;

use std::sync::{Arc, Weak};

/// A sum type over the callback results the stream can deliver.
///
/// Not every callback the crate defines has a variant here yet: the remaining
/// families are listed, by module, in the `CALLBACKS_NOT_IN_STREAM` debt list in
/// this file's test module, and `every_callback_is_streamable_or_listed_as_debt`
/// keeps that list from growing. Every callback is reachable through
/// `register_callback` regardless.
#[derive(Debug)]
pub enum CallbackResult {
    AddAppDependencyResult(AddAppDependencyResult),
    AddUGCDependencyResult(AddUGCDependencyResult),
    AuthSessionTicketResponse(AuthSessionTicketResponse),
    ClanChatMessageReceived(ClanChatMessageReceived),
    DownloadItemResult(DownloadItemResult),
    FloatingGamepadTextInputDismissed(FloatingGamepadTextInputDismissed),
    FriendChatMessageReceived(FriendChatMessageReceived),
    FriendRichPresenceUpdate(FriendRichPresenceUpdate),
    GameLobbyJoinRequested(GameLobbyJoinRequested),
    GameOverlayActivated(GameOverlayActivated),
    GamepadTextInputDismissed(GamepadTextInputDismissed),
    GameRichPresenceJoinRequested(GameRichPresenceJoinRequested),
    GetAppDependenciesResult(GetAppDependenciesResult),
    GetUserItemVoteResult(GetUserItemVoteResult),
    GlobalStatsReceived(GlobalStatsReceived),
    LobbyChatMsg(LobbyChatMsg),
    LobbyChatUpdate(LobbyChatUpdate),
    LobbyCreated(LobbyCreated),
    LobbyDataUpdate(LobbyDataUpdate),
    LobbyEnter(LobbyEnter),
    MicroTxnAuthorizationResponse(MicroTxnAuthorizationResponse),
    NetConnectionStatusChanged(NetConnectionStatusChanged),
    NetworkingMessagesSessionFailed(NetworkingMessagesSessionFailed),
    NetworkingMessagesSessionRequest(NetworkingMessagesSessionRequest),
    P2PSessionConnectFail(P2PSessionConnectFail),
    P2PSessionRequest(P2PSessionRequest),
    PersonaStateChange(PersonaStateChange),
    RelayNetworkStatusCallback(RelayNetworkStatusCallback),
    RemotePlayConnected(RemotePlayConnected),
    RemotePlayDisconnected(RemotePlayDisconnected),
    RemoveAppDependencyResult(RemoveAppDependencyResult),
    RemoveUGCDependencyResult(RemoveUGCDependencyResult),
    ScreenshotRequested(ScreenshotRequested),
    ScreenshotReady(ScreenshotReady),
    SetUserItemVoteResult(SetUserItemVoteResult),
    SteamServerConnectFailure(SteamServerConnectFailure),
    SteamServersConnected(SteamServersConnected),
    SteamServersDisconnected(SteamServersDisconnected),
    TimelineEventRecordingExists(TimelineEventRecordingExists),
    TimelineGamePhaseRecordingExists(TimelineGamePhaseRecordingExists),
    TicketForWebApiResponse(TicketForWebApiResponse),
    UnreadChatMessagesChanged(UnreadChatMessagesChanged),
    UserAchievementStored(UserAchievementStored),
    UserAchievementIconFetched(UserAchievementIconFetched),
    UserFavoriteItemsListChanged(UserFavoriteItemsListChanged),
    UserStatsReceived(UserStatsReceived),
    UserStatsStored(UserStatsStored),
    ValidateAuthTicketResponse(ValidateAuthTicketResponse),
    GSClientApprove(GSClientApprove),
    GSClientDeny(GSClientDeny),
    GSClientKick(GSClientKick),
    GSClientGroupStatus(GSClientGroupStatus),
    NewUrlLaunchParameters(NewUrlLaunchParameters),
    WorkshopEULAStatus(WorkshopEULAStatus),
}

impl CallbackResult {
    pub unsafe fn from_raw(discriminator: i32, data: *mut c_void) -> Option<Self> {
        Some(match discriminator {
            NetConnectionStatusChanged::ID => {
                Self::NetConnectionStatusChanged(NetConnectionStatusChanged::from_raw(data))
            }
            AuthSessionTicketResponse::ID => {
                Self::AuthSessionTicketResponse(AuthSessionTicketResponse::from_raw(data))
            }
            ClanChatMessageReceived::ID => {
                Self::ClanChatMessageReceived(ClanChatMessageReceived::from_raw(data))
            }
            DownloadItemResult::ID => Self::DownloadItemResult(DownloadItemResult::from_raw(data)),
            FloatingGamepadTextInputDismissed::ID => Self::FloatingGamepadTextInputDismissed(
                FloatingGamepadTextInputDismissed::from_raw(data),
            ),
            GameLobbyJoinRequested::ID => {
                Self::GameLobbyJoinRequested(GameLobbyJoinRequested::from_raw(data))
            }
            FriendChatMessageReceived::ID => {
                Self::FriendChatMessageReceived(FriendChatMessageReceived::from_raw(data))
            }
            FriendRichPresenceUpdate::ID => {
                Self::FriendRichPresenceUpdate(FriendRichPresenceUpdate::from_raw(data))
            }
            GameOverlayActivated::ID => {
                Self::GameOverlayActivated(GameOverlayActivated::from_raw(data))
            }
            GamepadTextInputDismissed::ID => {
                Self::GamepadTextInputDismissed(GamepadTextInputDismissed::from_raw(data))
            }
            GameRichPresenceJoinRequested::ID => {
                Self::GameRichPresenceJoinRequested(GameRichPresenceJoinRequested::from_raw(data))
            }
            GlobalStatsReceived::ID => {
                Self::GlobalStatsReceived(GlobalStatsReceived::from_raw(data))
            }
            LobbyChatMsg::ID => Self::LobbyChatMsg(LobbyChatMsg::from_raw(data)),
            LobbyChatUpdate::ID => Self::LobbyChatUpdate(LobbyChatUpdate::from_raw(data)),
            LobbyCreated::ID => Self::LobbyCreated(LobbyCreated::from_raw(data)),
            LobbyDataUpdate::ID => Self::LobbyDataUpdate(LobbyDataUpdate::from_raw(data)),
            LobbyEnter::ID => Self::LobbyEnter(LobbyEnter::from_raw(data)),
            MicroTxnAuthorizationResponse::ID => {
                Self::MicroTxnAuthorizationResponse(MicroTxnAuthorizationResponse::from_raw(data))
            }
            NetworkingMessagesSessionFailed::ID => Self::NetworkingMessagesSessionFailed(
                NetworkingMessagesSessionFailed::from_raw(data),
            ),
            NetworkingMessagesSessionRequest::ID => Self::NetworkingMessagesSessionRequest(
                NetworkingMessagesSessionRequest::from_raw(data),
            ),
            P2PSessionConnectFail::ID => {
                Self::P2PSessionConnectFail(P2PSessionConnectFail::from_raw(data))
            }
            P2PSessionRequest::ID => Self::P2PSessionRequest(P2PSessionRequest::from_raw(data)),
            PersonaStateChange::ID => Self::PersonaStateChange(PersonaStateChange::from_raw(data)),
            RelayNetworkStatusCallback::ID => {
                Self::RelayNetworkStatusCallback(RelayNetworkStatusCallback::from_raw(data))
            }
            RemotePlayConnected::ID => {
                Self::RemotePlayConnected(RemotePlayConnected::from_raw(data))
            }
            RemotePlayDisconnected::ID => {
                Self::RemotePlayDisconnected(RemotePlayDisconnected::from_raw(data))
            }
            ScreenshotRequested::ID => {
                Self::ScreenshotRequested(ScreenshotRequested::from_raw(data))
            }
            ScreenshotReady::ID => Self::ScreenshotReady(ScreenshotReady::from_raw(data)),
            SteamServerConnectFailure::ID => {
                Self::SteamServerConnectFailure(SteamServerConnectFailure::from_raw(data))
            }
            SteamServersConnected::ID => {
                Self::SteamServersConnected(SteamServersConnected::from_raw(data))
            }
            SteamServersDisconnected::ID => {
                Self::SteamServersDisconnected(SteamServersDisconnected::from_raw(data))
            }
            TimelineEventRecordingExists::ID => {
                Self::TimelineEventRecordingExists(TimelineEventRecordingExists::from_raw(data))
            }
            TimelineGamePhaseRecordingExists::ID => Self::TimelineGamePhaseRecordingExists(
                TimelineGamePhaseRecordingExists::from_raw(data),
            ),
            TicketForWebApiResponse::ID => {
                Self::TicketForWebApiResponse(TicketForWebApiResponse::from_raw(data))
            }
            UnreadChatMessagesChanged::ID => {
                Self::UnreadChatMessagesChanged(UnreadChatMessagesChanged::from_raw(data))
            }
            UserAchievementStored::ID => {
                Self::UserAchievementStored(UserAchievementStored::from_raw(data))
            }
            UserAchievementIconFetched::ID => {
                Self::UserAchievementIconFetched(UserAchievementIconFetched::from_raw(data))
            }
            UserStatsReceived::ID => Self::UserStatsReceived(UserStatsReceived::from_raw(data)),
            UserStatsStored::ID => Self::UserStatsStored(UserStatsStored::from_raw(data)),
            ValidateAuthTicketResponse::ID => {
                Self::ValidateAuthTicketResponse(ValidateAuthTicketResponse::from_raw(data))
            }
            AddAppDependencyResult::ID => {
                Self::AddAppDependencyResult(AddAppDependencyResult::from_raw(data))
            }
            AddUGCDependencyResult::ID => {
                Self::AddUGCDependencyResult(AddUGCDependencyResult::from_raw(data))
            }
            GetAppDependenciesResult::ID => {
                Self::GetAppDependenciesResult(GetAppDependenciesResult::from_raw(data))
            }
            GetUserItemVoteResult::ID => {
                Self::GetUserItemVoteResult(GetUserItemVoteResult::from_raw(data))
            }
            RemoveAppDependencyResult::ID => {
                Self::RemoveAppDependencyResult(RemoveAppDependencyResult::from_raw(data))
            }
            RemoveUGCDependencyResult::ID => {
                Self::RemoveUGCDependencyResult(RemoveUGCDependencyResult::from_raw(data))
            }
            SetUserItemVoteResult::ID => {
                Self::SetUserItemVoteResult(SetUserItemVoteResult::from_raw(data))
            }
            UserFavoriteItemsListChanged::ID => {
                Self::UserFavoriteItemsListChanged(UserFavoriteItemsListChanged::from_raw(data))
            }
            WorkshopEULAStatus::ID => Self::WorkshopEULAStatus(WorkshopEULAStatus::from_raw(data)),
            GSClientApprove::ID => Self::GSClientApprove(GSClientApprove::from_raw(data)),
            GSClientDeny::ID => Self::GSClientDeny(GSClientDeny::from_raw(data)),
            GSClientKick::ID => Self::GSClientKick(GSClientKick::from_raw(data)),
            GSClientGroupStatus::ID => {
                Self::GSClientGroupStatus(GSClientGroupStatus::from_raw(data))
            }
            NewUrlLaunchParameters::ID => {
                Self::NewUrlLaunchParameters(NewUrlLaunchParameters::from_raw(data))
            }
            _ => return None,
        })
    }
}

pub unsafe trait Callback {
    const ID: i32;
    unsafe fn from_raw(raw: *mut c_void) -> Self;
}

/// A handle that can be used to remove a callback
/// at a later point.
///
/// Removes the callback from the Steam API context when dropped.
pub struct CallbackHandle {
    id: i32,
    inner: Weak<Inner>,
}

impl Drop for CallbackHandle {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.upgrade() {
            let id = self.id;
            // Deferred when this thread is dispatching for this context: the
            // dispatch loop holds the callback lock while user code runs, and
            // a handle dropped by that user code must not re-enter it.
            inner
                .callbacks
                .run_or_defer(Arc::as_ptr(&inner).cast::<()>(), move |callbacks| {
                    callbacks.remove_handler(id)
                });
        }
    }
}

macro_rules! impl_callback {
    ($fn_arg_name:ident: $sys_ty:ident => $callback_ty:ident $from_raw_impl:tt) => {
        pastey::item! {
            unsafe impl Callback for $callback_ty {
                const ID: i32 = steamworks_sys::[<$sys_ty _ k_iCallback>] as i32;

                unsafe fn from_raw(raw: *mut c_void) -> Self {
                    let $fn_arg_name = raw.cast::<steamworks_sys::$sys_ty>().read_unaligned();
                    $from_raw_impl
                }
            }
        }
    };
}

pub(crate) unsafe fn register_callback<C, F>(inner: &Arc<Inner>, mut f: F) -> CallbackHandle
where
    C: Callback,
    F: FnMut(C) + Send + 'static,
{
    let handler = Box::new(move |param| {
        let param = C::from_raw(param);
        f(param)
    });
    // Registering from inside a callback of the same context is the common
    // case that used to self-deadlock, so it goes through the queue.
    inner
        .callbacks
        .run_or_defer(Arc::as_ptr(inner).cast::<()>(), move |callbacks| {
            callbacks.set_handler(C::ID, handler)
        });
    CallbackHandle {
        id: C::ID,
        inner: Arc::downgrade(inner),
    }
}

pub(crate) unsafe fn register_call_result<C, F>(
    inner: &Arc<Inner>,
    api_call: sys::SteamAPICall_t,
    f: F,
) where
    F: for<'a> FnOnce(&'a C, bool) + 'static + Send,
{
    inner.callbacks.call_results.lock().unwrap().insert(
        api_call,
        Box::new(move |param, failed| {
            let value = param.cast::<C>().read_unaligned();
            f(&value, failed)
        }),
    );
}

#[cfg(test)]
mod stream_coverage {
    use super::*;

    /// Callback families `CallbackResult` has never carried. They are still
    /// reachable through `register_callback`; only the stream is missing them.
    ///
    /// This list may only shrink. `every_callback_is_streamable_or_listed_as_debt`
    /// fails on any callback that is defined but neither streamed nor listed here,
    /// so a new callback cannot quietly widen the gap the doc claims is closed.
    const CALLBACKS_NOT_IN_STREAM: &[&str] = &[
        // ugc.rs
        "DeleteItemResult",
        // inventory.rs
        "InventoryDefinitionUpdate",
        "InventoryFullUpdate",
        "InventoryResultReady",
        // parties.rs
        "ActiveBeaconsUpdated",
        "AvailableBeaconLocationsUpdated",
        "PartyReservationNotification",
        // server_stats.rs
        "GameServerStatsReceived",
        "GameServerStatsStored",
        "GameServerStatsUnloaded",
        // html_surface.rs
        "HtmlBrowserReady",
        "HtmlCanGoBackAndForward",
        "HtmlCloseBrowser",
        "HtmlFileOpenDialog",
        "HtmlFinishedRequest",
        "HtmlJsAlert",
        "HtmlJsConfirm",
        "HtmlLinkAtPosition",
        "HtmlNewWindow",
        "HtmlStartRequest",
        "HtmlTitleChanged",
        "HtmlUrlChanged",
        // music.rs
        "MusicPlaybackStatusChanged",
        "MusicVolumeChanged",
    ];

    /// The `Name` out of every `impl_callback!(cb: X_t => Name { .. })` call.
    fn defined_callbacks() -> Vec<String> {
        let mut out = Vec::new();
        for path in source_files() {
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            for line in text.lines() {
                // The macro's own documentation shows a `=> Name` example; it is
                // not a callback definition.
                if line.trim_start().starts_with("//") {
                    continue;
                }
                let Some(at) = line.find("impl_callback!(") else {
                    continue;
                };
                let rest = &line[at + "impl_callback!".len()..];
                let Some(arrow) = rest.find("=> ") else {
                    continue;
                };
                let name = rest[arrow + 3..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect::<String>();
                if !name.is_empty() {
                    out.push(name);
                }
            }
        }
        out
    }

    /// The variant names inside `pub enum CallbackResult { .. }`.
    fn streamed_callbacks() -> Vec<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/callback.rs");
        let text = std::fs::read_to_string(path).unwrap();
        let mut out = Vec::new();
        let mut inside = false;
        for line in text.lines() {
            if !inside {
                inside = line.trim() == "pub enum CallbackResult {";
                continue;
            }
            if line.trim() == "}" {
                break;
            }
            let name = line
                .trim()
                .split('(')
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            if !name.is_empty() {
                out.push(name);
            }
        }
        out
    }

    fn source_files() -> Vec<std::path::PathBuf> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                    out.push(path);
                }
            }
        }
        out
    }

    #[test]
    fn the_two_parsers_see_the_whole_crate() {
        // Without these floors an empty parse would make both coverage tests
        // below pass by looking at nothing. Both numbers are measured, not rounded:
        // 78 definitions minus 54 streamed is the 24-entry `CALLBACKS_NOT_IN_STREAM` list,
        // so a matcher that starts missing whole files trips here rather than quietly
        // lowering the debt.
        let defined = defined_callbacks();
        let streamed = streamed_callbacks();
        assert!(
            defined.len() >= 78,
            "only parsed {} callback definitions -- the matcher is broken",
            defined.len()
        );
        assert!(
            streamed.len() >= 54,
            "only parsed {} CallbackResult variants -- the matcher is broken",
            streamed.len()
        );
    }

    #[test]
    fn every_callback_is_streamable_or_listed_as_debt() {
        let streamed = streamed_callbacks();
        let mut unlisted = Vec::new();
        for name in defined_callbacks() {
            if !streamed.contains(&name) && !CALLBACKS_NOT_IN_STREAM.contains(&name.as_str()) {
                unlisted.push(name);
            }
        }
        assert!(
            unlisted.is_empty(),
            "callbacks the stream cannot deliver and nobody registered: {unlisted:?} -- \
             add a CallbackResult variant or a line in CALLBACKS_NOT_IN_STREAM with the reason"
        );
    }

    #[test]
    fn debt_entries_point_at_callbacks_that_are_still_unstreamed() {
        let defined = defined_callbacks();
        let streamed = streamed_callbacks();
        for name in CALLBACKS_NOT_IN_STREAM {
            assert!(
                defined.iter().any(|defined| defined == name),
                "{name} is listed as debt but no longer a callback -- drop the list entry"
            );
            assert!(
                !streamed.iter().any(|variant| variant == name),
                "{name} is streamed now; the debt list entry is stale"
            );
        }
    }

    #[test]
    fn chat_callbacks_reach_the_stream() {
        // The receive path only exists for stream users if these IDs decode.
        // A variant added without its `from_raw` arm would compile and stay silent.
        let friend = sys::GameConnectedFriendChatMsg_t_k_iCallback as i32;
        let clan = sys::GameConnectedClanChatMsg_t_k_iCallback as i32;
        let unread = sys::UnreadChatMessagesChanged_t_k_iCallback as i32;
        let rich = sys::FriendRichPresenceUpdate_t_k_iCallback as i32;
        for id in [friend, clan, unread, rich] {
            let payload = [0u8; 64];
            let decoded = unsafe { CallbackResult::from_raw(id, payload.as_ptr() as *mut c_void) };
            assert!(
                decoded.is_some(),
                "callback id {id} is not dispatched by CallbackResult::from_raw"
            );
        }
        // Distinct IDs: two callbacks sharing one would make one of the arms
        // unreachable and its events would be delivered as the other type.
        let ids = [friend, clan, unread, rich];
        for i in 0..ids.len() {
            for j in (i + 1)..ids.len() {
                assert_ne!(ids[i], ids[j]);
            }
        }
    }

    #[test]
    fn stream_decodes_chat_payloads_field_by_field() {
        // Locks the field mapping of the two chat announcements: reading the
        // wrong member would still compile, and a stream user could not tell.
        let mut clan = sys::GameConnectedClanChatMsg_t {
            m_steamIDClanChat: steam_id(111),
            m_steamIDUser: steam_id(222),
            m_iMessageID: 7,
        };
        let raw = (&mut clan as *mut sys::GameConnectedClanChatMsg_t).cast::<c_void>();
        match unsafe { CallbackResult::from_raw(ClanChatMessageReceived::ID, raw) } {
            Some(CallbackResult::ClanChatMessageReceived(ev)) => {
                assert_eq!(ev.clan_chat, SteamId::from_raw(111));
                assert_eq!(ev.sender, SteamId::from_raw(222));
                assert_eq!(ev.message_id, 7);
            }
            other => panic!("clan chat announcement decoded as {other:?}"),
        }

        let mut friend = sys::GameConnectedFriendChatMsg_t {
            m_steamIDUser: steam_id(333),
            m_iMessageID: 9,
        };
        let raw = (&mut friend as *mut sys::GameConnectedFriendChatMsg_t).cast::<c_void>();
        match unsafe { CallbackResult::from_raw(FriendChatMessageReceived::ID, raw) } {
            Some(CallbackResult::FriendChatMessageReceived(ev)) => {
                assert_eq!(ev.sender, SteamId::from_raw(333));
                assert_eq!(ev.message_id, 9);
            }
            other => panic!("friend chat announcement decoded as {other:?}"),
        }
    }

    fn steam_id(raw: u64) -> sys::CSteamID {
        sys::CSteamID {
            m_steamid: sys::CSteamID_SteamID_t { m_unAll64Bits: raw },
        }
    }
}
