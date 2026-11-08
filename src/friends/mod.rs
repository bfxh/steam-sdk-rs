use super::*;
use crate::app::c_string_arg;
use std::net::Ipv4Addr;

mod callbacks;
mod chat;
mod friend;

pub use callbacks::*;
pub use chat::*;
pub use friend::*;
pub use groups::FriendsGroupID;
pub use profile_items::{ProfileItemProperty, ProfileItemPropertyKind, ProfileItemType};
// The three readers above are helpers, not API: visible to this module, not re-exported.
use chat::{chat_buffer_len, chat_entry_type_from, chat_text};

mod clan_chat;
mod clans;
mod flags;
mod followers;
mod groups;
mod history;
mod list;
mod messages;
mod overlay;
mod profile_items;
mod rich_presence;
mod sources;

pub use flags::*;
pub use overlay::*;

/// Access to the steam friends interface
pub struct Friends {
    pub(crate) friends: *mut sys::ISteamFriends,
    pub(crate) inner: Arc<Inner>,
}

impl Friends {
    /// Tells Steam the player is speaking in in-game voice, so the overlay does
    /// not duck game audio twice.
    pub fn set_in_game_voice_speaking(&self, user: SteamId, speaking: bool) {
        unsafe {
            sys::SteamAPI_ISteamFriends_SetInGameVoiceSpeaking(self.friends, user.raw(), speaking)
        }
    }

    /// Chats holding unread priority messages (group chats and the like).
    pub fn unread_priority_chat_count(&self) -> i32 {
        unsafe { sys::SteamAPI_ISteamFriends_GetNumChatsWithUnreadPriorityMessages(self.friends) }
    }

    /// Intercepts friend messages so the game can render them; while enabled,
    /// Steam does not show them itself.
    ///
    /// Turn this on only while something is reading: each hidden message arrives
    /// as [`FriendChatMessageReceived`], is read back with
    /// [`Friends::friend_message`], and is answered with
    /// [`Friends::reply_to_friend_message`]. Enabling it with no reader on the
    /// other side swallows the messages.
    ///
    /// The return value is Steam's acceptance.
    pub fn set_listen_for_friends_messages(&self, intercept: bool) -> bool {
        unsafe { sys::SteamAPI_ISteamFriends_SetListenForFriendsMessages(self.friends, intercept) }
    }

    /// Opens the overlay's Remote Play Together invite dialog for a lobby.
    pub fn open_remote_play_together_invite_dialog(&self, lobby: SteamId) {
        unsafe {
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayRemotePlayTogetherInviteDialog(
                self.friends,
                lobby.raw(),
            )
        }
    }
}
