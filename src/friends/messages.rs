//! Friend messages: reading one by id and replying to it.

use super::*;

impl Friends {
    /// Reads back one of the friend messages Steam stopped from being shown in
    /// the overlay, after [`Friends::set_listen_for_friends_messages`].
    ///
    /// Steam announces a message as [`FriendChatMessageReceived`] and this reads
    /// that announcement's `message_id` back; `max_bytes` is the receive buffer,
    /// capped at [`MAX_CHAT_MESSAGE_BYTES`].
    ///
    /// `None` when Steam copied no bytes for that id.
    pub fn friend_message(
        &self,
        friend: SteamId,
        message_id: i32,
        max_bytes: usize,
    ) -> Option<FriendChatMessage> {
        let capacity = chat_buffer_len(max_bytes);
        let mut buffer = vec![0u8; capacity];
        let mut entry_type: i32 = 0;
        let written = unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendMessage(
                self.friends,
                friend.raw(),
                message_id,
                buffer.as_mut_ptr().cast::<c_void>(),
                // Lossless because `chat_buffer_len` caps the value.
                capacity as i32,
                &mut entry_type as *mut i32 as *mut sys::EChatEntryType,
            )
        };
        if written <= 0 {
            return None;
        }
        let written = (written as usize).min(capacity);
        Some(FriendChatMessage {
            kind: chat_entry_type_from(entry_type),
            text: chat_text(&buffer[..written]),
            possibly_truncated: written == capacity,
        })
    }

    /// Answers a friend message out of the game, while message interception is
    /// enabled.
    ///
    /// `Err(SteamError::InvalidParameter)` means the text never reached Steam
    /// (it contains an interior nul); `Ok(false)` is Steam's own refusal.
    pub fn reply_to_friend_message(&self, friend: SteamId, text: &str) -> Result<bool, SteamError> {
        let text = c_string_arg(text)?;
        Ok(unsafe {
            sys::SteamAPI_ISteamFriends_ReplyToFriendMessage(
                self.friends,
                friend.raw(),
                text.as_ptr(),
            )
        })
    }
}
