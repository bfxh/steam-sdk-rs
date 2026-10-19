//! Who is inside one of Steam's "sources": a clan, a chat room, a lobby or a game server.
//!
//! Steam frames the whole face in four lines (`isteamfriends.h:256-259`): "iterators for getting
//! users in a chat room, lobby, game server or clan" and "steamIDSource can be the steamID of a
//! group, game server, lobby or chat room". The two lines between them are constraints on what the
//! numbers mean -- "note that large clans that cannot be iterated by the local user" and "note that
//! the current user must be in a lobby to retrieve CSteamIDs of other users in that lobby" -- and
//! neither says what a count or an entry comes back as when the source cannot be iterated. So the
//! three decisions below answer only for this wrapper's side of the boundary; what Steam actually
//! returns for an uniterable clan needs a live client and is registered as unverified.

use super::*;
use std::os::raw::c_int;

/// Which seat of Steam's list the caller named, refused when it cannot be handed over as its `int`.
///
/// This face is what Steam calls "iterators for getting users in a chat room, lobby, game server or
/// clan" (`isteamfriends.h:256`), and the seat is `int iFriend` (`:261`). An index beyond
/// `c_int::MAX` would reach Steam as a different number through a cast, and Steam would then answer
/// a question nobody asked -- this is the same rule `groups.rs` applies to its own iterator.
fn source_index(raw: usize) -> Option<c_int> {
    c_int::try_from(raw).ok()
}

/// How many, from a count Steam declares as a signed `int`.
///
/// The declaration is `virtual int GetFriendCountFromSource( CSteamID steamIDSource )`
/// (`isteamfriends.h:260`) of something Steam describes as "steamIDSource can be the steamID of a
/// group, game server, lobby or chat room" (`:259`), and it says nothing about out-of-range answers.
/// Zero is the only reading that cannot lie: it is what a source with nobody in it already looks like
/// to every caller of this module, while a negative cast to `usize` becomes the largest number the
/// type holds and then gets used as a loop bound.
fn source_count(raw: c_int) -> usize {
    usize::try_from(raw).unwrap_or(0)
}

/// A member, or nothing when Steam hands back the value its own default constructor writes.
///
/// `CSteamID()` -- Steam's own label on it is "Purpose: Constructor" (`steamclientpublic.h:489`) --
/// sets all four of its components to zero (`:491-497`), and the two
/// enums among them are the invalid ones: `k_EAccountTypeInvalid = 0` (`steamclientpublic.h:230`)
/// and `k_EUniverseInvalid = 0` (`steamuniverse.h:17`). Every field of a packed union holding zero is
/// the 64-bit value zero, so a bare `0` out of `GetFriendFromSourceByIndex` (`isteamfriends.h:261`)
/// is a `CSteamID` nobody ever put anything into. That is Steam's encoding, not a rule this crate
/// invented -- which is more than the header states for the index-based reads in `groups.rs`, where
/// no such default value is declared and the entries are therefore forwarded untouched.
fn id_or_unset(raw: u64) -> Option<SteamId> {
    if raw == 0 {
        None
    } else {
        Some(SteamId::from_raw(raw))
    }
}

impl Friends {
    /// How many users Steam will iterate for this source.
    pub fn friend_count_from_source(&self, source: SteamId) -> usize {
        let raw = unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendCountFromSource(self.friends, source.raw())
        };
        source_count(raw)
    }

    /// The user at this position in the source's list, or `None` past its end.
    ///
    /// `None` covers both Steam's unset `CSteamID` and an index this wrapper refuses to hand over
    /// as C's `int`; walk with [`Friends::friend_count_from_source`], which is the length Steam
    /// publishes for the same source.
    pub fn friend_from_source(&self, source: SteamId, index: usize) -> Option<SteamId> {
        let index = source_index(index)?;
        let raw = unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendFromSourceByIndex(
                self.friends,
                source.raw(),
                index,
            )
        };
        id_or_unset(raw)
    }

    /// Whether the local user can see this user as a member of, or inside, this source.
    ///
    /// Steam's line is "returns true if the local user can see that steamIDUser is a member or in
    /// steamIDSource" (`isteamfriends.h:263`), so the answer is one bit and `false` is a real answer
    /// rather than a failure -- there is no error channel here to invent one for.
    pub fn is_user_in_source(&self, user: SteamId, source: SteamId) -> bool {
        unsafe {
            sys::SteamAPI_ISteamFriends_IsUserInSource(self.friends, user.raw(), source.raw())
        }
    }
}

#[cfg(test)]
mod source_tests {
    //! The three readings of what Steam hands back. Which users are in which source needs a live
    //! client and is registered as unverified, along with what Steam returns for the clans its own
    //! comment says this user cannot iterate.

    use super::{id_or_unset, source_count, source_index};
    use std::os::raw::c_int;

    /// An index Steam cannot receive as its `int` is refused rather than cast into some other seat.
    #[test]
    fn an_index_that_cannot_be_a_c_int_is_never_asked_about() {
        assert_eq!(source_index(0), Some(0));
        assert_eq!(source_index(c_int::MAX as usize), Some(c_int::MAX));
        assert_eq!(source_index(c_int::MAX as usize + 1), None);
        assert_eq!(source_index(usize::MAX), None);
    }

    /// The count is a length by the time callers use it, so the wrap this refuses is the whole bug
    /// class: `-1` read as `usize` is 18446744073709551615, and a loop over it looks like a huge list
    /// rather than an answer nobody can iterate.
    #[test]
    fn a_signed_count_is_clamped_at_zero_instead_of_wrapped() {
        assert_eq!(source_count(0), 0);
        assert_eq!(source_count(12), 12);
        assert_eq!(source_count(-1), 0);
        assert_eq!(source_count(c_int::MIN), 0);
        assert_ne!(source_count(-1), usize::MAX);
    }

    /// Steam's all-zero `CSteamID` becomes `None`, and every other number survives intact -- the
    /// point of the test is that the refusal is about the value Steam's own constructor writes, not
    /// about ids this crate would rather not pass along.
    #[test]
    fn steam_s_unset_steam_id_is_nothing_rather_than_an_id() {
        assert_eq!(id_or_unset(0), None);
        assert_eq!(
            id_or_unset(761847234567890123).map(|id| id.raw()),
            Some(761847234567890123)
        );
        assert_eq!(id_or_unset(1).map(|id| id.raw()), Some(1));
    }
}
