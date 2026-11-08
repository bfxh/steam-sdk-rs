//! Clan chat rooms: joining and leaving, the member roster, sending and reading messages, and
//! the three window-state calls that ask the Steam client whether the room is on screen.

use super::*;

impl Friends {
    /// Joins a clan's chat room.
    ///
    /// Steam answers separately, as [`ClanChatRoomJoined`]; that answer's
    /// `clan_chat` is the id the room reads want
    /// ([`Friends::clan_chat_members`], [`Friends::clan_chat_message`]).
    ///
    /// The answer carries *why* a join failed in Steam's own enum rather than an
    /// error code, so a refused join still arrives as `Ok` with a non-success
    /// `response`.
    pub fn join_clan_chat_room<F>(&self, clan: SteamId, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<ClanChatRoomJoined, SteamError>) + 'static + Send,
    {
        let api_call =
            unsafe { sys::SteamAPI_ISteamFriends_JoinClanChatRoom(self.friends, clan.raw()) };
        unsafe {
            register_call_result::<sys::JoinClanChatRoomCompletionResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        Ok(ClanChatRoomJoined {
                            clan_chat: SteamId::from_raw(
                                v.m_steamIDClanChat.m_steamid.m_unAll64Bits,
                            ),
                            response: ChatRoomEnterResponse::from(
                                v.m_eChatRoomEnterResponse as u32,
                            ),
                        })
                    })
                },
            );
        }
        Ok(())
    }

    /// Leaves a clan chat room previously joined with
    /// [`Friends::join_clan_chat_room`].
    pub fn leave_clan_chat_room(&self, clan_chat: SteamId) -> bool {
        unsafe { sys::SteamAPI_ISteamFriends_LeaveClanChatRoom(self.friends, clan_chat.raw()) }
    }

    /// The members of a clan chat room, gathered by index up to
    /// [`Friends::clan_chat_member_count`]. An index Steam answers with the null
    /// id is skipped rather than returned as a player.
    pub fn clan_chat_members(&self, clan_chat: SteamId) -> Vec<SteamId> {
        let count = self.clan_chat_member_count(clan_chat);
        if count <= 0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(count as usize);
        for index in 0..count {
            let raw = unsafe {
                sys::SteamAPI_ISteamFriends_GetChatMemberByIndex(
                    self.friends,
                    clan_chat.raw(),
                    index,
                )
            };
            if raw != 0 {
                out.push(SteamId::from_raw(raw));
            }
        }
        out
    }

    /// Sends a message to a clan chat room.
    ///
    /// `Err(SteamError::InvalidParameter)` means the text never reached Steam
    /// (it contains an interior nul); `Ok(false)` is Steam's own refusal, for
    /// example a room this client has not joined.
    pub fn send_clan_chat_message(
        &self,
        clan_chat: SteamId,
        text: &str,
    ) -> Result<bool, SteamError> {
        let text = c_string_arg(text)?;
        Ok(unsafe {
            sys::SteamAPI_ISteamFriends_SendClanChatMessage(
                self.friends,
                clan_chat.raw(),
                text.as_ptr(),
            )
        })
    }

    /// Reads one message from a clan chat room, by the id Steam announced.
    ///
    /// Unlike [`Friends::friend_message`], this hands back the sender as well --
    /// as an out-parameter Steam may leave empty, which surfaces as
    /// `sender: None` rather than as some other player.
    pub fn clan_chat_message(
        &self,
        clan_chat: SteamId,
        index: i32,
        max_bytes: usize,
    ) -> Option<ClanChatMessage> {
        let capacity = chat_buffer_len(max_bytes);
        let mut buffer = vec![0u8; capacity];
        let mut entry_type: i32 = 0;
        let mut chatter = sys::CSteamID {
            m_steamid: sys::CSteamID_SteamID_t { m_unAll64Bits: 0 },
        };
        let written = unsafe {
            sys::SteamAPI_ISteamFriends_GetClanChatMessage(
                self.friends,
                clan_chat.raw(),
                index,
                buffer.as_mut_ptr().cast::<c_void>(),
                // Lossless because `chat_buffer_len` caps the value.
                capacity as i32,
                &mut entry_type as *mut i32 as *mut sys::EChatEntryType,
                &mut chatter,
            )
        };
        if written <= 0 {
            return None;
        }
        let written = (written as usize).min(capacity);
        let sender = unsafe { chatter.m_steamid.m_unAll64Bits };
        Some(ClanChatMessage {
            sender: (sender != 0).then(|| SteamId::from_raw(sender)),
            kind: chat_entry_type_from(entry_type),
            text: chat_text(&buffer[..written]),
            possibly_truncated: written == capacity,
        })
    }

    /// Whether the overlay already has this clan chat's window open.
    pub fn is_clan_chat_window_open_in_steam(&self, clan_chat: SteamId) -> bool {
        unsafe {
            sys::SteamAPI_ISteamFriends_IsClanChatWindowOpenInSteam(self.friends, clan_chat.raw())
        }
    }

    /// Opens the clan chat in the Steam overlay rather than in the game.
    pub fn open_clan_chat_window_in_steam(&self, clan_chat: SteamId) -> bool {
        unsafe {
            sys::SteamAPI_ISteamFriends_OpenClanChatWindowInSteam(self.friends, clan_chat.raw())
        }
    }

    /// Closes a clan chat window previously opened in the overlay.
    pub fn close_clan_chat_window_in_steam(&self, clan_chat: SteamId) -> bool {
        unsafe {
            sys::SteamAPI_ISteamFriends_CloseClanChatWindowInSteam(self.friends, clan_chat.raw())
        }
    }
}
