//! The other player: the handle a caller reads a name, a state and a rich-presence
//! table through, plus the two vocabularies Steam answers with (`FriendState`,
//! `FriendRelationship`) and the mapping tests that pin them.
//!
//! Split out of the friends module because this type grows when Steam adds a
//! per-friend query, which is a different axis from the interface methods in `mod.rs`.

use super::*;

pub struct Friend {
    id: SteamId,
    friends: *mut sys::ISteamFriends,
    _inner: Arc<Inner>,
}

impl Debug for Friend {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Friend({:?})", self.id)
    }
}

impl Friend {
    pub(crate) fn new(id: SteamId, friends: *mut sys::ISteamFriends, inner: Arc<Inner>) -> Self {
        Self {
            id,
            friends,
            _inner: inner,
        }
    }

    pub fn id(&self) -> SteamId {
        self.id
    }

    pub fn name(&self) -> String {
        unsafe {
            let name = sys::SteamAPI_ISteamFriends_GetFriendPersonaName(self.friends, self.id.0);
            let name = CStr::from_ptr(name);
            name.to_string_lossy().into_owned()
        }
    }
    /// Gets the nickname that the current user has set for the specified user.
    pub fn nick_name(&self) -> Option<String> {
        unsafe {
            let name = sys::SteamAPI_ISteamFriends_GetPlayerNickname(self.friends, self.id.0);
            if name.is_null() {
                return None;
            }

            let name = CStr::from_ptr(name);
            if name.is_empty() {
                None
            } else {
                Some(name.to_string_lossy().into_owned())
            }
        }
    }

    pub fn state(&self) -> FriendState {
        unsafe {
            persona_state_from(sys::SteamAPI_ISteamFriends_GetFriendPersonaState(
                self.friends,
                self.id.0,
            ))
        }
    }

    /// Returns information about the game the player is current playing if any
    pub fn game_played(&self) -> Option<FriendGame> {
        unsafe {
            let mut info: sys::FriendGameInfo_t = std::mem::zeroed();
            if sys::SteamAPI_ISteamFriends_GetFriendGamePlayed(self.friends, self.id.0, &mut info) {
                Some(FriendGame {
                    game: GameId(info.m_gameID.__bindgen_anon_1.m_ulGameID),
                    game_address: info.m_unGameIP.into(),
                    game_port: info.m_usGamePort,
                    query_port: info.m_usQueryPort,
                    lobby: LobbyId(info.m_steamIDLobby.m_steamid.m_unAll64Bits),
                })
            } else {
                None
            }
        }
    }
    /// Gets the app ID of the game that user played with someone on their recently-played-with list.
    pub fn coplay_game_played(&self) -> AppId {
        unsafe {
            let app_id = sys::SteamAPI_ISteamFriends_GetFriendCoplayGame(self.friends, self.id.0);
            AppId(app_id)
        }
    }

    /// Gets the timestamp of when the user played with someone on their recently-played-with list.
    pub fn coplay_time(&self) -> i32 {
        unsafe { sys::SteamAPI_ISteamFriends_GetFriendCoplayTime(self.friends, self.id.0) }
    }

    /// Returns a small (32x32) avatar for the user in RGBA format
    pub fn small_avatar(&self) -> Option<Vec<u8>> {
        unsafe {
            let utils = sys::SteamAPI_SteamUtils_v010();
            let img = sys::SteamAPI_ISteamFriends_GetSmallFriendAvatar(self.friends, self.id.0);
            if img == 0 {
                return None;
            }
            let mut width = 0;
            let mut height = 0;
            if !sys::SteamAPI_ISteamUtils_GetImageSize(utils, img, &mut width, &mut height) {
                return None;
            }
            let mut dest = vec![0; 32 * 32 * 4];
            if !sys::SteamAPI_ISteamUtils_GetImageRGBA(utils, img, dest.as_mut_ptr(), 32 * 32 * 4) {
                return None;
            }
            Some(dest)
        }
    }

    /// Returns a medium (64x64) avatar for the user in RGBA format
    pub fn medium_avatar(&self) -> Option<Vec<u8>> {
        unsafe {
            let utils = sys::SteamAPI_SteamUtils_v010();
            let img = sys::SteamAPI_ISteamFriends_GetMediumFriendAvatar(self.friends, self.id.0);
            if img == 0 {
                return None;
            }
            let mut width = 0;
            let mut height = 0;
            if !sys::SteamAPI_ISteamUtils_GetImageSize(utils, img, &mut width, &mut height) {
                return None;
            }
            let mut dest = vec![0; 64 * 64 * 4];
            if !sys::SteamAPI_ISteamUtils_GetImageRGBA(utils, img, dest.as_mut_ptr(), 64 * 64 * 4) {
                return None;
            }
            Some(dest)
        }
    }

