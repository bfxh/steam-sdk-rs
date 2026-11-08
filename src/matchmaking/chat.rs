//! Lobby chat: sending a message, paging through a lobby's chat history, and the enums
//! that say what changed or what kind of entry this is.

use super::*;

impl Matchmaking {
    /// Broadcasts a chat message (text or binary data) to all users in the lobby.
    ///
    /// # Parameters
    /// - `lobby`: The Steam ID of the lobby to send the chat message to.
    /// - `msg`: This can be text or binary data, up to 4 Kilobytes in size.
    ///
    /// # Description
    /// All users in the lobby (including the local user) will receive a `LobbyChatMsg_t` callback
    /// with the message.
    ///
    /// If you're sending binary data, you should prefix a header to the message so that you know
    /// to treat it as your custom data rather than a plain old text message.
    ///
    /// For communication that needs to be arbitrated (e.g., having a user pick from a set of characters),
    /// you can use the lobby owner as the decision maker. `GetLobbyOwner` returns the current lobby owner.
    /// There is guaranteed to always be one and only one lobby member who is the owner.
    /// So for the choose-a-character scenario, the user who is picking a character would send the binary
    /// message 'I want to be Zoe', the lobby owner would see that message, see if it was OK, and broadcast
    /// the appropriate result (user X is Zoe).
    ///
    /// These messages are sent via the Steam back-end, and so the bandwidth available is limited.
    /// For higher-volume traffic like voice or game data, you'll want to use the Steam Networking API.
    ///
    /// # Returns
    /// Returns `Ok(())` if the message was successfully sent. Returns an error of type `SteamError` if the
    /// message is too small or too large, or if no connection to Steam could be made.
    pub fn send_lobby_chat_message(&self, lobby: LobbyId, msg: &[u8]) -> Result<(), SteamError> {
        match unsafe {
            steamworks_sys::SteamAPI_ISteamMatchmaking_SendLobbyChatMsg(
                self.mm,
                lobby.0,
                msg.as_ptr().cast(),
                msg.len() as i32,
            )
        } {
            true => Ok(()),
            false => Err(SteamError::IOFailure),
        }
    }

    /// Gets the data from a lobby chat message after receiving a `LobbyChatMsg_t` callback.
    ///
    /// # Parameters
    /// - `lobby`: The Steam ID of the lobby to get the chat message from.
    /// - `chat_id`: The index of the chat entry in the lobby.
    /// - `buffer`: Buffer to save retrieved message data to. The buffer should be no
    /// more than 4 Kilobytes.
    ///
    /// # Returns
    /// Returns `&[u8]` A resliced byte array of the message buffer
    pub fn get_lobby_chat_entry<'a>(
        &self,
        lobby: LobbyId,
        chat_id: i32,
        buffer: &'a mut [u8],
    ) -> &'a [u8] {
        let mut steam_user = sys::CSteamID {
            m_steamid: sys::CSteamID_SteamID_t { m_unAll64Bits: 0 },
        };
        let mut chat_type = steamworks_sys::EChatEntryType::k_EChatEntryTypeInvalid;
        unsafe {
            let len = sys::SteamAPI_ISteamMatchmaking_GetLobbyChatEntry(
                self.mm,
                lobby.0,
                chat_id,
                &mut steam_user,
                buffer.as_mut_ptr().cast(),
                buffer.len() as _,
                &mut chat_type,
            );
            return &buffer[0..len as usize];
        }
    }
}

/// Flags describing how a users lobby state has changed. This is provided from `LobbyChatUpdate`.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ChatMemberStateChange {
    /// This user has joined or is joining the lobby.
    Entered,

    /// This user has left or is leaving the lobby.
    Left,

    /// User disconnected without leaving the lobby first.
    Disconnected,

    /// The user has been kicked.
    Kicked,

    /// The user has been kicked and banned.
    Banned,

    /// Steam sent a code this binding does not name, or a combination of the flags below.
    ///
    /// The field is a bitmask -- `isteammatchmaking.h:531-541` names five flags, one bit each from
    /// 0x0001 to 0x0010, under the header's own note that these are "fields specific to a chat
    /// member" -- and this wrapper reads only those five, so a combination lands here rather than
    /// panicking. Reading it as flags instead of one state is
    /// the open change; until then `Unknown` is the honest answer for anything unnamed.
    Unknown,
}

