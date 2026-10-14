//! The chat half of the friends surface: intercepted friend messages, clan chat rooms
//! and the announcements that say something arrived.
//!
//! Split out of `friends.rs` under the same rule as the workshop split -- one file per
//! thing the interface lets you do -- so the chat cluster has a home and the size
//! ceiling on the accessor stays meaningful.

use super::*;

/// The most one chat read will ask Steam to fill.
///
/// Chat messages are short, and `cubData`/`cchTextMax` are `int` arguments: an
/// unchecked `usize` would wrap negative and Steam would then be handed a
/// nonsense length.
pub const MAX_CHAT_MESSAGE_BYTES: usize = 64 * 1024;

/// One intercepted friend message, read back with
/// [`Friends::friend_message`].
#[derive(Clone, Debug, PartialEq)]
pub struct FriendChatMessage {
    /// What Steam says this entry is: text, a typing notification, an emote.
    pub kind: ChatEntryType,
    pub text: String,
    /// Steam filled the whole buffer, so `text` may be a prefix of a longer
    /// message.
    pub possibly_truncated: bool,
}

/// One clan chat message, read back with [`Friends::clan_chat_message`].
#[derive(Clone, Debug, PartialEq)]
pub struct ClanChatMessage {
    /// `None` when Steam left the sender empty.
    pub sender: Option<SteamId>,
    pub kind: ChatEntryType,
    pub text: String,
    pub possibly_truncated: bool,
}

/// The answer to [`Friends::join_clan_chat_room`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClanChatRoomJoined {
    pub clan_chat: SteamId,
    pub response: ChatRoomEnterResponse,
}

/// A friend message arrived and Steam has hidden it from the overlay, because
/// [`Friends::set_listen_for_friends_messages`] is on. Read it back with
/// [`Friends::friend_message`] using this callback's `message_id`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FriendChatMessageReceived {
    pub sender: SteamId,
    /// Pass this to [`Friends::friend_message`].
    pub message_id: i32,
}

impl_callback!(cb: GameConnectedFriendChatMsg_t => FriendChatMessageReceived {
    Self {
        sender: SteamId::from_raw(cb.m_steamIDUser.m_steamid.m_unAll64Bits),
        message_id: cb.m_iMessageID,
    }
});

/// A message arrived in a clan chat this client is connected to. Read it back
/// with [`Friends::clan_chat_message`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClanChatMessageReceived {
    pub clan_chat: SteamId,
    pub sender: SteamId,
    /// The id [`Friends::clan_chat_message`] reads back.
    pub message_id: i32,
}

impl_callback!(cb: GameConnectedClanChatMsg_t => ClanChatMessageReceived {
    Self {
        clan_chat: SteamId::from_raw(cb.m_steamIDClanChat.m_steamid.m_unAll64Bits),
        sender: SteamId::from_raw(cb.m_steamIDUser.m_steamid.m_unAll64Bits),
        message_id: cb.m_iMessageID,
    }
});

/// Some chat's unread count changed; Steam gives no numbers with it, so re-read
/// [`Friends::unread_priority_chat_count`] and the messages themselves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnreadChatMessagesChanged;

impl_callback!(_cb: UnreadChatMessagesChanged_t => UnreadChatMessagesChanged {
    Self
});

/// How much of a chat message to ask Steam for, kept inside `int` range.
pub(super) fn chat_buffer_len(requested: usize) -> usize {
    requested.clamp(1, MAX_CHAT_MESSAGE_BYTES)
}

