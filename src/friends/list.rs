//! Who is on the list and what Steam says about them: the friend and co-play rosters, one
//! lookup by id, the request that fills in name and avatar, and the persona state,
//! relationship and Steam level reads.

use super::*;

impl Friends {
    /// Returns the (display) name of the current user
    pub fn name(&self) -> String {
        unsafe {
            let name = sys::SteamAPI_ISteamFriends_GetPersonaName(self.friends);
            let name = CStr::from_ptr(name);
            name.to_string_lossy().into_owned()
        }
    }

    pub fn get_friends(&self, flags: FriendFlags) -> Vec<Friend> {
        unsafe {
            let count = sys::SteamAPI_ISteamFriends_GetFriendCount(self.friends, flags.bits() as _);
            if count == -1 {
                return Vec::new();
            }
            let mut friends = Vec::with_capacity(count as usize);
            for idx in 0..count {
                let friend = SteamId(sys::SteamAPI_ISteamFriends_GetFriendByIndex(
                    self.friends,
                    idx,
                    flags.bits() as _,
                ));
                friends.push(self.get_friend(friend));
            }

            friends
        }
    }

    /// Returns recently played with players list
    pub fn get_coplay_friends(&self) -> Vec<Friend> {
        unsafe {
            let count = sys::SteamAPI_ISteamFriends_GetCoplayFriendCount(self.friends);
            if count == -1 {
                return Vec::new();
            }
            let mut friends = Vec::with_capacity(count as usize);
            for idx in 0..count {
                let friend = SteamId(sys::SteamAPI_ISteamFriends_GetCoplayFriend(
                    self.friends,
                    idx,
                ));
                friends.push(self.get_friend(friend));
            }
            friends
        }
    }

    pub fn get_friend(&self, friend: SteamId) -> Friend {
        Friend::new(friend, self.friends, self.inner.clone())
    }

    pub fn request_user_information(&self, user: SteamId, name_only: bool) -> bool {
        unsafe {
            sys::SteamAPI_ISteamFriends_RequestUserInformation(self.friends, user.0, name_only)
        }
    }

    /// The local player's own persona state, as Steam currently sees it.
    pub fn persona_state(&self) -> FriendState {
        unsafe { persona_state_from(sys::SteamAPI_ISteamFriends_GetPersonaState(self.friends)) }
    }

    /// How the local player relates to `friend` (friend, blocked, ignored, ...).
    pub fn friend_relationship(&self, friend: SteamId) -> FriendRelationship {
        unsafe {
            relationship_from(sys::SteamAPI_ISteamFriends_GetFriendRelationship(
                self.friends,
                friend.raw(),
            ))
        }
    }

    /// The friend's Steam level.
    pub fn friend_steam_level(&self, friend: SteamId) -> i32 {
        unsafe { sys::SteamAPI_ISteamFriends_GetFriendSteamLevel(self.friends, friend.raw()) }
    }
}
