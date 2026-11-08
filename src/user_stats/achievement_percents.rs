//! What the whole player base did with this game's achievements: the two calls that read the
//! percentages Steam fetches for you.
//!
//! This crate has been able to *ask* for that data for a long time
//! ([`UserStats::request_global_achievement_percentages`]) and had no way to read the answer, which
//! is the gap these two methods close. Both are the same C shape: a caller-sized name buffer, two
//! out-parameters, and an `int` that is either the next iterator or Steam's stop signal.
//!
//! Two things here are this wrapper's decision rather than Steam's, and both are said plainly where
//! they are used. The name buffer is sized by the caller because the header states no maximum length
//! for an achievement name (measured: `isteamuserstats.h` gives no length constant for these two
//! calls), and a buffer that comes back with no terminator is reported as a failure instead of being
//! shortened to fit.

use super::*;
use std::os::raw::c_char;

/// The caller's buffer length, as the `uint32` Steam asks for.
fn checked_capacity(raw: usize) -> SteamResult<u32> {
    let widened = u32::try_from(raw).map_err(|_| SteamError::InvalidParameter)?;
    if widened == 0 {
        // A zero-length buffer cannot hold a terminator, so it can never produce a name.
        return Err(SteamError::InvalidParameter);
    }
    Ok(widened)
}

/// One row of the global achievement percentages, as Steam wrote it.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalAchievementInfo {
    /// The achievement's token name, copied out of the caller's buffer.
    pub name: String,
    /// The share of players who have it, in the fraction Steam's `float` carries. This is the global
    /// table's row; the per-achievement question is the one `AchievementHelper` already answers.
    pub percent: f32,
    /// Whether *the current user* has it, which is what the header's `pbAchieved` out-parameter is.
    pub achieved_by_current_user: bool,
    /// The iterator to continue with, or `None` when Steam signalled that the walk is over.
    ///
    /// Steam uses one value for two different endings: `-1` means both "no data on achievement"
    /// percentages (`isteamuserstats.h:260`) and comes back "after the last" achievement "has been
    /// iterated" (`:265-266`). So a `None` here does not say which of the two it was, and nothing in
    /// this type pretends to.
    pub next_iterator: Option<i32>,
}

/// The name, from a buffer that was zero-filled before Steam wrote into it.
///
/// `None` means either there was no terminator anywhere in the capacity -- the name did not fit --
/// or the bytes before the terminator are not text. Returning the prefix that did fit would hand back
/// a token that is not any achievement's name.
fn name_from_buffer(buffer: &[u8]) -> Option<String> {
    let end = buffer.iter().position(|byte| *byte == 0)?;
    std::str::from_utf8(&buffer[..end])
        .map(str::to_string)
        // Steam's strings are UTF-8, so bytes that are not are not a name to shorten -- they are an
        // answer this wrapper cannot report as one.
        .ok()
}

/// Steam's `int` as "keep going" or "stop", keeping zero a legal iterator.
///
/// The header names `-1` and nothing else on the negative side, so any negative stops the walk rather
/// than being fed back in as an index.
fn iteration(raw: i32) -> Option<i32> {
    if raw < 0 {
        None
    } else {
        Some(raw)
    }
}

/// The one tail both calls share.
///
/// Steam's stop signal wins over the buffer: when the iterator says there is no row, nothing was asked
/// of the buffer. Otherwise a name that did not come back whole is an error, not a row with a missing
/// name and a usable iterator.
fn finish_row(
    raw: i32,
    buffer: &[u8],
    percent: f32,
    achieved: bool,
) -> SteamResult<Option<GlobalAchievementInfo>> {
    let Some(next) = iteration(raw) else {
        return Ok(None);
    };
    match name_from_buffer(buffer) {
        Some(name) => Ok(Some(GlobalAchievementInfo {
            name,
            percent,
            achieved_by_current_user: achieved,
            next_iterator: Some(next),
        })),
        None => Err(SteamError::InvalidParameter),
    }
}

impl UserStats {
    /// The most-achieved achievement in the game, and the iterator to continue from.
    ///
    /// Steam describes both halves in one sentence: this gets "the info on the most achieved
    /// achievement for the game, returns an iterator index you can use to fetch" the next
    /// (`isteamuserstats.h:259`), and it "Will return -1 if there is no data on achievement"
    /// percentages -- which the same block spells out as "you haven't called
    /// RequestGlobalAchievementPercentages and waited on the callback" (`:260-261`). That is why the
    /// return is `Option`: Steam's stop signal is a value in the same field as the answer, not a
    /// separate failure.
    ///
    /// `name_capacity` sizes the buffer the name is read into, because the header states no maximum
    /// achievement name. A capacity too small for the name and its terminator comes back as
    /// `Err(SteamError::InvalidParameter)` rather than as a shortened name; so does `0`, which could
    /// never hold a terminator.
    pub fn most_achieved_achievement_info(
        &self,
        name_capacity: usize,
    ) -> SteamResult<Option<GlobalAchievementInfo>> {
        let capacity = checked_capacity(name_capacity)?;
        let mut buffer = vec![0u8; name_capacity];
        let mut percent = 0f32;
        let mut achieved = false;
        let raw = unsafe {
            sys::SteamAPI_ISteamUserStats_GetMostAchievedAchievementInfo(
                self.user_stats,
                buffer.as_mut_ptr() as *mut c_char,
                capacity,
                &mut percent,
                &mut achieved,
            )
        };
        finish_row(raw, &buffer, percent, achieved)
    }

