//! The names a friend has used before the current one.
//!
//! Steam describes the face in one line: "accesses old friends names - returns an empty string when
//! their are no more items in the history" (`isteamfriends.h:219`). That sentence does two jobs here,
//! and both are Steam's: the iterator's end is a value inside the same call rather than a second
//! count call, and the value it uses for "no more" is the empty string. So the wrapper's only added
//! judgement is what to do with the two shapes that are not that marker -- a null pointer and a
//! buffer that is not valid UTF-8 -- both answered as `None` rather than as a name.

use super::*;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int};

/// Which name in the history the caller asked for, refused when it cannot be handed over as Steam's
/// `int iPersonaName` (`isteamfriends.h:220`) -- the line above it is what Steam calls this face:
/// "accesses old friends names - returns an empty string when their are no more items in the history"
/// (`:219`).
fn history_index(raw: usize) -> Option<c_int> {
    c_int::try_from(raw).ok()
}

/// One name, or nothing -- and "nothing" covers three different things.
///
/// The empty string is what Steam declares as the end -- "returns an empty string when their are no
/// more items in the history" (`isteamfriends.h:219`) -- so it ends the walk instead of becoming a
/// name somebody used. A null pointer is not the marker (the header does not offer it), and a
/// buffer that is not UTF-8 is not a name either: converting it lossily would hand back replacement
/// characters that look like a persona name and compare against nothing a caller could check.
fn history_name(raw: *const c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    let text = unsafe { CStr::from_ptr(raw) }.to_str().ok()?;
    if text.is_empty() {
        return None;
    }
    Some(text.to_owned())
}

impl Friends {
    /// The `index`-th older name this friend has used, or `None` past the end of the history.
    ///
    /// Walk it from zero and stop at the first `None`: Steam puts the end of the list inside this
    /// same call ("returns an empty string when their are no more items in the history",
    /// `isteamfriends.h:219`), which is why there is no separate count here.
    pub fn persona_name_history(&self, user: SteamId, index: usize) -> Option<String> {
        let index = history_index(index)?;
        let raw = unsafe {
            sys::SteamAPI_ISteamFriends_GetFriendPersonaNameHistory(self.friends, user.raw(), index)
        };
        history_name(raw)
    }
}

#[cfg(test)]
mod history_tests {
    //! The three readings of the pointer, and the index that cannot be sent. Which names a real
    //! friend has used needs a live client and is registered as unverified.

    use super::{history_index, history_name};
    use std::ffi::CString;
    use std::os::raw::c_int;

    /// Steam's own end-of-history value is a name nobody could have chosen, so it ends the walk
    /// instead of becoming the first entry.
    #[test]
    fn the_empty_string_ends_the_walk_rather_than_naming_somebody() {
        let empty = CString::new("").unwrap();
        assert_eq!(history_name(empty.as_ptr()), None);
        let used = CString::new("old-handle").unwrap();
        assert_eq!(
            history_name(used.as_ptr()),
            Some("old-handle".to_owned()),
            "a non-empty name is forwarded whole"
        );
    }

    /// A buffer that is not UTF-8 is not a name: `to_string_lossy` would invent replacement
    /// characters, and a caller comparing against a real handle would get a silent false.
    #[test]
    fn a_history_entry_is_returned_whole_or_not_at_all() {
        assert_eq!(history_name(std::ptr::null()), None);
        let bytes = [0xc3_u8, 0x28, 0x00];
        assert_eq!(history_name(bytes.as_ptr().cast()), None);
    }

    /// An index Steam cannot receive as its `int` is never asked about, because a cast would have
    /// queried a different entry than the caller named.
    #[test]
    fn an_index_that_cannot_be_a_c_int_is_never_asked_about() {
        assert_eq!(history_index(0), Some(0));
        assert_eq!(history_index(c_int::MAX as usize), Some(c_int::MAX));
        assert_eq!(history_index(c_int::MAX as usize + 1), None);
    }
}
