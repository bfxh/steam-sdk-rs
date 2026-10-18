//! The current player's own numbers and the lifecycle around them: ask Steam for the stats
//! (`request_user_stats`), read or write one stat, then push the batch back (`store_stats`)
//! or throw it away (`reset_all_stats`).

use super::*;
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
}
