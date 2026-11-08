use super::*;
use crate::app::c_string_arg;

/// Achievement API.
///
/// Methods require
/// [`request_current_stats()`](../struct.UserStats.html#method.request_current_stats)
/// to have been called and a successful [`UserStatsReceived`](../struct.UserStatsReceived.html)
/// callback processed.
///
/// # Example
///
/// ```no_run
/// # use steamworks::*;
/// # let client = steamworks::Client::init().unwrap();
/// // Unlock the 'WIN_THE_GAME' achievement
/// client.user_stats().achievement("WIN_THE_GAME").set()?;
/// # Ok::<(), steamworks::SteamError>(())
/// ```
pub struct AchievementHelper<'parent> {
    pub(crate) name: CString,
    pub(crate) parent: &'parent UserStats,
}

impl AchievementHelper<'_> {
    /// Gets the unlock status of the Achievement.
    ///
    /// This call only modifies Steam's in-memory state so it is quite cheap. To send the unlock
    /// status to the server and to trigger the Steam overlay notification you must call
    /// [`store_stats()`](../struct.UserStats.html#method.store_stats).
    ///
    /// Fails if this achievement's 'API Name' is unknown, or unsuccessful
    /// [`UserStatsReceived`](../struct.UserStatsReceived.html).
    pub fn get(&self) -> SteamResult<bool> {
        unsafe {
            let mut achieved = false;
            let success = sys::SteamAPI_ISteamUserStats_GetAchievement(
                self.parent.user_stats,
                self.name.as_ptr(),
                &mut achieved,
            );
            if success {
                Ok(achieved)
            } else {
                Err(SteamError::InvalidParameter)
            }
        }
    }

    /// Unlocks an achievement.
    ///
    /// This call only modifies Steam's in-memory state so it is quite cheap. To send the unlock
    /// status to the server and to trigger the Steam overlay notification you must call
    /// [`store_stats()`](../struct.UserStats.html#method.store_stats).
    ///
    /// Fails if this achievement's 'API Name' is unknown, or unsuccessful
    /// [`UserStatsReceived`](../struct.UserStatsReceived.html).
    pub fn set(&self) -> SteamResult<()> {
        let success = unsafe {
            sys::SteamAPI_ISteamUserStats_SetAchievement(self.parent.user_stats, self.name.as_ptr())
        };
        if success {
            Ok(())
        } else {
            Err(SteamError::InvalidParameter)
        }
    }

    /// Resets the unlock status of an achievement.
    ///
    /// This call only modifies Steam's in-memory state so it is quite cheap. To send the unlock
    /// status to the server and to trigger the Steam overlay notification you must call
    /// [`store_stats()`](../struct.UserStats.html#method.store_stats).
    ///
    /// Fails if this achievement's 'API Name' is unknown, or unsuccessful
    /// [`UserStatsReceived`](../struct.UserStatsReceived.html).
    pub fn clear(&self) -> SteamResult<()> {
        let success = unsafe {
            sys::SteamAPI_ISteamUserStats_ClearAchievement(
                self.parent.user_stats,
                self.name.as_ptr(),
            )
        };
        if success {
            Ok(())
        } else {
            Err(SteamError::InvalidParameter)
        }
    }

    /// Returns the percentage of users who have unlocked the specified achievement.
    ///
    /// You must have called `request_global_achievement_percentages()` and it needs to return
    /// successfully via its callback prior to calling this.
    ///
    /// *Note: Always returns an error for AppId `480` (Spacewar)!
    /// Other AppIds work fine though.*
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use steamworks::*;
    /// # let client = steamworks::Client::init().unwrap();
    /// // Get the current unlock percentage for the 'WIN_THE_GAME' achievement
    /// client.user_stats().request_global_achievement_percentages(move|result| {
    ///     if !result.is_err() {
    ///         let user_stats = client.user_stats();
    ///         let achievement = user_stats.achievement("WIN_THE_GAME");
    ///         let ach_percent = achievement.get_achievement_achieved_percent().expect("Failed to get achievement percentage");
    ///
    ///         println!("{}",ach_percent);
    ///     } else {
    ///         println!("Error requesting global achievement percentages");
    ///     }
    /// });
    /// # Ok::<(), steamworks::SteamError>(())
    /// ```
    pub fn get_achievement_achieved_percent(&self) -> SteamResult<f32> {
        unsafe {
            let mut percent = 0.0;
            let success = sys::SteamAPI_ISteamUserStats_GetAchievementAchievedPercent(
                self.parent.user_stats,
                self.name.as_ptr(),
                &mut percent,
            );
            if success {
                Ok(percent)
            } else {
                Err(SteamError::InvalidParameter)
            }
        }
    }

    /// Gets the achievement status, and the time it was unlocked if unlocked.
    ///
    /// You must call `request_current_stats() or request_user_stats(steam_user_id)` and
    /// successfully returned it's callback.
    ///
    /// If the return value is true but the unlock time is zero, that means it was unlocked before
    /// Steam began tracking achievement unlock times (December 2009). The time is provided in Unix
    /// epoch format, seconds since January 1, 1970 UTC.
    pub fn get_achievement_and_unlock_time(&self) -> SteamResult<(bool, u32)> {
        let mut achieved = false;
        let mut unlocktime = 0u32;

        let success = unsafe {
            sys::SteamAPI_ISteamUserStats_GetAchievementAndUnlockTime(
                self.parent.user_stats,
                self.name.as_ptr(),
                &mut achieved,
                &mut unlocktime,
            )
        };

        if success {
            Ok((achieved, unlocktime))
        } else {
            Err(SteamError::InvalidParameter)
        }
    }

    /// Get general attributes for an achievement. Currently provides: `Name`, `Description`,
    /// and `Hidden` status.
    ///
    /// This receives the value from a dictionary/map keyvalue store, so you must provide one
    /// of the following keys:
    ///
    /// - `"name"` to retrive the localized achievement name in UTF8
    /// - `"desc"` to retrive the localized achievement description in UTF8
    /// - `"hidden"` for retrieving if an achievement is hidden. Returns `"0"` when not hidden,
    /// `"1"` when hidden
    ///
    /// This localization is provided based on the games language if it's set, otherwise it
    /// checks if a localization is available for the users Steam UI Language. If that fails
    /// too, then it falls back to english.
    ///
    /// This function returns the value as a `string` upon success if all of the following
    /// conditions are met; otherwise, an empty string: `""`.
    ///
    /// - `request_current_stats()` has completed and successfully returned its callback.
    /// - The specified achievement exists in App Admin on the Steamworks website, and the
    /// changes are published.
    /// - The specified `pchKey` is valid.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use steamworks::*;
    /// # let client = steamworks::Client::init().unwrap();
    /// // Get the "description" string for the 'WIN_THE_GAME' achievement
    /// client.user_stats().achievement("WIN_THE_GAME").get_achievement_display_attribute("desc").unwrap();
    /// # Ok::<(), steamworks::SteamError>(())
    /// ```
    pub fn get_achievement_display_attribute(&self, key: &str) -> SteamResult<&str> {
        unsafe {
            // A nul in the caller's key is a failure to ask the question at all, which is what
            // this function's `InvalidParameter` already means.
            let key_c_str = c_string_arg(key)?;

            let str = sys::SteamAPI_ISteamUserStats_GetAchievementDisplayAttribute(
                self.parent.user_stats,
                self.name.as_ptr(),
                key_c_str.as_ptr(),
            );

            let c_str = CStr::from_ptr(str);

            match c_str.to_str() {
                Ok(result) => Ok(result),
                Err(_) => Err(SteamError::InvalidParameter),
            }
        }
    }

    /// Gets the icon for an achievement as RGBA bytes.
    ///
    /// Steam hands back an image *handle* (`isteamuserstats.h:134`: "Gets the icon of the
    /// achievement, which is a handle to be used in `ISteamUtils::GetImageRGBA()`, or 0 if
    /// none set"); this function resolves that handle into the pixel buffer and discards it,
    /// so the returned `Vec<u8>` is `width * height * 4` bytes.
    ///
    /// `None` means no image is in hand right now. Four measured paths reach it: the handle
    /// is 0, `GetImageSize` fails, the icon is not the 64x64 this getter insists on, or
    /// `GetImageRGBA` fails.
    ///
    /// A zero handle is ambiguous by design -- `isteamuserstats.h:135-137` warns that "A return
    /// value of 0 may indicate we are still
    /// fetching data, and you can wait for the `UserAchievementIconFetched_t` callback which
    /// will notify you when the bits are ready. If the callback still returns zero, then there
    /// is no image set for the specified achievement." So retry after that callback before
    /// concluding the achievement has no icon.
    pub fn get_achievement_icon(&self) -> Option<Vec<u8>> {
        Some(self.internal_get_achievement_icon(true)?.0)
    }

    /// The RGBA buffer size Steam writes for an image of `width` x `height`, and the value to pass as
    /// its size: "the destination buffer size should be 4 * height * width * sizeof(char)"
    /// (`isteamutils.h:85`).
    ///
    /// An associated function rather than an inline expression, because the arithmetic has to be
    /// checked. The product used to be `width * height * 4` in `u32` with `.try_into().unwrap()`, and
    /// the dimensions are values `ISteamUtils::GetImageSize` fills in -- not the caller's. A large
    /// image overflowed the multiplication (debug builds panic, release builds wrap), so `dest` was
    /// allocated at a length far below the buffer size handed to Steam, which the FFI then wrote past.
    /// Clamping at the `c_int` boundary is the same hazard from the other side: this is the 2GB half of
    /// the asymmetry `docs/defect-03-resize-deep-dive.md` measures on the callback path.
    fn rgba_buffer_len(width: u32, height: u32) -> Option<i32> {
        let bytes = width.checked_mul(height)?.checked_mul(4)?;
        i32::try_from(bytes).ok()
    }

    fn internal_get_achievement_icon(&self, avoid_big_icons: bool) -> Option<(Vec<u8>, u32, u32)> {
        unsafe {
            let utils: *mut sys::ISteamUtils = sys::SteamAPI_SteamUtils_v010();
            let img = sys::SteamAPI_ISteamUserStats_GetAchievementIcon(
                self.parent.user_stats,
                self.name.as_ptr(),
            );
            if img == 0 {
                return None;
            }
            let mut width = 0;
            let mut height = 0;
            if !sys::SteamAPI_ISteamUtils_GetImageSize(utils, img, &mut width, &mut height) {
                return None;
            }
            if avoid_big_icons && (width != 64 || height != 64) {
                return None;
            }
            // Steam fills the dimensions, so the size is checked rather than trusted: an image whose
            // 4*h*w does not fit the `c_int` the API takes would otherwise allocate a buffer the FFI
            // writes past.
            let Some(len) = Self::rgba_buffer_len(width, height) else {
                return None;
            };
            let mut dest = vec![0; len as usize];
            if !sys::SteamAPI_ISteamUtils_GetImageRGBA(utils, img, dest.as_mut_ptr(), len) {
                return None;
            }
            Some((dest, width, height))
        }
    }

    /// Gets the icon for an achievement.
    ///
    /// The image is returned as a handle to be used with `ISteamUtils::GetImageRGBA` to get
    /// the actual image data.*
    ///
    /// **Note: This is handled within the function. Returns a `ImageBuffer::<image::Rgba<u8>, Vec<u8>>` from the image crate on success**
    /// ** Note: This may return None if Steam has not retrieved the icon yet. In that case an `UserAchievementIconFetched` callback will be processed
    #[cfg(feature = "image")]
    pub fn get_achievement_icon_v2(&self) -> Option<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>> {
        let (vec, width, height) = self.internal_get_achievement_icon(false)?;
        let img = image::ImageBuffer::<image::Rgba<u8>, Vec<u8>>::from_vec(width, height, vec)?;
        return Some(img);
    }
}