    /// Returns a large (184x184) avatar for the user in RGBA format
    pub fn large_avatar(&self) -> Option<Vec<u8>> {
        unsafe {
            let utils = sys::SteamAPI_SteamUtils_v010();
            let img = sys::SteamAPI_ISteamFriends_GetLargeFriendAvatar(self.friends, self.id.0);
            if img == 0 {
                return None;
            }
            let mut width = 0;
            let mut height = 0;
            if !sys::SteamAPI_ISteamUtils_GetImageSize(utils, img, &mut width, &mut height) {
                return None;
            }
            let mut dest = vec![0; 184 * 184 * 4];
            if !sys::SteamAPI_ISteamUtils_GetImageRGBA(utils, img, dest.as_mut_ptr(), 184 * 184 * 4)
            {
                return None;
            }
            Some(dest)
        }
    }

    /// Checks if the user meets the specified criteria. (Friends, blocked, users on the same server, etc)
    pub fn has_friend(&self, flags: FriendFlags) -> bool {
        unsafe { sys::SteamAPI_ISteamFriends_HasFriend(self.friends, self.id.0, flags.bits() as _) }
    }

    /// Invites a friend or clan member to the current game using a special invite string.
    /// If the target user accepts the invite then the ConnectString gets added to the command-line when launching the game.
    /// If the game is already running for that user, then they will receive a GameRichPresenceJoinRequested_t callback with the connect string.
    pub fn invite_user_to_game(&self, connect_string: &str) {
        unsafe {
            let connect_string = CString::new(connect_string).unwrap();
            sys::SteamAPI_ISteamFriends_InviteUserToGame(
                self.friends,
                self.id.0,
                connect_string.as_ptr(),
            );
        }
    }

    /// Mark a target user as 'played with'.
    /// NOTE: The current user must be in game with the other player for the association to work.
    pub fn set_played_with(&self) {
        unsafe {
            sys::SteamAPI_ISteamFriends_SetPlayedWith(self.friends, self.id.0);
        }
    }

    /// Get a Rich Presence value from a specified friend.
    pub fn rich_presence(&self, key: &str) -> Option<String> {
        let key = CString::new(key).unwrap();
        let value = unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendRichPresence(self.friends, self.id.0, key.as_ptr())
        };
        if value.is_null() {
            return None;
        }
        let cstr = unsafe { CStr::from_ptr(value) };
        if cstr.is_empty() {
            return None;
        }
        Some(cstr.to_string_lossy().into_owned())
    }
}

/// Maps Steam's persona-state answer.
///
/// `k_EPersonaStateMax` is a real answer for a state this binding does not name
/// -- the previous code reached `unreachable!()` on it, which panicked.
pub(crate) fn persona_state_from(state: sys::EPersonaState) -> FriendState {
    match state {
        sys::EPersonaState::k_EPersonaStateOffline => FriendState::Offline,
        sys::EPersonaState::k_EPersonaStateOnline => FriendState::Online,
        sys::EPersonaState::k_EPersonaStateInvisible => FriendState::Invisible,
        sys::EPersonaState::k_EPersonaStateBusy => FriendState::Busy,
        sys::EPersonaState::k_EPersonaStateAway => FriendState::Away,
        sys::EPersonaState::k_EPersonaStateSnooze => FriendState::Snooze,
        sys::EPersonaState::k_EPersonaStateLookingToPlay => FriendState::LookingToPlay,
        sys::EPersonaState::k_EPersonaStateLookingToTrade => FriendState::LookingToTrade,
        _ => FriendState::Unknown,
    }
}

/// How the local player relates to another user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FriendRelationship {
    /// Not friends, and no request pending.
    None,
    Blocked,
    /// They asked to be friends; a reply is owed.
    RequestRecipient,
    Friend,
    /// We asked to be friends; waiting on their reply.
    RequestInitiator,
    Ignored,
    /// Ignored, but still a friend.
    IgnoredFriend,
    /// A relationship value this binding does not name.
    Unknown,
}

pub(crate) fn relationship_from(relationship: sys::EFriendRelationship) -> FriendRelationship {
    match relationship {
        sys::EFriendRelationship::k_EFriendRelationshipNone => FriendRelationship::None,
        sys::EFriendRelationship::k_EFriendRelationshipBlocked => FriendRelationship::Blocked,
        sys::EFriendRelationship::k_EFriendRelationshipRequestRecipient => {
            FriendRelationship::RequestRecipient
        }
        sys::EFriendRelationship::k_EFriendRelationshipFriend => FriendRelationship::Friend,
        sys::EFriendRelationship::k_EFriendRelationshipRequestInitiator => {
            FriendRelationship::RequestInitiator
        }
        sys::EFriendRelationship::k_EFriendRelationshipIgnored => FriendRelationship::Ignored,
        sys::EFriendRelationship::k_EFriendRelationshipIgnoredFriend => {
            FriendRelationship::IgnoredFriend
        }
        // Includes `k_EFriendRelationshipSuggested_DEPRECATED` (Valve retired it)
        // and `k_EFriendRelationshipMax`, so neither can panic the caller.
        _ => FriendRelationship::Unknown,
    }
}

