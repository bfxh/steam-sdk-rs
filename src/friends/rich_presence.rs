//! Rich presence: writing a key, clearing the whole table, reading a friend's key list back,
//! and asking Steam to fetch a friend's presence.

use super::*;

impl Friends {
    /// Set rich presence for the user. Unsets the rich presence if `value` is None or empty.
    ///
    /// See [Steam API](https://partner.steamgames.com/doc/api/ISteamFriends#SetRichPresence)
    ///
    /// # Panics
    ///
    /// Panics if the `key` or `value` str slices contain a null byte.
    pub fn set_rich_presence(&self, key: &str, value: Option<&str>) -> bool {
        let key = CString::new(key).unwrap();
        let value = value.map(|v| CString::new(v).unwrap());
        let value_ptr = value
            .as_ref()
            .map_or(std::ptr::null(), |value| value.as_ptr());
        unsafe {
            sys::SteamAPI_ISteamFriends_SetRichPresence(self.friends, key.as_ptr(), value_ptr)
        }
    }

    /// Clears all of the current user's Rich Presence key/values.
    pub fn clear_rich_presence(&self) {
        unsafe {
            sys::SteamAPI_ISteamFriends_ClearRichPresence(self.friends);
        }
    }

    /// The key names present in a friend's rich presence; feed them to
    /// [`Friend::rich_presence`] to read the values back.
    ///
    /// Steam owns the strings it returns here, so each one is copied out before
    /// the next query; a null entry is skipped rather than pushed empty.
    ///
    /// Keys only appear once Steam has the data: see
    /// [`Friends::request_friend_rich_presence`].
    pub fn rich_presence_keys(&self, friend: SteamId) -> Vec<String> {
        let count = unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendRichPresenceKeyCount(self.friends, friend.raw())
        };
        if count <= 0 {
            return Vec::new();
        }
        let mut keys = Vec::with_capacity(count as usize);
        for index in 0..count {
            let raw = unsafe {
                sys::SteamAPI_ISteamFriends_GetFriendRichPresenceKeyByIndex(
                    self.friends,
                    friend.raw(),
                    index,
                )
            };
            if raw.is_null() {
                continue;
            }
            let key = unsafe { CStr::from_ptr(raw) };
            keys.push(key.to_string_lossy().into_owned());
        }
        keys
    }

    /// Asks Steam to fetch a friend's rich presence.
    ///
    /// Without this, [`Friends::rich_presence_keys`] and
    /// [`Friend::rich_presence`] usually read nothing: Steam only pulls another
    /// user's presence on request, and then reports changes through
    /// [`FriendRichPresenceUpdate`]. Fire-and-forget, so there is no answer to
    /// check.
    pub fn request_friend_rich_presence(&self, friend: SteamId) {
        unsafe {
            sys::SteamAPI_ISteamFriends_RequestFriendRichPresence(self.friends, friend.raw());
        }
    }
}
