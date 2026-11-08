//! The user's own friend groups (Steam's word is "tags"): how many there are, which index holds
//! which id, what a group is called, and who is in it.
//!
//! Steam's block introducing these is at `isteammatchmaking`'s friend-list neighbour -- it reads
//! "friend grouping (tag) apis" (`isteamfriends.h:228`) -- and this SDK exposes **only the five
//! readers**: measured, `SteamAPI_ISteamFriends_CreateFriendsGroup`,
//! `..._RemoveFriendsGroup` and `..._AddFriendToFriendsGroup` have zero occurrences in the generated
//! layer, so there is no mutator here to wrap and none to pretend this module covers.
//!
//! Which groups exist, and who is in them, needs a running client -- registered as unverified. What
//! is judged below without one is the four places where a number or a pointer arriving from Steam
//! turns into a Rust value, because each of those is a decision this crate makes, not a fact Steam
//! states.

use super::*;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

/// One of the user's friend groups, as Steam numbers it.
///
/// `i16` because that is the declared width: "typedef int16 FriendsGroupID_t"
/// (`isteamfriends.h:40`). The value Steam uses for a group that is not there,
/// `k_FriendsGroupID_Invalid`, is declared at `:43` and is `-1`; the boundary
/// methods here return `Option`, so a `-1` obtained by constructing this newtype directly is still
/// answered by the same table as one obtained from Steam.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub struct FriendsGroupID(pub i16);

/// Which index Steam's list is being asked for, refused when it cannot be expressed as its `int`.
///
/// The header's own answer to an index that is merely too large is `k_FriendsGroupID_Invalid`
/// ("invalid indices return k_FriendsGroupID_Invalid", `isteamfriends.h:231`) -- that is Steam's
/// judgement and stays Steam's. This one is different: an index that does not fit a C `int` would
/// reach Steam as some *other* number through a cast, which is asking a question nobody asked.
fn group_index(raw: usize) -> Option<c_int> {
    c_int::try_from(raw).ok()
}

/// A group id, or nothing when Steam said its marker.
fn group_id_or_none(raw: i16) -> Option<FriendsGroupID> {
    if raw == sys::k_FriendsGroupID_Invalid {
        None
    } else {
        Some(FriendsGroupID(raw))
    }
}

/// How many, from a count Steam declares as a signed `int`.
///
/// Steam asks for "returns the number of friends groups" (`isteamfriends.h:229-230`) and, for one
/// group, "returns the number of members in a given friends group" (`:235-236`). Both are `int`, and
/// neither says what a negative would mean. Zero is the only safe
/// reading: it is what no groups and no members already look like, and a negative turned into a
/// `usize` would become the largest number the type holds and then be used as a buffer length.
fn count_or_zero(raw: c_int) -> usize {
    usize::try_from(raw).unwrap_or(0)
}

/// A name, or nothing when Steam returned a null pointer.
///
/// "returns the name for the given friends group (NULL in the case of invalid friends group IDs)"
/// (`isteamfriends.h:233`) -- so the null is an answer, not a failure, and it is reported as one.
fn name_or_none(raw: *const c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(raw) }
            .to_string_lossy()
            .into_owned(),
    )
}

/// The slots to hand back from a member list, given how many Steam was told it could write.
///
/// The header's array annotation says the length comes from the count call
/// (`STEAM_OUT_ARRAY_CALL(nMembersCount, GetFriendsGroupMembersCount, friendsGroupID )`,
/// `isteamfriends.h:238`), which is the pairing this method uses. Its comment about the remainder
/// is "if fewer exist than requested those positions' SteamIDs will be invalid" (`:237`) -- but the
/// predicate that decides which ones are invalid, `CSteamID::BValid()`, is not in the vendored SDK
/// (measured: `steamclientpublic.h:796` offers only `BValidExternalSteamID()`, declared and defined
/// inside the closed library). So this crate forwards the number it asked for and does not invent a
/// validity rule to drop entries with: a group that changed size between the two calls shows up as
/// trailing SteamIDs, and [`SteamId::is_invalid`] -- which decodes the account-type field that same
/// default constructor writes (`steamclientpublic.h:494`) -- is the comparison a caller has, without
/// this crate claiming it is Steam's `BValid()`.
fn member_slots(count: c_int) -> usize {
    count_or_zero(count)
}

impl Friends {
    /// How many friend groups the user has.
    pub fn friends_group_count(&self) -> usize {
        let raw = unsafe { sys::SteamAPI_ISteamFriends_GetFriendsGroupCount(self.friends) };
        count_or_zero(raw)
    }

    /// The id of the group at this position in Steam's list, or `None`.
    ///
    /// `None` means one of two things, and both are Steam's answer rather than this wrapper's: the
    /// index is past the end ("invalid indices return k_FriendsGroupID_Invalid",
    /// `isteamfriends.h:231`), or the number could not be handed over as C's `int` at all. Walk the
    /// list with [`Friends::friends_group_count`], which is the length Steam publishes for it.
    pub fn friends_group_id(&self, index: usize) -> Option<FriendsGroupID> {
        let index = group_index(index)?;
        let raw =
            unsafe { sys::SteamAPI_ISteamFriends_GetFriendsGroupIDByIndex(self.friends, index) };
        group_id_or_none(raw)
    }

