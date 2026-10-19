//! The user's own list of multiplayer servers, which is the part of `ISteamMatchmaking` that has
//! nothing to do with lobbies: "game server favorites storage", which "saves basic details about a
//! multiplayer game server locally" (`isteammatchmaking.h:60-61`).

use super::*;
use std::net::Ipv4Addr;
use std::os::raw::c_int;

bitflags! {
    /// Which of Steam's two local lists a stored entry belongs to.
    ///
    /// The field that carries them is described as "*punFlags specify whether the game server was
    /// stored as an explicit favorite or in the history of connections" (`isteammatchmaking.h:69`)
    /// and the two bits are declared at `:525-527`. A value with any other bit set keeps it:
    /// [`FavoriteFlags::bits`] can always show the number Steam wrote, because a flag word this
    /// wrapper cannot name is still Steam's answer about the user's list.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[repr(C)]
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    pub struct FavoriteFlags: u32 {
        /// "this game favorite entry is for the favorites list" (`isteammatchmaking.h:526`).
        const FAVORITE = sys::k_unFavoriteFlagFavorite;
        /// "this game favorite entry is for the history list" (`isteammatchmaking.h:527`).
        const HISTORY = sys::k_unFavoriteFlagHistory;
    }
}

/// One stored server: the seven values Steam fills in for `GetFavoriteGame`
/// (`isteammatchmaking.h:71`).
///
/// `ip` and `connect_port` are what Steam reports as the server's address -- "*pnIP, *pnConnPort
/// are filled in the with IP:port of the game server" (`:68`) -- and `last_played_on_server` is
/// "the Unix time the favorite was added" (`:70`). The word order of that `uint32` is not
/// specified by the header, so this type adopts the reading the crate already uses in both
/// directions (`u32::from(addr)` out, `Ipv4Addr::from(bits)` back, so `192.168.1.1` is
/// `0xC0A80101`); the reasoning, including which two call sites make it round-trip, is written out
/// on `crate::user::User::advertise_game` rather than argued again here. `last_played_on_server`
/// stays a raw `u32` because the header names no zero value: reading `0` as "1970" would be this
/// wrapper's invention, and reading it as "unset" would be a claim Steam never made.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FavoriteGame {
    pub app_id: AppId,
    pub ip: Ipv4Addr,
    pub connect_port: u16,
    pub query_port: u16,
    pub flags: FavoriteFlags,
    pub last_played_on_server: u32,
}

impl Matchmaking {
    /// How many server entries the user's local list holds.
    ///
    /// Steam answers with "the number of favorites servers the user has stored"
    /// (`isteammatchmaking.h:63`) as a signed int, and this wrapper needs it as a length, so a
    /// negative answer becomes 0 rather than wrapping into a huge one -- that clamp is
    /// `favorite_count`, and it is tested on its own.
    pub fn favorite_game_count(&self) -> usize {
        favorite_count(unsafe { sys::SteamAPI_ISteamMatchmaking_GetFavoriteGameCount(self.mm) })
    }

    /// The entry at `index`, or `None` when Steam says there is none.
    ///
    /// The stated range is "iGame is of range [0,GetFavoriteGameCount())"
    /// (`isteammatchmaking.h:67`), and the call answers `false` outside it. This wrapper reports
    /// that as `None` and reads none of the six out parameters in that case -- `favorite_entry`
    /// is where the two paths separate, and it is judged without a client. An index that cannot be a
    /// `c_int` never reaches Steam: handing it a wrapped negative would ask for an entry nobody named
    /// (`favorite_index`).
    pub fn favorite_game(&self, index: usize) -> Option<FavoriteGame> {
        let raw_index = favorite_index(index)?;
        let mut app_id = 0;
        let mut ip = 0;
        let mut connect_port = 0;
        let mut query_port = 0;
        let mut flags = 0;
        let mut last_played = 0;
        let answered = unsafe {
            sys::SteamAPI_ISteamMatchmaking_GetFavoriteGame(
                self.mm,
                raw_index,
                &mut app_id,
                &mut ip,
                &mut connect_port,
                &mut query_port,
                &mut flags,
                &mut last_played,
            )
        };
        favorite_entry(
            answered,
            app_id,
            ip,
            connect_port,
            query_port,
            flags,
            last_played,
        )
    }

