//! The read path that answers about *another* user, plus the progress bounds Steam keeps for an
//! achievement. Every method here has the same shape: Steam returns a `bool` and writes an
//! out-parameter, and the out-parameter means nothing when the `bool` is false. That is the whole
//! defect class this module exists to pin down, so both decisions -- whether to trust a value and
//! what an unlock time says -- are pure functions judged below without a client.
//!
//! The error for "Steam said no" is `SteamError::InvalidParameter`, the same answer the sibling
//! interface already gives for its identical shape --
//! [`GameServerStats::get_user_stat_i32`](crate::GameServerStats::get_user_stat_i32),
//! `get_user_stat_f32` and `get_user_achievement` in `src/server_stats.rs`. The two interfaces
//! decline for the same reasons (no stats loaded for that user, or no stat or achievement under
//! that name).

use super::*;
use crate::app::c_string_arg;

/// What Steam says about one achievement's unlock state, from the two numbers it writes.
///
/// The three documented shapes come from Steam's own note on `GetAchievementAndUnlockTime`, which
/// the friends-side call forwards to: "If the return value is true, but the unlock time is zero,
/// that means it was unlocked before Steam" began tracking (`isteamuserstats.h:119`), and "Time is
/// seconds since January 1, 1970." (`:120`). The fourth arm is the combination the note does not
/// describe -- a time written while the flag says not achieved -- and it keeps the number instead
/// of rounding it into [`AchievementUnlock::NotAchieved`], because a caller who sees a nonzero time
/// should be able to ask why, not be told there was none.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AchievementUnlock {
    /// Not achieved, and Steam wrote no time with it.
    NotAchieved,
    /// Achieved, and the time is zero: "unlocked before Steam" started keeping unlock times
    /// (`isteamuserstats.h:119`), which the same line dates as December 2009.
    AchievedBeforeTracking,
    /// Achieved at this many seconds since the epoch, which is what Steam's "Time is seconds since
    /// January 1, 1970." (`:120`) states, forwarded unchanged.
    ///
    /// The number stays Steam's own `uint32`: converting it to a `SystemTime` here would pick a
    /// calendar reading the header does not ask for, while losing nothing that a caller cannot add
    /// back in one line.
    UnlockedAt(u32),
    /// Not achieved, and yet a time came back. Steam's note covers the other three cells and says
    /// nothing here: its own words are "If the return value is true, but the unlock time is zero"
    /// (`isteamuserstats.h:119`), which is a sentence about an achieved flag. Forwarded as a number,
    /// not folded into [`AchievementUnlock::NotAchieved`].
    UnachievedWithTime(u32),
}

/// The four cells of (achieved, time), each with its own name.
fn unlock_state(achieved: bool, raw_seconds: u32) -> AchievementUnlock {
    match (achieved, raw_seconds) {
        (false, 0) => AchievementUnlock::NotAchieved,
        (false, seconds) => AchievementUnlock::UnachievedWithTime(seconds),
        (true, 0) => AchievementUnlock::AchievedBeforeTracking,
        (true, seconds) => AchievementUnlock::UnlockedAt(seconds),
    }
}

/// Steam's `bool` decides whether the value it wrote is an answer.
///
/// On `false` nothing is returned -- not a zero, not the last value the caller put in the
/// out-parameter -- because the headers describe no value for the out-parameter of a declined call.
/// Every declined read in this module reports `SteamError::InvalidParameter` from this one place.
/// Calls that return only a `bool` use the same rule with `()` as the value, so every Steam `bool`
/// on this interface is decided once.
pub(crate) fn read_answer<T>(answered: bool, value: T) -> SteamResult<T> {
    if answered {
        Ok(value)
    } else {
        Err(SteamError::InvalidParameter)
    }
}

