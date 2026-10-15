//! The three asynchronous asks: an officer list, a follower count, and whether one user
//! follows another. Each validates its arguments before reaching Steam and answers through a
//! callback.

use super::*;

impl Friends {
    /// Fetches a clan's officer list. Until the answer arrives,
    /// [`Friends::clan_officer_count`] and [`Friends::clan_officers`] report
    /// nothing useful for that clan.
    pub fn request_clan_officer_list<F>(&self, clan: SteamId, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<ClanOfficerListResponse, SteamError>) + 'static + Send,
    {
        let api_call =
            unsafe { sys::SteamAPI_ISteamFriends_RequestClanOfficerList(self.friends, clan.raw()) };
        unsafe {
            register_call_result::<sys::ClanOfficerListResponse_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        Ok(ClanOfficerListResponse {
                            clan: SteamId::from_raw(v.m_steamIDClan.m_steamid.m_unAll64Bits),
                            officer_count: v.m_cOfficers,
                            success: v.m_bSuccess != 0,
                        })
                    })
                },
            );
        }
        Ok(())
    }

    /// Asks Steam how many users follow `user`; the answer arrives as
    /// [`FollowerCount`]. The sibling question is [`Friends::is_following`] --
    /// they are separate round-trips because Steam answers them separately.
    pub fn follower_count<F>(&self, user: SteamId, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<FollowerCount, SteamError>) + 'static + Send,
    {
        let api_call =
            unsafe { sys::SteamAPI_ISteamFriends_GetFollowerCount(self.friends, user.raw()) };
        unsafe {
            register_call_result::<sys::FriendsGetFollowerCount_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult).map(|()| FollowerCount {
                            user: SteamId::from_raw(v.m_steamID.m_steamid.m_unAll64Bits),
                            count: v.m_nCount,
                        })
                    })
                },
            );
        }
        Ok(())
    }

    /// Asks whether the local user follows `other`; the answer arrives as
    /// [`IsFollowingResult`].
    pub fn is_following<F>(&self, other: SteamId, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<IsFollowingResult, SteamError>) + 'static + Send,
    {
        let api_call =
            unsafe { sys::SteamAPI_ISteamFriends_IsFollowing(self.friends, other.raw()) };
        unsafe {
            register_call_result::<sys::FriendsIsFollowing_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult).map(|()| IsFollowingResult {
                            target: SteamId::from_raw(v.m_steamID.m_steamid.m_unAll64Bits),
                            following: v.m_bIsFollowing,
                        })
                    })
                },
            );
        }
        Ok(())
    }
}
