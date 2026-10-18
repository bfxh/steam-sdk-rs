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
    pub fn get_num_achievements(&self) -> Result<u32, ()> {
        unsafe {
            let num = sys::SteamAPI_ISteamUserStats_GetNumAchievements(self.user_stats);
            if num != 0 {
                Ok(num)
            } else {
                Err(())
            }
        }
    }

    /// Returns an array of all achievement names for the current AppId.
    ///
    /// Returns an empty string for an achievement name if `iAchievement` is not a valid index,
    /// and the current AppId must have achievements.
    pub fn get_achievement_names(&self) -> Option<Vec<String>> {
        let num = self
            .get_num_achievements()
            .expect("Failed to get number of achievements");
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