    /// Stores a server, or refreshes the time of one already stored, and reports the slot Steam
    /// answered with.
    ///
    /// Steam "adds the game server to the local list; updates the time played of the server if it
    /// already exists in the list" (`isteammatchmaking.h:73`). The header says nothing about what a
    /// refusal looks like, so this wrapper states its own rule instead of guessing at Steam's:
    /// an answer that is not a slot index comes back as `None`, and a slot index cannot be negative,
    /// which is what `stored_favorite_slot` decides and the test checks. Anyone who wants Steam's
    /// exact number can still take `sys::SteamAPI_ISteamMatchmaking_AddFavoriteGame` directly.
    pub fn add_favorite_game(&self, entry: &FavoriteGame) -> Option<usize> {
        let raw = unsafe {
            sys::SteamAPI_ISteamMatchmaking_AddFavoriteGame(
                self.mm,
                entry.app_id.0,
                favorite_address_word(entry.ip),
                entry.connect_port,
                entry.query_port,
                entry.flags.bits(),
                entry.last_played_on_server,
            )
        };
        stored_favorite_slot(raw)
    }

    /// Removes the stored entry that matches all of `entry`, answering whether one did.
    ///
    /// The match is not on the app id alone: `flags` is part of the key, because the same server can
    /// sit in either list -- the bits "specify whether the game server was stored as an explicit
    /// favorite or in the history of connections" (`isteammatchmaking.h:69`). So removing a
    /// favorites-list entry needs `FavoriteFlags::FAVORITE`, and passing `HISTORY` instead is a
    /// different key, not a wildcard. Steam's answer is `true`/`false` and nothing else is
    /// reported back, so this stays a `bool`.
    pub fn remove_favorite_game(&self, entry: &FavoriteGame) -> bool {
        unsafe {
            sys::SteamAPI_ISteamMatchmaking_RemoveFavoriteGame(
                self.mm,
                entry.app_id.0,
                favorite_address_word(entry.ip),
                entry.connect_port,
                entry.query_port,
                entry.flags.bits(),
            )
        }
    }
}

/// Steam's count, read as a length: never negative, never wrapped.
fn favorite_count(raw: c_int) -> usize {
    raw.max(0) as usize
}

/// Only an index that still says the same number once it is Steam's `int` may be handed over.
fn favorite_index(index: usize) -> Option<c_int> {
    c_int::try_from(index).ok()
}

/// The decision behind [`Matchmaking::favorite_game`]: `false` is "no such entry", and then the
/// bytes Steam left in the out parameters are not an answer about anything.
fn favorite_entry(
    answered: bool,
    app_id: u32,
    ip: u32,
    connect_port: u16,
    query_port: u16,
    flags: u32,
    last_played: u32,
) -> Option<FavoriteGame> {
    answered.then(|| FavoriteGame {
        app_id: AppId(app_id),
        ip: favorite_address_from_word(ip),
        connect_port,
        query_port,
        flags: FavoriteFlags::from_bits_retain(flags),
        last_played_on_server: last_played,
    })
}

/// The word Steam keeps an IPv4 address in: the first octet most significant, which is the reading
/// this crate already uses in both directions (`u32::from(addr)` out, `Ipv4Addr::from(bits)` back).
/// Named rather than spelled at each call site so the pairing has one owner and one lock; the header
/// states no byte order for this field, so the choice is ours and is stated on the struct.
fn favorite_address_word(ip: Ipv4Addr) -> u32 {
    u32::from(ip)
}

/// The other half of [`favorite_address_word`].
fn favorite_address_from_word(word: u32) -> Ipv4Addr {
    Ipv4Addr::from(word)
}

/// The reading this wrapper chose for `AddFavoriteGame`'s int: a negative answer is not a slot.
fn stored_favorite_slot(raw: c_int) -> Option<usize> {
    usize::try_from(raw).ok()
}

#[cfg(test)]
mod favorites_tests {
    //! Everything above, without a client: the two clamps, the answered/unanswered split, the flag
    //! word, and the address pairing. The four `Matchmaking` methods need a running client and so
    //! are not claimed as covered here.

    use super::{
        favorite_address_from_word, favorite_address_word, favorite_count, favorite_entry,
        favorite_index, stored_favorite_slot, FavoriteFlags, FavoriteGame,
    };
    use crate::AppId;
    use std::net::Ipv4Addr;
    use steamworks_sys as sys;

    const SAMPLE: FavoriteGame = FavoriteGame {
        app_id: AppId(440),
        ip: Ipv4Addr::new(192, 168, 1, 1),
        connect_port: 27015,
        query_port: 27016,
        flags: FavoriteFlags::FAVORITE,
        last_played_on_server: 1_700_000_000,
    };

