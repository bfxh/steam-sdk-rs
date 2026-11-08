//! The payloads `ISteamMatchmaking` fires: chat traffic, member changes, lobby creation,
//! data updates and the result of entering a lobby.

use super::*;

/// A chat (text or binary) message for this lobby has been received. After getting this you must use GetLobbyChatEntry to retrieve the contents of this message.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LobbyChatMsg {
    /// The Steam ID of the lobby this message was sent in.
    pub lobby: LobbyId,
    /// Steam ID of the user who sent this message. Note that it could have been the local user.
    pub user: SteamId,
    /// Type of message received. This is actually a EChatEntryType.
    pub chat_entry_type: ChatEntryType,
    /// The index of the chat entry to use with GetLobbyChatEntry, this is not valid outside of the scope of this callback and should never be stored.
    pub chat_id: i32,
}

impl_callback!(cb: LobbyChatMsg_t => LobbyChatMsg {
    Self {
        lobby: LobbyId(cb.m_ulSteamIDLobby),
        user: SteamId(cb.m_ulSteamIDUser),
        chat_entry_type: cb.m_eChatEntryType.into(),
        chat_id: cb.m_iChatID as i32,
    }
});

/// A lobby chat room state has changed, this is usually sent when a user has joined or left the lobby.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LobbyChatUpdate {
    /// The Steam ID of the lobby.
    pub lobby: LobbyId,
    /// The user who's status in the lobby just changed - can be recipient.
    pub user_changed: SteamId,
    /// Chat member who made the change. This can be different from `user_changed` if kicking, muting, etc. For example, if one user kicks another from the lobby, this will be set to the id of the user who initiated the kick.
    pub making_change: SteamId,
    /// "ChatMemberStateChange" values.
    pub member_state_change: ChatMemberStateChange,
}

impl_callback!(cb: LobbyChatUpdate_t => LobbyChatUpdate {
    Self {
        lobby: LobbyId(cb.m_ulSteamIDLobby),
        user_changed: SteamId(cb.m_ulSteamIDUserChanged),
        making_change: SteamId(cb.m_ulSteamIDMakingChange),
        member_state_change: chat_member_state_change(cb.m_rgfChatMemberStateChange),
    }
});

/// Result of our request to create a Lobby. At this point, the lobby has been joined and is ready for use, a LobbyEnter_t callback will also be received (since the local user is joining their own lobby).
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LobbyCreated {
    /// The result of the operation
    pub result: SteamResult,
    /// The Steam ID of the lobby that was created, 0 if failed.
    pub lobby: LobbyId,
}

impl_callback!(cb: LobbyCreated_t => LobbyCreated {
    Self {
        result: to_steam_result(cb.m_eResult),
        lobby: LobbyId(cb.m_ulSteamIDLobby),
    }
});

/// The lobby metadata has changed.
/// If m_ulSteamIDMember is a user in the lobby, then use GetLobbyMemberData to access per-user details; otherwise, if m_ulSteamIDMember == m_ulSteamIDLobby, use GetLobbyData to access the lobby metadata.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LobbyDataUpdate {
    /// The Steam ID of the Lobby.
    pub lobby: LobbyId,
    /// Steam ID of either the member whose data changed, or the room itself.
    pub member: SteamId,
    /// true if the lobby data was successfully changed, otherwise false.
    pub success: bool,
}

impl_callback!(cb: LobbyDataUpdate_t => LobbyDataUpdate {
    Self {
        lobby: LobbyId(cb.m_ulSteamIDLobby),
        member: SteamId(cb.m_ulSteamIDMember),
        success: cb.m_bSuccess != 0,
    }
});

/// Recieved upon attempting to enter a lobby. Lobby metadata is available to use immediately after receiving this.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LobbyEnter {
    /// The steam ID of the Lobby you have entered.
    pub lobby: LobbyId,
    /// Unused - Always 0.
    pub chat_permissions: u32,
    /// If true, then only invited users may join.
    pub blocked: bool,
    /// This is actually a EChatRoomEnterResponse value. This will be set to k_EChatRoomEnterResponseSuccess if the lobby was successfully joined, otherwise it will be k_EChatRoomEnterResponseError.
    pub chat_room_enter_response: ChatRoomEnterResponse,
}

impl_callback!(cb: LobbyEnter_t => LobbyEnter {
    Self {
        lobby: LobbyId(cb.m_ulSteamIDLobby),
        chat_permissions: cb.m_rgfChatPermissions,
        blocked: cb.m_bLocked,
        chat_room_enter_response: cb.m_EChatRoomEnterResponse.into(),
    }
});