    /// What the user calls this group.
    ///
    /// The empty string is a name Steam could have returned and is forwarded as one; `None` is the
    /// pointer Steam calls "NULL in the case of invalid friends group IDs" (`isteamfriends.h:233`).
    pub fn friends_group_name(&self, group: FriendsGroupID) -> Option<String> {
        let raw = unsafe { sys::SteamAPI_ISteamFriends_GetFriendsGroupName(self.friends, group.0) };
        name_or_none(raw)
    }

    /// How many members Steam counts in this group.
    pub fn friends_group_member_count(&self, group: FriendsGroupID) -> usize {
        let raw = unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendsGroupMembersCount(self.friends, group.0)
        };
        count_or_zero(raw)
    }

    /// The group's members, sized by Steam's own count for it.
    ///
    /// The count and the list are two calls, so a group that changes size in between is possible, and
    /// Steam covers it with "if fewer exist than requested those positions' SteamIDs will be invalid"
    /// (`isteamfriends.h:237-238`). `member_slots` records why this crate does not decide validity on
    /// Steam's behalf.
    pub fn friends_group_members(&self, group: FriendsGroupID) -> Vec<SteamId> {
        let requested = unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendsGroupMembersCount(self.friends, group.0)
        };
        let slots = member_slots(requested);
        if slots == 0 {
            return Vec::new();
        }
        // `SteamId` is a wrapper over `u64` with no invalid-bit pattern of its own, so zeroed memory
        // is a well defined value here: Steam overwrites each slot it was told about, and the ones it
        // does not are SteamIDs it has already called invalid in the sentence above.
        let mut members = vec![SteamId(0); slots];
        unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendsGroupMembersList(
                self.friends,
                group.0,
                members.as_mut_ptr().cast(),
                requested,
            );
        }
        members
    }
}

#[cfg(test)]
mod group_tests {
    //! The four readings of what Steam hands back. Which groups exist, and who is in them, needs a
    //! live client and is registered as unverified -- including the sizing rule in
    //! `friends_group_members`, which is a pairing of two calls no test can reach.

    use super::{
        count_or_zero, group_id_or_none, group_index, member_slots, name_or_none, FriendsGroupID,
    };
    use std::os::raw::c_int;
    use steamworks_sys as sys;

    /// An index that cannot be expressed as C's `int` is refused before anything is asked, because a
    /// cast would have handed Steam a different number than the caller named.
    #[test]
    fn an_index_that_does_not_fit_steam_is_never_asked_about() {
        assert_eq!(group_index(0), Some(0));
        assert_eq!(group_index(1), Some(1));
        assert_eq!(group_index(c_int::MAX as usize), Some(c_int::MAX));
        assert_eq!(group_index(c_int::MAX as usize + 1), None);
        assert_eq!(group_index(usize::MAX), None);
    }

    /// Steam's invalid marker becomes `None`; the ids either side of it stay distinguishable, because
    /// the header's list is signed and `-2` is somebody else's answer.
    #[test]
    fn steam_s_no_such_group_is_nothing_rather_than_a_negative_id() {
        assert_eq!(group_id_or_none(sys::k_FriendsGroupID_Invalid), None);
        assert_eq!(group_id_or_none(0), Some(FriendsGroupID(0)));
        assert_eq!(group_id_or_none(7), Some(FriendsGroupID(7)));
        assert_eq!(group_id_or_none(-2), Some(FriendsGroupID(-2)));
        assert_eq!(
            sys::k_FriendsGroupID_Invalid,
            -1,
            "the boundary methods' `None` rests on this being the value the header gives it"
        );
    }

    /// A count Steam declares as signed is clamped at zero and never wrapped: `usize::try_from` is
    /// the whole difference between "no groups" and a buffer length of 18446744073709551615.
    #[test]
    fn a_signed_count_becomes_a_length_without_wrapping() {
        assert_eq!(count_or_zero(0), 0);
        assert_eq!(count_or_zero(3), 3);
        assert_eq!(count_or_zero(c_int::MAX), c_int::MAX as usize);
        assert_eq!(
            count_or_zero(-1),
            0,
            "a negative count is not a buffer length"
        );
        assert_eq!(count_or_zero(c_int::MIN), 0);
        // The member list uses the same reading, so the two cannot drift apart.
        assert_eq!(member_slots(-4), 0);
        assert_eq!(member_slots(4), 4);
    }

    /// A null pointer is Steam's answer for an id it does not know, and it must not become the empty
    /// name: those are different facts, and a caller that shows the name would show nothing for one
    /// and nothing at all for the other.
    #[test]
    fn a_name_is_read_or_its_absence_is_reported() {
        let owned = std::ffi::CString::new("buddies").unwrap();
        assert_eq!(name_or_none(owned.as_ptr()), Some("buddies".to_string()));
        let empty = std::ffi::CString::new("").unwrap();
        assert_eq!(name_or_none(empty.as_ptr()), Some(String::new()));
        assert_eq!(name_or_none(std::ptr::null()), None);
    }
}
