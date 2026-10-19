//! Aggregate numbers Steam publishes about *every* player: the global stat value and its
//! per-day history, fetched by `request_global_stats`. Nothing here reads the local player's
//! own stats -- that lives in `stats_io`.

use super::*;
use crate::app::c_string_arg;

impl UserStats {
    /// Asynchronously requests global stats data, which is available for stats marked as "aggregated".
    ///
    /// This call is asynchronous, with the results returned in [`GlobalStatsReceived`] callback.
    ///
    /// # Arguments
    ///
    /// * `history_days` - Specifies how many days of day-by-day history to retrieve in addition
    ///   to the overall totals. The limit is 60.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use steamworks::*;
    /// # let client = steamworks::Client::init().unwrap();
    /// let user_stats = client.user_stats();
    ///
    /// // Request global stats with 7 days of history
    /// user_stats.request_global_stats(7, |result| {
    ///     match result {
    ///         Ok(game_id) => {
    ///             println!("Global stats received for game: {:?}", game_id);
    ///         }
    ///         Err(e) => {
    ///             println!("Failed to get global stats: {:?}", e);
    ///         }
    ///     }
    /// });
    /// ```
    pub fn request_global_stats<F>(&self, history_days: i32, cb: F)
    where
        F: FnOnce(Result<GameId, SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call =
                sys::SteamAPI_ISteamUserStats_RequestGlobalStats(self.user_stats, history_days);
            register_call_result::<sys::GlobalStatsReceived_t, _>(
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

    /// Gets the lifetime total for an aggregated stat as an `i64`.
    ///
    /// The specified stat must exist and be marked as "aggregated" in the Steamworks App Admin.
    ///
    /// Requires [`request_global_stats()`](Self::request_global_stats) to have been called
    /// and a successful [`GlobalStatsReceived`] callback processed.
    ///
    /// # Arguments
    ///
    /// * `name` - The 'API Name' of the stat. Must not be longer than `k_cchStatNameMax`.
    ///
    /// # Returns
    ///
    /// Returns `Ok(i64)` with the stat value if successful, or
    /// `Err(SteamError::InvalidParameter)` when the stat does not exist
    /// or hasn't been received yet.
    pub fn get_global_stat_i64(&self, name: &str) -> SteamResult<i64> {
        let name = c_string_arg(name)?;
        let mut value: i64 = 0;
        let success = unsafe {
            sys::SteamAPI_ISteamUserStats_GetGlobalStatInt64(
                self.user_stats,
                name.as_ptr(),
                &mut value,
            )
        };
        if success {
            Ok(value)
        } else {
            Err(SteamError::InvalidParameter)
        }
    }

    /// Gets the lifetime total for an aggregated stat as an `f64`.
    ///
    /// The specified stat must exist and be marked as "aggregated" in the Steamworks App Admin.
    ///
    /// Requires [`request_global_stats()`](Self::request_global_stats) to have been called
    /// and a successful [`GlobalStatsReceived`] callback processed.
    ///
    /// # Arguments
    ///
    /// * `name` - The 'API Name' of the stat. Must not be longer than `k_cchStatNameMax`.
    ///
    /// # Returns
    ///
    /// Returns `Ok(f64)` with the stat value if successful, or
    /// `Err(SteamError::InvalidParameter)` when the stat does not exist
    /// or hasn't been received yet.
    pub fn get_global_stat_f64(&self, name: &str) -> SteamResult<f64> {
        let name = c_string_arg(name)?;
        let mut value: f64 = 0.0;
        let success = unsafe {
            sys::SteamAPI_ISteamUserStats_GetGlobalStatDouble(
                self.user_stats,
                name.as_ptr(),
                &mut value,
            )
        };
        if success {
            Ok(value)
        } else {
            Err(SteamError::InvalidParameter)
        }
    }

    /// Gets history for an aggregated stat as `i64` values.
    ///
    /// The data will be filled with daily values, starting with today.
    /// So when called, `data[0]` will be today, `data[1]` will be yesterday, and `data[2]` will be
    /// two days ago, etc.
    ///
    /// The specified stat must exist and be marked as "aggregated" in the Steamworks App Admin.
    ///
    /// Requires [`request_global_stats()`](Self::request_global_stats) to have been called
    /// and a successful [`GlobalStatsReceived`] callback processed.
    ///
    /// # Arguments
    ///
    /// * `name` - The 'API Name' of the stat. Must not be longer than `k_cchStatNameMax`.
    /// * `max_days` - The maximum number of days of history to retrieve. This should match
    ///   or be less than the `history_days` value passed to `request_global_stats()`.
    ///
    /// # Returns
    ///
    /// Returns `Ok(Vec<i64>)` containing the daily values (from today backwards) if successful,
    /// or `Err(SteamError::InvalidParameter)` when the stat does not exist or has not been
    /// received yet.
    pub fn get_global_stat_history_i64(
        &self,
        name: &str,
        max_days: usize,
    ) -> SteamResult<Vec<i64>> {
        let name = c_string_arg(name)?;
        let mut data = vec![0i64; max_days];
        let count = unsafe {
            sys::SteamAPI_ISteamUserStats_GetGlobalStatHistoryInt64(
                self.user_stats,
                name.as_ptr(),
                data.as_mut_ptr(),
                (max_days * std::mem::size_of::<i64>()) as u32,
            )
        };
        if count >= 0 {
            data.truncate(count as usize);
            Ok(data)
        } else {
            Err(SteamError::InvalidParameter)
        }
    }

    /// Gets history for an aggregated stat as `f64` values.
    ///
    /// The data will be filled with daily values, starting with today.
    /// So when called, `data[0]` will be today, `data[1]` will be yesterday, and `data[2]` will be
    /// two days ago, etc.
    ///
    /// The specified stat must exist and be marked as "aggregated" in the Steamworks App Admin.
    ///
    /// Requires [`request_global_stats()`](Self::request_global_stats) to have been called
    /// and a successful [`GlobalStatsReceived`] callback processed.
    ///
    /// # Arguments
    ///
    /// * `name` - The 'API Name' of the stat. Must not be longer than `k_cchStatNameMax`.
    /// * `max_days` - The maximum number of days of history to retrieve. This should match
    ///   or be less than the `history_days` value passed to `request_global_stats()`.
    ///
    /// # Returns
    ///
    /// Returns `Ok(Vec<f64>)` containing the daily values (from today backwards) if successful,
    /// or `Err(SteamError::InvalidParameter)` when the stat does not exist or has not been
    /// received yet.
    pub fn get_global_stat_history_f64(
        &self,
        name: &str,
        max_days: usize,
    ) -> SteamResult<Vec<f64>> {
        let name = c_string_arg(name)?;
        let mut data = vec![0f64; max_days];
        let count = unsafe {
            sys::SteamAPI_ISteamUserStats_GetGlobalStatHistoryDouble(
                self.user_stats,
                name.as_ptr(),
                data.as_mut_ptr(),
                (max_days * std::mem::size_of::<f64>()) as u32,
            )
        };
        if count >= 0 {
            data.truncate(count as usize);
            Ok(data)
        } else {
            Err(SteamError::InvalidParameter)
        }
    }
}

#[test]
#[ignore]
#[serial]
fn test_global_stats() {
    let client = Client::init().unwrap();
    let stats = client.user_stats();

    // Get stat name from environment variable, default to "test_stat"
    let stat_name =
        std::env::var("TEST_GLOBAL_STAT_NAME").unwrap_or_else(|_| "test_stat".to_string());
    println!("Using global stat name: {}", stat_name);

    // Test request_global_stats with 7 days of history
    let c2 = client.clone();
    let stat_name_clone = stat_name.clone();
    stats.request_global_stats(7, move |result| {
        match result {
            Ok(game_id) => {
                println!("Global stats received for game: {:?}", game_id);

                // Test get_global_stat_i64
                match c2.user_stats().get_global_stat_i64(&stat_name_clone) {
                    Ok(value) => println!("Global stat (i64): {}", value),
                    Err(_) => println!(
                        "Failed to get global stat (i64) - stat may not exist or not be aggregated"
                    ),
                }

                // Test get_global_stat_f64
                match c2.user_stats().get_global_stat_f64(&stat_name_clone) {
                    Ok(value) => println!("Global stat (f64): {}", value),
                    Err(_) => println!(
                        "Failed to get global stat (f64) - stat may not exist or not be aggregated"
                    ),
                }

                // Test get_global_stat_history_i64
                match c2
                    .user_stats()
                    .get_global_stat_history_i64(&stat_name_clone, 7)
                {
                    Ok(history) => println!("Global stat history (i64): {:?}", history),
                    Err(_) => println!("Failed to get global stat history (i64)"),
                }

                // Test get_global_stat_history_f64
                match c2
                    .user_stats()
                    .get_global_stat_history_f64(&stat_name_clone, 7)
                {
                    Ok(history) => println!("Global stat history (f64): {:?}", history),
                    Err(_) => println!("Failed to get global stat history (f64)"),
                }
            }
            Err(e) => {
                println!("Failed to get global stats: {:?}", e);
            }
        }
    });

    // Run callbacks to process the async result
    for _ in 0..50 {
        client.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(100));
    }
}

#[test]
fn test_global_stat_cstring_error() {
    // Test that get_global_stat methods properly handle invalid CString input
    // This test doesn't require Steam to be running

    // Create a string with null byte which is invalid for CString
    let invalid_name = "test\0stat";

    // We can't actually test the UserStats methods without initializing Steam,
    // but we can verify the CString conversion behavior
    assert!(CString::new(invalid_name).is_err());
}
