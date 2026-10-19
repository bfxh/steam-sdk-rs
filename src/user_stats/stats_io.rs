//! The current player's own numbers and the lifecycle around them: ask Steam for the stats
//! (`request_user_stats`), read or write one stat, then push the batch back (`store_stats`)
//! or throw it away (`reset_all_stats`).

use super::*;
use crate::app::c_string_arg;
use crate::utils::try_c_string;

impl UserStats {
    /// Triggers a [`UserStatsReceived`](./struct.UserStatsReceived.html) callback.
    pub fn request_user_stats(&self, steam_user_id: u64) {
        unsafe {
            sys::SteamAPI_ISteamUserStats_RequestUserStats(self.user_stats, steam_user_id);
        }
    }

    /// Send the changed stats and achievements data to the server for permanent storage.
    ///
    /// * Triggers a [`UserStatsStored`](../struct.UserStatsStored.html) callback if successful.
    /// * Triggers a [`UserAchievementStored`](../struct.UserAchievementStored.html) callback
    ///   if achievements have been unlocked.
    ///
    /// Requires [`request_current_stats()`](#method.request_current_stats) to have been called
    /// and a successful [`UserStatsReceived`](./struct.UserStatsReceived.html) callback processed.
    pub fn store_stats(&self) -> Result<(), ()> {
        let success = unsafe { sys::SteamAPI_ISteamUserStats_StoreStats(self.user_stats) };
        if success {
            Ok(())
        } else {
            Err(())
        }
    }

    /// Resets the current users stats and, optionally achievements.
    pub fn reset_all_stats(&self, achievements_too: bool) -> Result<(), ()> {
        let success = unsafe {
            sys::SteamAPI_ISteamUserStats_ResetAllStats(self.user_stats, achievements_too)
        };
        if success {
            Ok(())
        } else {
            Err(())
        }
    }

    /// Gets the value of a given stat for the current user
    ///
    /// The specified stat must exist and match the type set on the Steamworks App Admin website.
    ///
    /// Requires [`request_current_stats()`](#method.request_current_stats) to have been called
    /// and a successful [`UserStatsReceived`](./struct.UserStatsReceived.html) callback processed.
    /// A stat name containing a nul byte answers `Err(())`: the stat that would be read is not the one
    /// named, and `()` is the failure this type already means.
    pub fn get_stat_i32(&self, name: &str) -> Result<i32, ()> {
        let name = try_c_string(name).ok_or(())?;

        let mut value: i32 = 0;
        let success = unsafe {
            sys::SteamAPI_ISteamUserStats_GetStatInt32(self.user_stats, name.as_ptr(), &mut value)
        };
        if success {
            Ok(value)
        } else {
            Err(())
        }
    }

    /// Sets / updates the value of a given stat for the current user
    ///
    /// This call only changes the value in-memory and is very cheap. To commit the stats you
    /// must call [`store_stats()`](#method.store_stats)
    ///
    /// The specified stat must exist and match the type set on the Steamworks App Admin website.
    ///
    /// Requires [`request_current_stats()`](#method.request_current_stats) to have been called
    /// and a successful [`UserStatsReceived`](./struct.UserStatsReceived.html) callback processed.
    pub fn set_stat_i32(&self, name: &str, stat: i32) -> Result<(), ()> {
        let name = try_c_string(name).ok_or(())?;

        let success = unsafe {
            sys::SteamAPI_ISteamUserStats_SetStatInt32(self.user_stats, name.as_ptr(), stat)
        };
        if success {
            Ok(())
        } else {
            Err(())
        }
    }

    /// Gets the value of a given stat for the current user
    ///
    /// The specified stat must exist and match the type set on the Steamworks App Admin website.
    ///
    /// Requires [`request_current_stats()`](#method.request_current_stats) to have been called
    /// and a successful [`UserStatsReceived`](./struct.UserStatsReceived.html) callback processed.
    pub fn get_stat_f32(&self, name: &str) -> Result<f32, ()> {
        let name = try_c_string(name).ok_or(())?;

        let mut value: f32 = 0.0;
        let success = unsafe {
            sys::SteamAPI_ISteamUserStats_GetStatFloat(self.user_stats, name.as_ptr(), &mut value)
        };
        if success {
            Ok(value)
        } else {
            Err(())
        }
    }

    /// Sets / updates the value of a given stat for the current user
    ///
    /// This call only changes the value in-memory and is very cheap. To commit the stats you
    /// must call [`store_stats()`](#method.store_stats)
    ///
    /// The specified stat must exist and match the type set on the Steamworks App Admin website.
    ///
    /// Requires [`request_current_stats()`](#method.request_current_stats) to have been called
    /// and a successful [`UserStatsReceived`](./struct.UserStatsReceived.html) callback processed.
    pub fn set_stat_f32(&self, name: &str, stat: f32) -> Result<(), ()> {
        let name = try_c_string(name).ok_or(())?;

        let success = unsafe {
            sys::SteamAPI_ISteamUserStats_SetStatFloat(self.user_stats, name.as_ptr(), stat)
        };
        if success {
            Ok(())
        } else {
            Err(())
        }
    }

    /// Reports progress towards an achievement, and nothing else.
    ///
    /// Steam's own two sentences are the whole contract: "Achievement progress - triggers an
    /// AchievementProgress callback, that is all." (`isteamuserstats.h:145`) and "Calling this w/ N
    /// out of N progress will NOT set the achievement, the game must still do that." (`:146`), on the
    /// declaration at `:147`. So reaching `n_max_progress` here unlocks nothing; the unlock is
    /// [`AchievementHelper::set`](crate::user_stats::stats::AchievementHelper::set) followed by a
    /// successful store.
    ///
    /// Both numbers are `u32` because that is what Steam declares. No comparison between them is
    /// made: the header describes none, and a wrapper that refused `cur > max` would be reporting its
    /// own opinion about the game's progress data as an error.
    pub fn indicate_achievement_progress(
        &self,
        achievement: &str,
        current: u32,
        maximum: u32,
    ) -> SteamResult {
        let achievement = c_string_arg(achievement)?;
        let answered = unsafe {
            sys::SteamAPI_ISteamUserStats_IndicateAchievementProgress(
                self.user_stats,
                achievement.as_ptr(),
                current,
                maximum,
            )
        };
        super::user_reads::read_answer(answered, ())
    }

    /// Adds this session's count to one of Steam's average-rate stats.
    ///
    /// This sits under Steam's heading "Set / update data" (`isteamuserstats.h:104`) beside the two
    /// `SetStat` forms, and is declared at `:111` as a count for this session over a session length.
    /// Both numbers are forwarded exactly as Steam types them (`float` and `double`); the header says
    /// nothing about their domains, so a negative length or a NaN count is not turned into an error
    /// here -- it is what the game computed, and Steam is the one that stores it.
    pub fn update_avg_rate_stat(
        &self,
        stat: &str,
        count_this_session: f32,
        session_length: f64,
    ) -> SteamResult {
        let stat = c_string_arg(stat)?;
        let answered = unsafe {
            sys::SteamAPI_ISteamUserStats_UpdateAvgRateStat(
                self.user_stats,
                stat.as_ptr(),
                count_this_session,
                session_length,
            )
        };
        super::user_reads::read_answer(answered, ())
    }
}