impl UserStats {
    /// Reads one user's 32-bit integer stat.
    ///
    /// Steam's instruction is that this "requests stat information for a user, usable after a
    /// successful call to RequestUserStats()" (`isteamuserstats.h:164`); the two flat names it
    /// gives the same C++ overload are `GetUserStatInt32` (`:165-166`) and `GetUserStatFloat`
    /// (`:168-169`), and this crate keeps them apart the way the header does.
    pub fn user_stat_i32(&self, user: SteamId, stat: &str) -> SteamResult<i32> {
        let stat = c_string_arg(stat)?;
        let mut value = 0i32;
        let answered = unsafe {
            sys::SteamAPI_ISteamUserStats_GetUserStatInt32(
                self.user_stats,
                user.raw(),
                stat.as_ptr(),
                &mut value,
            )
        };
        read_answer(answered, value)
    }

    /// Reads one user's float stat. Steam's precondition for both forms is that this "requests stat
    /// information for a user, usable after a successful call to RequestUserStats()"
    /// (`isteamuserstats.h:164`); see [`UserStats::user_stat_i32`] for the declined answer's shape.
    pub fn user_stat_f32(&self, user: SteamId, stat: &str) -> SteamResult<f32> {
        let stat = c_string_arg(stat)?;
        let mut value = 0f32;
        let answered = unsafe {
            sys::SteamAPI_ISteamUserStats_GetUserStatFloat(
                self.user_stats,
                user.raw(),
                stat.as_ptr(),
                &mut value,
            )
        };
        read_answer(answered, value)
    }

    /// Reads whether one user has earned an achievement.
    ///
    /// This sits under Steam's heading "Friends stats & achievements" (`isteamuserstats.h:155`) and
    /// shares `GetUserStat`'s precondition at `:164`. For the unlock *time* as well, use
    /// [`UserStats::user_achievement_and_unlock_time`], whose own line says "See notes for
    /// GetAchievementAndUnlockTime above" (`:172`).
    pub fn user_achievement(&self, user: SteamId, achievement: &str) -> SteamResult<bool> {
        let achievement = c_string_arg(achievement)?;
        let mut achieved = false;
        let answered = unsafe {
            sys::SteamAPI_ISteamUserStats_GetUserAchievement(
                self.user_stats,
                user.raw(),
                achievement.as_ptr(),
                &mut achieved,
            )
        };
        read_answer(answered, achieved)
    }

    /// Reads whether one user has earned an achievement, and what Steam says about when.
    ///
    /// Steam's whole instruction for this one is "See notes for GetAchievementAndUnlockTime above"
    /// (`isteamuserstats.h:172`), and those notes are the two lines about the zero time (`:119-120`);
    /// [`AchievementUnlock`] carries what each of the four combinations means, and `unlock_state` is
    /// where that is decided.
    pub fn user_achievement_and_unlock_time(
        &self,
        user: SteamId,
        achievement: &str,
    ) -> SteamResult<AchievementUnlock> {
        let achievement = c_string_arg(achievement)?;
        let mut achieved = false;
        let mut raw_seconds = 0u32;
        let answered = unsafe {
            sys::SteamAPI_ISteamUserStats_GetUserAchievementAndUnlockTime(
                self.user_stats,
                user.raw(),
                achievement.as_ptr(),
                &mut achieved,
                &mut raw_seconds,
            )
        };
        // Both locals are initialised above, so building the answer here cannot read anything Steam
        // did not write; `read_answer` is still the only place that decides whether it counts.
        read_answer(answered, unlock_state(achieved, raw_seconds))
    }

    /// The progress bounds Steam keeps for an achievement that has related Progress stats.
    ///
    /// "For achievements that have related Progress stats, use this to query what the bounds of that
    /// progress are." (`isteamuserstats.h:297`), and the sentence after it says why: "You may want
    /// this info to selectively call IndicateAchievementProgress" (`:298`). The pair is
    /// `(min, max)`, forwarded in that order; nothing here checks that `min <= max`, because the
    /// header states no such constraint and a wrapper that enforced it would be reporting its own
    /// opinion about Steam's data as an error.
    pub fn achievement_progress_limits_i32(&self, achievement: &str) -> SteamResult<(i32, i32)> {
        let achievement = c_string_arg(achievement)?;
        let mut min = 0i32;
        let mut max = 0i32;
        let answered = unsafe {
            sys::SteamAPI_ISteamUserStats_GetAchievementProgressLimitsInt32(
                self.user_stats,
                achievement.as_ptr(),
                &mut min,
                &mut max,
            )
        };
        read_answer(answered, (min, max))
    }