    #[test]
    fn a_negative_count_is_zero_and_not_a_huge_length() {
        assert_eq!(favorite_count(-1), 0);
        assert_eq!(favorite_count(0), 0);
        assert_eq!(favorite_count(3), 3);
        // The mutation this exists for is `raw as usize`, which turns -1 into
        // 18446744073709551615 and makes any loop over the list a walk off the end.
        assert_ne!(favorite_count(-1), usize::MAX);
    }

    #[test]
    fn an_index_that_is_not_an_int_never_reaches_steam() {
        assert_eq!(favorite_index(0), Some(0));
        assert_eq!(favorite_index(12), Some(12));
        assert!(favorite_index(usize::MAX).is_none());
        assert!(favorite_index(isize::MAX as usize + 1).is_none());
        // Casting instead of checking would have handed Steam -1: a number for a slot that this
        // wrapper's own reading of the count says does not exist.
        assert_ne!(favorite_index(usize::MAX), Some(-1));
    }

    #[test]
    fn an_unanswered_read_is_no_entry_and_not_an_empty_one() {
        assert!(favorite_entry(false, 440, 0xC0A8_0101, 27015, 27016, 1, 1_700_000_000).is_none());
        let read = favorite_entry(
            true,
            SAMPLE.app_id.0,
            SAMPLE.ip.to_bits(),
            SAMPLE.connect_port,
            SAMPLE.query_port,
            SAMPLE.flags.bits(),
            SAMPLE.last_played_on_server,
        );
        assert_eq!(read, Some(SAMPLE));
    }

    /// The pairing that makes the raw `uint32` mean something: out through `to_bits`, back through
    /// `Ipv4Addr::from`, so the first octet stays first -- the reading the crate already uses in both
    /// directions and documents on `User::advertise_game`.
    #[test]
    fn the_address_word_keeps_its_octets_in_the_crates_one_order() {
        assert_eq!(favorite_address_word(SAMPLE.ip), 0xC0A8_0101);
        assert_eq!(favorite_address_from_word(0xC0A8_0101), SAMPLE.ip);
        let entry = favorite_entry(true, 440, u32::from(SAMPLE.ip), 27015, 27016, 1, 0).unwrap();
        assert_eq!(entry.ip, Ipv4Addr::new(192, 168, 1, 1));
        // The reversed word is a different host, and the only way to catch a one-sided change.
        assert_ne!(entry.ip, Ipv4Addr::new(1, 1, 168, 192));
    }

    #[test]
    fn the_two_list_bits_are_the_ones_the_header_declares() {
        assert_eq!(
            FavoriteFlags::FAVORITE.bits(),
            sys::k_unFavoriteFlagFavorite
        );
        assert_eq!(FavoriteFlags::HISTORY.bits(), sys::k_unFavoriteFlagHistory);
        assert_eq!(sys::k_unFavoriteFlagNone, 0);
        assert!(FavoriteFlags::empty().is_empty());
        assert_ne!(
            FavoriteFlags::FAVORITE.bits(),
            FavoriteFlags::HISTORY.bits(),
            "one flag word cannot stand for two different lists"
        );
    }

    #[test]
    fn a_bit_this_wrapper_cannot_name_is_kept_not_dropped() {
        let unknown = FavoriteFlags::from_bits_retain(0x04);
        assert_eq!(unknown.bits(), 0x04);
        assert!(!unknown.contains(FavoriteFlags::FAVORITE));
        assert!(!unknown.contains(FavoriteFlags::HISTORY));
        // Both declared bits at once is a legal stored value, not a contradiction.
        let both = FavoriteFlags::FAVORITE | FavoriteFlags::HISTORY;
        assert_eq!(
            both.bits(),
            sys::k_unFavoriteFlagFavorite | sys::k_unFavoriteFlagHistory
        );
        assert!(both.contains(FavoriteFlags::FAVORITE) && both.contains(FavoriteFlags::HISTORY));
        // Truncating instead of retaining would silently rewrite Steam's answer to 0 here.
        assert_eq!(
            favorite_entry(true, 440, 0, 0, 0, 0x04, 0).unwrap().flags,
            unknown
        );
    }

    #[test]
    fn a_negative_slot_is_not_a_place_in_the_list() {
        assert_eq!(stored_favorite_slot(0), Some(0));
        assert_eq!(stored_favorite_slot(5), Some(5));
        assert_eq!(stored_favorite_slot(-1), None);
        assert!(stored_favorite_slot(-1024).is_none());
    }
}
