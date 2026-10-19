//! Achievements: unlocking and inspecting one (the helper `achievement()` builds lives in
//! `stats.rs`), counting them, naming them, and the opt-in request for the global
//! percentage-of-players table.

use super::*;
use crate::utils::c_string_prefix;

impl UserStats {
    /// Asynchronously fetch the data for the percentage of players who have received each achievement
    /// for the current game globally.
    ///
    /// You must have called `request_current_stats()` and it needs to return successfully via its
    /// callback prior to calling this!*
    ///
    /// **Note: Not sure if this is applicable, as the other achievement functions requiring
    /// `request_current_stats()` don't specifically need it to be called in order for them to complete
    /// successfully. Maybe it autoruns via `Client::init()/init_app()` somehow?*
    pub fn request_global_achievement_percentages<F>(&self, cb: F)
    where
        F: FnOnce(Result<GameId, SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call =
                sys::SteamAPI_ISteamUserStats_RequestGlobalAchievementPercentages(self.user_stats);
            register_call_result::<sys::GlobalAchievementPercentagesReady_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        Ok(GameId(v.m_nGameID))
                    })
                },
            );
        }
    }

    /// Access achievement API for a given achievement 'API Name'.
    ///
    /// Requires [`request_current_stats()`](#method.request_current_stats) to have been called
    /// and a successful [`UserStatsReceived`](./struct.UserStatsReceived.html) callback processed.
    #[inline]
    #[must_use]
    pub fn achievement(&self, name: &str) -> stats::AchievementHelper<'_> {
        stats::AchievementHelper {
            name: c_string_prefix(name),
            parent: self,
        }
    }

    /// Get the number of achievements defined in the App Admin panel of the Steamworks website.
    ///
    /// This is used for iterating through all of the achievements with GetAchievementName.
    ///
    /// Returns 0 if the current App ID has no achievements.
    ///
    /// *Note: Returns an error for AppId `480` (Spacewar)!*
    pub fn get_num_achievements(&self) -> SteamResult<u32> {
        // SAFETY: the call reads only this handle and returns a plain count; the decision made from
        // that number is not taken here -- it lives in `achievement_count`, where it can be judged.
        let num = unsafe { sys::SteamAPI_ISteamUserStats_GetNumAchievements(self.user_stats) };
        achievement_count(num)
    }

    /// Returns an array of all achievement names for the current AppId.
    ///
    /// Returns an empty string for an achievement name if `iAchievement` is not a valid index,
    /// and the current AppId must have achievements.
    ///
    /// `None` is the answer when there is no list to build: `get_num_achievements` reports a count of
    /// zero as its error case, and zero is a normal state -- this file's own note says the call returns 0
    /// for an AppId with no achievements, and stats that were never requested answer the same way. The
    /// `.expect(..)` that used to sit here turned both of those ordinary states into a panic inside a
    /// getter whose signature already says the answer may be absent.
    pub fn get_achievement_names(&self) -> Option<Vec<String>> {
        let num = self.get_num_achievements().ok()?;
        let mut names = Vec::new();

        for i in 0..num {
            unsafe {
                let name = sys::SteamAPI_ISteamUserStats_GetAchievementName(self.user_stats, i);

                let c_str = CStr::from_ptr(name).to_string_lossy().into_owned();

                names.push(c_str);
            }
        }
        Some(names)
    }
}

/// What a `GetNumAchievements` answer means for a caller.
///
/// The header declares "virtual uint32 GetNumAchievements() = 0;" (`isteamuserstats.h:151`) with no
/// failure value and no error channel of its own, so zero is simultaneously the app having no
/// achievements at all -- which this file documents as the ordinary answer -- and stats never having
/// been requested, which is what `UserStats::request_current_stats` is for. Those two are not
/// distinguishable from the number, and this crate chose to surface both as an error rather than as an
/// empty list; that choice is the contract here, and it is what makes the arm decidable without a
/// client.
fn achievement_count(raw: u32) -> SteamResult<u32> {
    if raw != 0 {
        Ok(raw)
    } else {
        Err(SteamError::InvalidState)
    }
}

#[cfg(test)]
mod achievement_count_tests {
    //! The pure half of the achievement iteration: the count that decides whether there is anything to
    //! walk. The FFI read needs a running client, the reading of its number does not, so the decision
    //! lives in `achievement_count` and is judged here.
    use super::achievement_count;
    use crate::SteamError;

    #[test]
    fn a_positive_count_is_the_number_of_names_to_walk() {
        assert_eq!(achievement_count(1), Ok(1));
        assert_eq!(achievement_count(97), Ok(97));
        assert_eq!(achievement_count(u32::MAX), Ok(u32::MAX));
    }

    /// Zero is the state a game hits before `request_current_stats()` has landed, and the state an AppId
    /// with no achievements sits in permanently. Neither may reach the caller as a panic.
    #[test]
    fn zero_is_the_documented_error_case_not_a_panic() {
        assert_eq!(achievement_count(0), Err(SteamError::InvalidState));
    }

    /// Anti-vacuity: the caller's `.ok()?` really does have a failure arm to propagate. Without this,
    /// a mutation that made `achievement_count` total would still leave the getter's tests green.
    #[test]
    fn the_propagated_error_really_is_an_error() {
        assert!(achievement_count(0).is_err());
        assert!(achievement_count(0).ok().is_none());
    }
}