    /// The next row of the same walk.
    ///
    /// Steam's instruction is "Call this after GetMostAchievedAchievementInfo or another" call of this
    /// shape, passing "the iterator from the previous call", and it "Returns -1 after the last"
    /// achievement "has been iterated" (`isteamuserstats.h:264-266`). The iterator is not a row number
    /// and not a count of rows seen, so this wrapper forwards the caller's value untouched rather than
    /// second-guessing it.
    pub fn next_most_achieved_achievement_info(
        &self,
        previous_iterator: i32,
        name_capacity: usize,
    ) -> SteamResult<Option<GlobalAchievementInfo>> {
        let capacity = checked_capacity(name_capacity)?;
        let mut buffer = vec![0u8; name_capacity];
        let mut percent = 0f32;
        let mut achieved = false;
        let raw = unsafe {
            sys::SteamAPI_ISteamUserStats_GetNextMostAchievedAchievementInfo(
                self.user_stats,
                previous_iterator,
                buffer.as_mut_ptr() as *mut c_char,
                capacity,
                &mut percent,
                &mut achieved,
            )
        };
        finish_row(raw, &buffer, percent, achieved)
    }
}

#[cfg(test)]
mod percent_tests {
    //! The buffer rule, the capacity rule and the iterator rule, judged without a client. Which rows
    //! Steam actually returns is registered as unverified: both methods need a live session whose
    //! global percentages were requested and answered.

    use super::{checked_capacity, finish_row, iteration, name_from_buffer};
    use crate::SteamError;

    /// A name that fits comes back whole; one that does not comes back as nothing. The mutation this
    /// exists for is taking the prefix anyway, which yields a token that is no achievement's name and
    /// reads downstream like a data problem.
    #[test]
    fn a_name_is_either_whole_or_not_reported() {
        assert_eq!(
            name_from_buffer(b"WIN_THE_GAME\0\0\0\0"),
            Some("WIN_THE_GAME".to_string())
        );
        // Sixteen bytes, no terminator anywhere: the name did not fit.
        assert_eq!(name_from_buffer(b"AWWERLYLONGNAME!"), None);
        // A terminator in the very last byte still counts, which is the boundary a `len - 2` loop
        // would miss.
        assert_eq!(
            name_from_buffer(b"AWWERLYLONGNAME\0"),
            Some("AWWERLYLONGNAME".to_string())
        );
    }

    /// The zero-length name is forwarded as a name, not turned into "no name": Steam wrote a terminator
    /// at the front, and inventing a failure there would read a meaning into the bytes that is not in
    /// them.
    #[test]
    fn an_empty_name_is_empty_rather_than_absent() {
        assert_eq!(name_from_buffer(b"\0\0\0\0"), Some(String::new()));
    }

    /// Bytes that are not UTF-8 are refused rather than converted lossily, because a lossy name is one
    /// the caller cannot compare against anything Steam said.
    #[test]
    fn bytes_that_are_not_text_are_not_lossy_converted() {
        assert_eq!(name_from_buffer(b"\xff\xfe\x00"), None);
    }

    /// Zero is a legal iterator, because the header names only `-1` as its stop signal. The mutation
    /// this exists for is `<= 0`, which ends the walk before the first row.
    #[test]
    fn only_the_negative_side_is_steam_stop_signal() {
        assert_eq!(iteration(-1), None);
        assert_eq!(iteration(0), Some(0));
        assert_eq!(iteration(7), Some(7));
        assert_eq!(iteration(i32::MIN), None);
        assert_eq!(iteration(i32::MAX), Some(i32::MAX));
    }

    /// A buffer the caller cannot have meant: zero bytes has no room for a terminator, so it is
    /// refused before Steam is asked, instead of coming back as a name that is always empty.
    #[test]
    fn a_capacity_that_cannot_hold_a_terminator_is_refused() {
        assert_eq!(checked_capacity(0), Err(SteamError::InvalidParameter));
        assert_eq!(checked_capacity(1), Ok(1));
        assert_eq!(checked_capacity(u32::MAX as usize), Ok(u32::MAX));
    }

    /// The tail is where Steam's stop signal beats the buffer, and where a cut-off name beats a
    /// plausible iterator. Checked by crossing those two, which a mutation turns into a silent
    /// `Ok(None)` -- the caller stops iterating and never learns why.
    #[test]
    fn a_truncated_name_is_not_reported_as_the_end_of_the_walk() {
        let row = finish_row(3, b"WIN\0", 0.5, true)
            .unwrap()
            .expect("a whole name and a live iterator is a row");
        assert_eq!(row.name, "WIN");
        assert_eq!(row.percent, 0.5);
        assert!(row.achieved_by_current_user);
        assert_eq!(row.next_iterator, Some(3));

        assert_eq!(
            finish_row(3, b"NO TERMINATOR", 0.5, true),
            Err(SteamError::InvalidParameter)
        );
        assert_eq!(finish_row(-1, b"NO TERMINATOR", 0.0, false), Ok(None));
    }
}
