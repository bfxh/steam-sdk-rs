//! The payloads `ISteamFriends` fires as callbacks: clan counters, the officer list,
//! follower count, rich-presence and persona-state changes, and the three overlay
//! join requests.
//!
//! Split out of the friends module because each entry here is a plain struct plus its
//! `impl_callback!` bridge, and the list grows when Steam adds a callback -- not when
//! `Friends` gains a method.

use super::*;

/// Cached clan activity from [`Friends::clan_activity`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClanActivity {
    pub online: i32,
    pub in_game: i32,
    pub chatting: i32,
}

/// Answers [`Friends::request_clan_officer_list`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClanOfficerListResponse {
    pub clan: SteamId,
    /// How many officers to read back with [`Friends::clan_officers`].
    pub officer_count: i32,
    pub success: bool,
}

/// Answers [`Friends::follower_count`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FollowerCount {
    pub user: SteamId,
    pub count: i32,
}

/// Answers [`Friends::is_following`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsFollowingResult {
    pub target: SteamId,
    pub following: bool,
}

/// A friend's rich presence changed, typically after
/// [`Friends::request_friend_rich_presence`] or while they play.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FriendRichPresenceUpdate {
    pub friend: SteamId,
    /// The game the presence belongs to; Steam reports the current app id here.
    pub app_id: AppId,
}

impl_callback!(cb: FriendRichPresenceUpdate_t => FriendRichPresenceUpdate {
    Self {
        friend: SteamId::from_raw(cb.m_steamIDFriend.m_steamid.m_unAll64Bits),
        app_id: AppId(cb.m_nAppID),
    }
});

/// Copies a Steam-owned C string out before the caller's next query, treating a
/// null pointer as "not available" instead of dereferencing it.
pub(crate) unsafe fn steam_string<T>(
    raw: *const std::os::raw::c_char,
    read: impl FnOnce(&CStr) -> T,
) -> Option<T> {
    if raw.is_null() {
        return None;
    }
    Some(read(CStr::from_ptr(raw)))
}

/// Information about a friend's current state in a game
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct FriendGame {
    /// The id of the game that the friend is
    /// playing
    pub game: GameId,
    /// The address of the server the player is in
    pub game_address: Ipv4Addr,
    /// The game port of the server the player is in
    pub game_port: u16,
    /// The query port of the server the player is in
    pub query_port: u16,
    /// Optional id of the lobby the player is in
    pub lobby: LobbyId,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PersonaStateChange {
    pub steam_id: SteamId,
    pub flags: PersonaChange,
}

impl_callback!(cb: PersonaStateChange_t => PersonaStateChange {
    Self {
        steam_id: SteamId(cb.m_ulSteamID),
        flags: PersonaChange::from_bits_truncate(cb.m_nChangeFlags as i32),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GameOverlayActivated {
    pub active: bool,
}

impl_callback!(cb: GameOverlayActivated_t => GameOverlayActivated {
    Self {
        active: cb.m_bActive == 1,
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GameLobbyJoinRequested {
    pub lobby_steam_id: LobbyId,
    pub friend_steam_id: SteamId,
}

impl_callback!(cb: GameLobbyJoinRequested_t => GameLobbyJoinRequested {
    Self {
        lobby_steam_id: LobbyId(cb.m_steamIDLobby.m_steamid.m_unAll64Bits),
        friend_steam_id: SteamId(cb.m_steamIDFriend.m_steamid.m_unAll64Bits),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GameRichPresenceJoinRequested {
    /// If you are joining a friend/being invited from a friend, this `SteamId` will be of said friend.
    /// If it's not coming from a friend, this `SteamId` will be invalid, which you can check with the `.is_invalid()`
    /// method.
    pub friend_steam_id: SteamId,
    /// the connect string, holding custom data to join a game or friend
    pub connect: String,
}

impl_callback!(cb: GameRichPresenceJoinRequested_t => GameRichPresenceJoinRequested {
    // "char m_rgchConnect[k_cchMaxRichPresenceValueLength];" (isteamfriends.h:570, with
    // "enum { k_cchMaxRichPresenceValueLength = 256 };" at :125) is an array inside a callback
    // payload: the header promises no terminator and no encoding. Reading it as a C string and then
    // demanding UTF-8 -- the two `.expect(..)` calls this used -- aborts the callback that was asked
    // to decode it, which is the failure mode `src/utils/strings.rs` exists to avoid.
    let connect = crate::utils::fixed_string(&cb.m_rgchConnect);

    let friend_steam_id = SteamId::from_raw(cb.m_steamIDFriend.m_steamid.m_unAll64Bits);

    GameRichPresenceJoinRequested {
        friend_steam_id,
        connect,
    }
});
