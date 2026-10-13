use super::*;

/// Access to the game server stats interface
///
/// Lets a dedicated server read and write the stats and achievements of users
/// who have played on it, after loading them with
/// [`GameServerStats::request_user_stats`].
pub struct GameServerStats {
    pub(crate) gs: *mut sys::ISteamGameServerStats,
    pub(crate) _inner: Arc<Inner>,
}

fn name(value: &str) -> Result<CString, SteamError> {
    CString::new(value).map_err(|_| SteamError::InvalidParameter)
}

impl GameServerStats {
    /// Loads a user's stats from Steam so they become readable here.
    ///
    /// Every `get_user_*` call below is only meaningful afterwards; Steam
    /// unloads idle users again, which surfaces as
    /// [`GameServerStatsUnloaded`].
    pub fn request_user_stats<F>(&self, user: SteamId, callback: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call =
                sys::SteamAPI_ISteamGameServerStats_RequestUserStats(self.gs, user.raw());
            register_call_result::<sys::GSStatsReceived_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult)
                    });
                },
            );
        }
    }

    /// Reads a 32-bit integer stat for a user.
    pub fn get_user_stat_i32(&self, user: SteamId, stat: &str) -> Result<i32, SteamError> {
        let stat = name(stat)?;
        let mut data = 0i32;
        unsafe {
            if !sys::SteamAPI_ISteamGameServerStats_GetUserStatInt32(
                self.gs,
                user.raw(),
                stat.as_ptr(),
                &mut data,
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(data)
    }

    /// Reads a float stat for a user.
    pub fn get_user_stat_f32(&self, user: SteamId, stat: &str) -> Result<f32, SteamError> {
        let stat = name(stat)?;
        let mut data = 0f32;
        unsafe {
            if !sys::SteamAPI_ISteamGameServerStats_GetUserStatFloat(
                self.gs,
                user.raw(),
                stat.as_ptr(),
                &mut data,
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(data)
    }

    /// Reads whether a user has earned an achievement.
    pub fn get_user_achievement(
        &self,
        user: SteamId,
        achievement: &str,
    ) -> Result<bool, SteamError> {
        let achievement = name(achievement)?;
        let mut achieved = false;
        unsafe {
            if !sys::SteamAPI_ISteamGameServerStats_GetUserAchievement(
                self.gs,
                user.raw(),
                achievement.as_ptr(),
                &mut achieved,
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(achieved)
    }

    /// Sets a 32-bit integer stat for a user.
    ///
    /// Only queued until [`GameServerStats::store_user_stats`] succeeds.
    pub fn set_user_stat_i32(
        &self,
        user: SteamId,
        stat: &str,
        data: i32,
    ) -> Result<(), SteamError> {
        let stat = name(stat)?;
        unsafe {
            if !sys::SteamAPI_ISteamGameServerStats_SetUserStatInt32(
                self.gs,
                user.raw(),
                stat.as_ptr(),
                data,
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Sets a float stat for a user.
    ///
    /// Only queued until [`GameServerStats::store_user_stats`] succeeds.
    pub fn set_user_stat_f32(
        &self,
        user: SteamId,
        stat: &str,
        data: f32,
    ) -> Result<(), SteamError> {
        let stat = name(stat)?;
        unsafe {
            if !sys::SteamAPI_ISteamGameServerStats_SetUserStatFloat(
                self.gs,
                user.raw(),
                stat.as_ptr(),
                data,
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Records a sample for an average-rate stat (e.g. kills per minute).
    ///
    /// `session_length` is the elapsed session length in seconds. Only queued
    /// until [`GameServerStats::store_user_stats`] succeeds.
    pub fn update_user_avg_rate_stat(
        &self,
        user: SteamId,
        stat: &str,
        count_this_session: f32,
        session_length: f64,
    ) -> Result<(), SteamError> {
        let stat = name(stat)?;
        unsafe {
            if !sys::SteamAPI_ISteamGameServerStats_UpdateUserAvgRateStat(
                self.gs,
                user.raw(),
                stat.as_ptr(),
                count_this_session,
                session_length,
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Marks an achievement as earned for a user.
    ///
    /// Only queued until [`GameServerStats::store_user_stats`] succeeds.
    pub fn set_user_achievement(&self, user: SteamId, achievement: &str) -> Result<(), SteamError> {
        let achievement = name(achievement)?;
        unsafe {
            if !sys::SteamAPI_ISteamGameServerStats_SetUserAchievement(
                self.gs,
                user.raw(),
                achievement.as_ptr(),
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Clears an achievement for a user.
    ///
    /// Only queued until [`GameServerStats::store_user_stats`] succeeds.
    pub fn clear_user_achievement(
        &self,
        user: SteamId,
        achievement: &str,
    ) -> Result<(), SteamError> {
        let achievement = name(achievement)?;
        unsafe {
            if !sys::SteamAPI_ISteamGameServerStats_ClearUserAchievement(
                self.gs,
                user.raw(),
                achievement.as_ptr(),
            ) {
                return Err(SteamError::InvalidParameter);
            }
        }
        Ok(())
    }

    /// Persists the queued changes for a user.
    pub fn store_user_stats<F>(&self, user: SteamId, callback: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamGameServerStats_StoreUserStats(self.gs, user.raw());
            register_call_result::<sys::GSStatsStored_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult)
                    });
                },
            );
        }
    }
}

fn user_of(raw: sys::CSteamID) -> SteamId {
    // Reading the union arm is the crate-wide pattern for CSteamID (see
    // friends.rs); SteamID and AccountID share the same bit layout.
    SteamId::from_raw(unsafe { raw.m_steamid.m_unAll64Bits })
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// A user's stats finished loading on this game server
pub struct GameServerStatsReceived {
    /// The user whose stats arrived
    pub user: SteamId,
    /// The result of the load
    pub result: Result<(), SteamError>,
}

impl_callback!(cb: GSStatsReceived_t => GameServerStatsReceived {
    Self {
        user: user_of(cb.m_steamIDUser),
        result: to_steam_result(cb.m_eResult),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// A user's stats were stored from this game server
pub struct GameServerStatsStored {
    /// The user whose stats were written
    pub user: SteamId,
    /// The result of the store
    pub result: Result<(), SteamError>,
}

impl_callback!(cb: GSStatsStored_t => GameServerStatsStored {
    Self {
        user: user_of(cb.m_steamIDUser),
        result: to_steam_result(cb.m_eResult),
    }
});

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
/// Steam unloaded a user's session; their stats must be requested again
pub struct GameServerStatsUnloaded {
    /// The user that was unloaded
    pub user: SteamId,
}

impl_callback!(cb: GSStatsUnloaded_t => GameServerStatsUnloaded {
    Self {
        user: user_of(cb.m_steamIDUser),
    }
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_names_reject_interior_nul() {
        // Upstream's user_stats path panics on these; the server path must not.
        assert!(matches!(name("kills\0"), Err(SteamError::InvalidParameter)));
        assert!(name("kills").is_ok());
        assert!(name("").is_ok());
    }

    #[test]
    fn steam_id_survives_the_csteamid_round_trip() {
        // Guards the union read in user_of against a layout or arm mistake.
        for bits in [0u64, 1, 76561197960287930, u64::MAX] {
            let raw = sys::CSteamID {
                m_steamid: sys::CSteamID_SteamID_t {
                    m_unAll64Bits: bits,
                },
            };
            assert_eq!(user_of(raw).raw(), bits);
        }
    }
}