    /// The float form of [`UserStats::achievement_progress_limits_i32`]. Steam's flat name for it
    /// is `GetAchievementProgressLimitsFloat` (`isteamuserstats.h:303-304`) under the same
    /// "bounds of that progress are" instruction (`:297`).
    pub fn achievement_progress_limits_f32(&self, achievement: &str) -> SteamResult<(f32, f32)> {
        let achievement = c_string_arg(achievement)?;
        let mut min = 0f32;
        let mut max = 0f32;
        let answered = unsafe {
            sys::SteamAPI_ISteamUserStats_GetAchievementProgressLimitsFloat(
                self.user_stats,
                achievement.as_ptr(),
                &mut min,
                &mut max,
            )
        };
        read_answer(answered, (min, max))
    }
}

#[cfg(test)]
mod user_reads_tests {
    //! The two tables, judged without a client. What Steam answers over FFI is registered as
    //! unverified: every method above needs stats that were actually requested for a user.

    use super::{read_answer, unlock_state, AchievementUnlock};
    use crate::SteamError;

    /// All four cells of (achieved, time), each with a fixture that only that cell produces. The
    /// mutation this exists for is two cells sharing an arm, which reads plausible for the three
    /// documented ones and loses the fourth.
    #[test]
    fn every_combination_of_the_two_numbers_has_its_own_answer() {
        assert_eq!(unlock_state(false, 0), AchievementUnlock::NotAchieved);
        assert_eq!(
            unlock_state(true, 0),
            AchievementUnlock::AchievedBeforeTracking
        );
        assert_eq!(
            unlock_state(true, 1_234_567_890),
            AchievementUnlock::UnlockedAt(1_234_567_890)
        );
        assert_eq!(
            unlock_state(false, 1_234_567_890),
            AchievementUnlock::UnachievedWithTime(1_234_567_890)
        );
        // Four distinct answers, so no two cells collapse under a renamed arm either.
        let all = [
            unlock_state(false, 0),
            unlock_state(true, 0),
            unlock_state(true, 7),
            unlock_state(false, 7),
        ];
        for (left, right) in all.iter().zip(all.iter().skip(1)) {
            assert_ne!(left, right, "{left:?} and {right:?} share an answer");
        }
    }

    /// The zero time is not the epoch, and the epoch is not the zero time. This is the one place
    /// Steam's own sentence changes the answer's type rather than its number: a `0` here would read
    /// as 1970-01-01 to anyone who turned it into a date.
    #[test]
    fn a_zero_time_on_an_achieved_flag_is_not_a_date() {
        assert_eq!(
            unlock_state(true, 0),
            AchievementUnlock::AchievedBeforeTracking
        );
        assert_ne!(
            unlock_state(true, 0),
            unlock_state(true, 1),
            "the boundary is Steam's December 2009 note, and 0 is the documented sentinel"
        );
    }

    /// The undocumented combination keeps its number. Whether Steam ever writes it is registered as
    /// unverified -- nothing here reaches a client -- but the table is total, so the wrapper is not
    /// guessing when it happens.
    #[test]
    fn an_unachieved_flag_with_a_time_keeps_the_time() {
        assert_eq!(
            unlock_state(false, 42),
            AchievementUnlock::UnachievedWithTime(42)
        );
        assert_eq!(
            unlock_state(false, u32::MAX),
            AchievementUnlock::UnachievedWithTime(u32::MAX),
            "the largest number Steam can write survives the round trip"
        );
    }

    /// A declined call returns nothing. The mutation this exists for is `Ok(value)` unconditionally,
    /// which hands back whatever the caller's buffer held -- here, a value that could not be
    /// confused with the zero a fresh buffer would hold.
    #[test]
    fn a_declined_read_produces_no_value() {
        assert_eq!(read_answer(true, 12345i32), Ok(12345));
        assert_eq!(
            read_answer(false, 12345i32),
            Err(SteamError::InvalidParameter),
            "the number Steam did not confirm must not come back as an answer"
        );
        assert_eq!(read_answer(false, 0i32), Err(SteamError::InvalidParameter));
    }
}