/// The text of a chat entry, cut where Steam ended it.
///
/// Steam does not promise a nul inside the bytes it reports, and does not
/// promise valid UTF-8, so this stops at the first nul and replaces the rest.
pub(super) fn chat_text(bytes: &[u8]) -> String {
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// Maps Steam's `EChatEntryType` answer.
///
/// The wrapper reads the out-parameter as a plain 32-bit integer instead of a
/// `sys::EChatEntryType`: the generated enum is `#[repr(i32)]` on Windows and
/// `#[repr(u32)]` elsewhere, and both are `#[non_exhaustive]`, so a value Steam
/// has not named here must not be turned into one of its variants.
pub(super) fn chat_entry_type_from(raw: i32) -> ChatEntryType {
    u8::try_from(raw)
        .map(ChatEntryType::from)
        .unwrap_or(ChatEntryType::Invalid)
}

#[cfg(test)]
mod chat_tests {
    use super::*;

    #[test]
    fn chat_buffer_len_keeps_the_length_argument_an_int() {
        // `cubData`/`cchTextMax` are `int` arguments: an uncapped `usize` would
        // wrap negative, and Steam would be handed a nonsense length for a
        // buffer this side never allocated.
        assert_eq!(chat_buffer_len(0), 1);
        assert_eq!(chat_buffer_len(64), 64);
        assert_eq!(
            chat_buffer_len(MAX_CHAT_MESSAGE_BYTES),
            MAX_CHAT_MESSAGE_BYTES
        );
        assert_eq!(chat_buffer_len(usize::MAX), MAX_CHAT_MESSAGE_BYTES);
        for requested in [
            0,
            1,
            7,
            MAX_CHAT_MESSAGE_BYTES,
            MAX_CHAT_MESSAGE_BYTES + 1,
            MAX_CHAT_MESSAGE_BYTES * 2,
            i32::MAX as usize,
            usize::MAX,
        ] {
            let len = chat_buffer_len(requested);
            assert!(
                i32::try_from(len).is_ok(),
                "{requested} -> {len} does not fit the int argument"
            );
            assert!(
                len >= 1,
                "{requested} -> {len} leaves Steam a zero-length buffer"
            );
        }
    }

    #[test]
    fn chat_text_stops_where_steam_ended_it() {
        assert_eq!(chat_text(b"hello\0trailing"), "hello");
        assert_eq!(chat_text(b"no terminator"), "no terminator");
        assert_eq!(chat_text(b""), "");
        assert_eq!(chat_text(b"\0"), "");
        // Steam hands back bytes, not promised UTF-8: a lone 0xFF must not panic
        // someone reading a chat message.
        assert_eq!(chat_text(&[0xff, b'a']), "\u{fffd}a");
    }

    #[test]
    fn entry_type_answer_is_never_wrapped_to_fit() {
        use ChatEntryType::*;
        let named = [
            (1, ChatMsg),
            (2, Typing),
            (3, InviteGame),
            (4, Emote),
            (6, LeftConversation),
            (7, Entered),
            (8, WasKicked),
            (9, WasBanned),
            (10, Disconnected),
            (11, HistoricalChat),
            (14, LinkBlocked),
        ];
        for (code, expected) in named {
            assert_eq!(chat_entry_type_from(code), expected);
        }
        let mapped: Vec<ChatEntryType> = named.iter().map(|(_, value)| *value).collect();
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(mapped[i], mapped[j], "distinct Steam codes must not merge");
            }
        }
        // Gaps in Valve's numbering are not aliases for a neighbour.
        for gap in [0, 5, 12, 13, 15, 200] {
            assert_eq!(
                chat_entry_type_from(gap),
                Invalid,
                "{gap} is not a named code"
            );
        }
        // 257 would read back as `ChatMsg` if the raw int were cast down to `u8`
        // instead of range-checked first.
        assert_eq!(chat_entry_type_from(257), Invalid);
        assert_eq!(chat_entry_type_from(-1), Invalid);
        assert_eq!(chat_entry_type_from(i32::MAX), Invalid);
    }

    #[test]
    fn chat_text_argument_with_an_interior_nul_is_refused_not_panicked() {
        assert!(c_string_arg("plain").is_ok());
        assert_eq!(
            c_string_arg("bad\0text").err(),
            Some(SteamError::InvalidParameter)
        );
    }
}