#[cfg(test)]
mod state_mapping_tests {
    use super::*;

    #[test]
    fn steam_owned_strings_survive_a_null_answer() {
        // A null answer is Steam saying "not cached yet"; reading it as an empty
        // name would be a different claim than the one it made.
        let null = std::ptr::null::<std::os::raw::c_char>();
        assert_eq!(
            unsafe { steam_string(null, |s| s.to_string_lossy().into_owned()) },
            None
        );

        let name = CString::new("Delta Squad").unwrap();
        assert_eq!(
            unsafe { steam_string(name.as_ptr(), |s| s.to_string_lossy().into_owned()) }.as_deref(),
            Some("Delta Squad")
        );
    }
    #[test]
    fn unknown_persona_state_is_not_a_panic() {
        // The previous mapping ended in `unreachable!()`, and Steam does answer
        // with the sentinel for states this binding does not name.
        assert_eq!(
            persona_state_from(sys::EPersonaState::k_EPersonaStateMax),
            FriendState::Unknown
        );
        let named = [
            (
                sys::EPersonaState::k_EPersonaStateOffline,
                FriendState::Offline,
            ),
            (
                sys::EPersonaState::k_EPersonaStateOnline,
                FriendState::Online,
            ),
            (
                sys::EPersonaState::k_EPersonaStateInvisible,
                FriendState::Invisible,
            ),
            (sys::EPersonaState::k_EPersonaStateBusy, FriendState::Busy),
            (sys::EPersonaState::k_EPersonaStateAway, FriendState::Away),
            (
                sys::EPersonaState::k_EPersonaStateSnooze,
                FriendState::Snooze,
            ),
            (
                sys::EPersonaState::k_EPersonaStateLookingToPlay,
                FriendState::LookingToPlay,
            ),
            (
                sys::EPersonaState::k_EPersonaStateLookingToTrade,
                FriendState::LookingToTrade,
            ),
        ];
        let mut seen = vec![];
        for (raw, expected) in named {
            assert_eq!(persona_state_from(raw), expected);
            seen.push(persona_state_from(raw));
        }
        // Distinct Steam states must not collapse into one game-visible state.
        for i in 0..seen.len() {
            for j in (i + 1)..seen.len() {
                assert_ne!(seen[i], seen[j]);
            }
        }
    }

    #[test]
    fn relationships_map_onto_distinct_values_and_the_retired_one_is_unknown() {
        use sys::EFriendRelationship as R;
        let mapped: Vec<FriendRelationship> = [
            R::k_EFriendRelationshipNone,
            R::k_EFriendRelationshipBlocked,
            R::k_EFriendRelationshipRequestRecipient,
            R::k_EFriendRelationshipFriend,
            R::k_EFriendRelationshipRequestInitiator,
            R::k_EFriendRelationshipIgnored,
            R::k_EFriendRelationshipIgnoredFriend,
        ]
        .iter()
        .map(|raw| relationship_from(*raw))
        .collect();
        assert_eq!(mapped[0], FriendRelationship::None);
        assert_eq!(mapped[3], FriendRelationship::Friend);
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(mapped[i], mapped[j]);
            }
        }
        assert_eq!(
            relationship_from(R::k_EFriendRelationshipSuggested_DEPRECATED),
            FriendRelationship::Unknown,
            "a retired Valve value must not masquerade as a live relationship"
        );
        assert_eq!(
            relationship_from(R::k_EFriendRelationshipMax),
            FriendRelationship::Unknown
        );
    }

    #[test]
    fn game_id_field_read_covers_the_whole_union() {
        // `game_played` reads `m_ulGameID` instead of transmuting the union. That is
        // only equivalent while the union is exactly one `u64` wide: a wider binding
        // would make the field read silently drop bytes, which no compile error would
        // catch. This assert is the lock for that assumption.
        assert_eq!(
            std::mem::size_of::<sys::CGameID>(),
            std::mem::size_of::<u64>(),
            "CGameID must stay a single 64-bit union for m_ulGameID to be the whole value"
        );
        let id = sys::CGameID {
            __bindgen_anon_1: sys::CGameID__bindgen_ty_1 {
                m_ulGameID: 0x0100_0000_0000_0421,
            },
        };
        assert_eq!(
            unsafe { id.__bindgen_anon_1.m_ulGameID },
            0x0100_0000_0000_0421
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FriendState {
    /// Steam answered with a state this binding does not name (including its
    /// `k_EPersonaStateMax` sentinel) rather than one of the known values below.
    Unknown,
    Offline,
    Online,
    Invisible,
    Busy,
    Away,
    Snooze,
    LookingToTrade,
    LookingToPlay,
}