#[cfg(test)]
mod tests {
    //! The size arithmetic Steam's answer depends on, judged without a client. The FFI call needs one,
    //! so the decision it feeds is the part that can be pinned here -- and that decision is what used to
    //! allocate a buffer smaller than the write Steam performs.
    use super::AchievementHelper;

    #[test]
    fn a_normal_icon_size_needs_four_bytes_per_pixel() {
        assert_eq!(AchievementHelper::rgba_buffer_len(64, 64), Some(16_384));
        assert_eq!(AchievementHelper::rgba_buffer_len(1, 1), Some(4));
        assert_eq!(AchievementHelper::rgba_buffer_len(0, 40), Some(0));
    }

    /// 32768x32768 is where `width * height * 4` left `u32`: the old expression wrapped (and panicked
    /// in debug), so `dest` came out smaller than the size handed to `GetImageRGBA`, which then wrote
    /// past it. A refusal is the only defined answer here.
    #[test]
    fn a_product_that_leaves_u32_is_refused_rather_than_wrapped() {
        assert_eq!(AchievementHelper::rgba_buffer_len(32_768, 32_768), None);
        assert_eq!(AchievementHelper::rgba_buffer_len(u32::MAX, 2), None);
    }

    /// The buffer size parameter is a signed `c_int`, so a product that fits `u32` but not `i32` would
    /// otherwise be handed over as a negative length. This is the 2GB/4GB split
    /// `docs/defect-03-resize-deep-dive.md` measures on the callback path.
    #[test]
    fn a_size_that_overflows_the_signed_parameter_is_refused() {
        assert_eq!(
            AchievementHelper::rgba_buffer_len(16_384, 16_384),
            Some(1_073_741_824)
        );
        assert_eq!(AchievementHelper::rgba_buffer_len(32_768, 16_384), None);
    }
}