/// One lobby-state flag, or [`ChatMemberStateChange::Unknown`] for anything else.
pub(crate) fn chat_member_state_change(code: u32) -> ChatMemberStateChange {
    if code == sys::EChatMemberStateChange::k_EChatMemberStateChangeEntered as u32 {
        return ChatMemberStateChange::Entered;
    }
    if code == sys::EChatMemberStateChange::k_EChatMemberStateChangeLeft as u32 {
        return ChatMemberStateChange::Left;
    }
    if code == sys::EChatMemberStateChange::k_EChatMemberStateChangeDisconnected as u32 {
        return ChatMemberStateChange::Disconnected;
    }
    if code == sys::EChatMemberStateChange::k_EChatMemberStateChangeKicked as u32 {
        return ChatMemberStateChange::Kicked;
    }
    if code == sys::EChatMemberStateChange::k_EChatMemberStateChangeBanned as u32 {
        return ChatMemberStateChange::Banned;
    }
    ChatMemberStateChange::Unknown
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ChatEntryType {
    Invalid,
    ChatMsg,
    Typing,
    InviteGame,
    Emote,
    LeftConversation,
    Entered,
    WasKicked,
    WasBanned,
    Disconnected,
    HistoricalChat,
    LinkBlocked,
}

impl From<u8> for ChatEntryType {
    fn from(value: u8) -> Self {
        match value {
            x if x == sys::EChatEntryType::k_EChatEntryTypeInvalid as u8 => ChatEntryType::Invalid,
            x if x == sys::EChatEntryType::k_EChatEntryTypeChatMsg as u8 => ChatEntryType::ChatMsg,
            x if x == sys::EChatEntryType::k_EChatEntryTypeTyping as u8 => ChatEntryType::Typing,
            x if x == sys::EChatEntryType::k_EChatEntryTypeInviteGame as u8 => {
                ChatEntryType::InviteGame
            }
            x if x == sys::EChatEntryType::k_EChatEntryTypeEmote as u8 => ChatEntryType::Emote,
            x if x == sys::EChatEntryType::k_EChatEntryTypeLeftConversation as u8 => {
                ChatEntryType::LeftConversation
            }
            x if x == sys::EChatEntryType::k_EChatEntryTypeEntered as u8 => ChatEntryType::Entered,
            x if x == sys::EChatEntryType::k_EChatEntryTypeWasKicked as u8 => {
                ChatEntryType::WasKicked
            }
            x if x == sys::EChatEntryType::k_EChatEntryTypeWasBanned as u8 => {
                ChatEntryType::WasBanned
            }
            x if x == sys::EChatEntryType::k_EChatEntryTypeDisconnected as u8 => {
                ChatEntryType::Disconnected
            }
            x if x == sys::EChatEntryType::k_EChatEntryTypeHistoricalChat as u8 => {
                ChatEntryType::HistoricalChat
            }
            x if x == sys::EChatEntryType::k_EChatEntryTypeLinkBlocked as u8 => {
                ChatEntryType::LinkBlocked
            }
            _ => ChatEntryType::Invalid,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum ChatRoomEnterResponse {
    Success,
    DoesntExist,
    NotAllowed,
    Full,
    Error,
    Banned,
    Limited,
    ClanDisabled,
    CommunityBan,
    MemberBlockedYou,
    YouBlockedMember,
    RatelimitExceeded,
}

impl From<u32> for ChatRoomEnterResponse {
    fn from(value: u32) -> Self {
        match value {
            x if x == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseSuccess as u32 => {
                ChatRoomEnterResponse::Success
            }
            x if x == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseDoesntExist as u32 => {
                ChatRoomEnterResponse::DoesntExist
            }
            x if x == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseNotAllowed as u32 => {
                ChatRoomEnterResponse::NotAllowed
            }
            x if x == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseFull as u32 => {
                ChatRoomEnterResponse::Full
            }
            x if x == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseError as u32 => {
                ChatRoomEnterResponse::Error
            }
            x if x == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseBanned as u32 => {
                ChatRoomEnterResponse::Banned
            }
            x if x == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseLimited as u32 => {
                ChatRoomEnterResponse::Limited
            }
            x if x == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseClanDisabled as u32 => {
                ChatRoomEnterResponse::ClanDisabled
            }
            x if x == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseCommunityBan as u32 => {
                ChatRoomEnterResponse::CommunityBan
            }
            x if x
                == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseMemberBlockedYou as u32 =>
            {
                ChatRoomEnterResponse::MemberBlockedYou
            }
            x if x
                == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseYouBlockedMember as u32 =>
            {
                ChatRoomEnterResponse::YouBlockedMember
            }
            x if x
                == sys::EChatRoomEnterResponse::k_EChatRoomEnterResponseRatelimitExceeded
                    as u32 =>
            {
                ChatRoomEnterResponse::RatelimitExceeded
            }
            _ => ChatRoomEnterResponse::Error,
        }
    }
}

#[cfg(test)]
mod chat_flag_tests {
    use super::*;

    #[test]
    fn each_named_flag_maps_to_its_own_state() {
        let named = [
            (1u32, ChatMemberStateChange::Entered),
            (2, ChatMemberStateChange::Left),
            (4, ChatMemberStateChange::Disconnected),
            (8, ChatMemberStateChange::Kicked),
            (16, ChatMemberStateChange::Banned),
        ];
        for (code, expected) in named {
            assert_eq!(chat_member_state_change(code), expected, "code {code}");
        }
    }

    #[test]
    fn an_unnamed_code_or_a_combination_answers_unknown_instead_of_panicking() {
        // Steam's field is a bitmask, so `Entered | Left` and `Kicked | Banned` are expressible,
        // and a newer runtime can send a flag this binding never heard of. Before this mapping
        // existed, all four of these were `unreachable!()` -- a panic inside the caller's process.
        for code in [0u32, 3, 24, 32, 1 << 20] {
            assert_eq!(
                chat_member_state_change(code),
                ChatMemberStateChange::Unknown,
                "code {code} must not panic"
            );
        }
    }
}
