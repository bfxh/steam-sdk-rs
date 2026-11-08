//! Leaderboards: the async handles that name one, the calls that read or upload a score, and
//! the value types that describe one (sort order, display type, an entry, an upload result).
//! The manual `test()` at the bottom is the end-to-end leaderboard round-trip against Steam.

use super::*;
use crate::utils::c_string_prefix;

/// How many downloaded entries the callback may iterate and reserve for.
///
/// `m_cEntryCount` is "the number of entries downloaded" (`isteamuserstats.h:385`) and Steam's own
/// sample iterates "use a for loop from 0 to LeaderboardScoresDownloaded_t::m_cEntryCount" (`:220`). It
/// is a plain `int`, and the old code fed it to `Vec::with_capacity(len as usize)`: a negative answer
/// became ~1.8e19 and an enormous positive one a multi-gigabyte reservation, either of which aborts --
/// here, inside the callback Steam is dispatching, where a panic cannot be caught by the caller. The
/// count is therefore only ever trusted as far as a non-negative number, and the entries vector is
/// filled by pushing rather than by pre-reserving a stranger's arithmetic.
fn entry_count(raw: ::std::os::raw::c_int) -> usize {
    usize::try_from(raw).unwrap_or(0)
}

/// How many of an entry's detail slots were really written.
///
/// `m_cDetails` is "number of int32 details available for this entry" (`isteamuserstats.h:78`) and is
/// filled by `GetDownloadedLeaderboardEntry`, which "Returns data about a single leaderboard entry"
/// (`:219,233`) as a `bool`. Both halves matter: the value is a signed `int32`, and the vector it sizes
/// holds at most `asked` slots. Casting a negative to `usize` yields a number so large that `min`
/// selects the whole capacity, and `set_len` then hands the caller capacity's worth of memory nobody
/// wrote -- uninitialised bytes presented as leaderboard data, plus the failure case where the `bool`
/// said nothing was filled at all.
fn details_len(raw: i32, asked: usize) -> usize {
    usize::try_from(raw).unwrap_or(0).min(asked)
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LeaderboardEntry {
    pub user: SteamId,
    pub global_rank: i32,
    pub score: i32,
    pub details: Vec<i32>,
}

pub enum LeaderboardDataRequest {
    Global,
    GlobalAroundUser,
    Friends,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct LeaderboardScoreUploaded {
    pub score: i32,
    pub was_changed: bool,
    pub global_rank_new: i32,
    pub global_rank_previous: i32,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum UploadScoreMethod {
    KeepBest,
    ForceUpdate,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LeaderboardSortMethod {
    Ascending,
    Descending,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum LeaderboardDisplayType {
    Numeric,
    TimeSeconds,
    TimeMilliSeconds,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct Leaderboard(u64);

impl Leaderboard {
    /// Returns the raw 64 bit value of the leaderboard id
    ///
    /// Useful for serializing leaderboard ids over a
    /// network or to a save format.
    pub fn raw(&self) -> u64 {
        self.0
    }
}

impl UserStats {
    pub fn find_leaderboard<F>(&self, name: &str, cb: F)
    where
        F: FnOnce(Result<Option<Leaderboard>, SteamError>) + 'static + Send,
    {
        unsafe {
            let name = c_string_prefix(name);
            let api_call =
                sys::SteamAPI_ISteamUserStats_FindLeaderboard(self.user_stats, name.as_ptr());
            register_call_result::<sys::LeaderboardFindResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        Ok(if v.m_bLeaderboardFound != 0 {
                            Some(Leaderboard(v.m_hSteamLeaderboard))
                        } else {
                            None
                        })
                    })
                },
            );
        }
    }

    pub fn find_or_create_leaderboard<F>(
        &self,
        name: &str,
        sort_method: LeaderboardSortMethod,
        display_type: LeaderboardDisplayType,
        cb: F,
    ) where
        F: FnOnce(Result<Option<Leaderboard>, SteamError>) + 'static + Send,
    {
        unsafe {
            let name = c_string_prefix(name);

            let sort_method = match sort_method {
                LeaderboardSortMethod::Ascending => {
                    sys::ELeaderboardSortMethod::k_ELeaderboardSortMethodAscending
                }
                LeaderboardSortMethod::Descending => {
                    sys::ELeaderboardSortMethod::k_ELeaderboardSortMethodDescending
                }
            };

            let display_type = match display_type {
                LeaderboardDisplayType::Numeric => {
                    sys::ELeaderboardDisplayType::k_ELeaderboardDisplayTypeNumeric
                }
                LeaderboardDisplayType::TimeSeconds => {
                    sys::ELeaderboardDisplayType::k_ELeaderboardDisplayTypeTimeSeconds
                }
                LeaderboardDisplayType::TimeMilliSeconds => {
                    sys::ELeaderboardDisplayType::k_ELeaderboardDisplayTypeTimeMilliSeconds
                }
            };

            let api_call = sys::SteamAPI_ISteamUserStats_FindOrCreateLeaderboard(
                self.user_stats,
                name.as_ptr(),
                sort_method,
                display_type,
            );
            register_call_result::<sys::LeaderboardFindResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        Ok(if v.m_bLeaderboardFound != 0 {
                            Some(Leaderboard(v.m_hSteamLeaderboard))
                        } else {
                            None
                        })
                    })
                },
            );
        }
    }

    pub fn upload_leaderboard_score<F>(
        &self,
        leaderboard: &Leaderboard,
        method: UploadScoreMethod,
        score: i32,
        details: &[i32],
        cb: F,
    ) where
        F: FnOnce(Result<Option<LeaderboardScoreUploaded>, SteamError>) + 'static + Send,
    {
        unsafe {
            let method = match method {
                UploadScoreMethod::KeepBest => {
                    sys::ELeaderboardUploadScoreMethod::k_ELeaderboardUploadScoreMethodKeepBest
                }
                UploadScoreMethod::ForceUpdate => {
                    sys::ELeaderboardUploadScoreMethod::k_ELeaderboardUploadScoreMethodForceUpdate
                }
            };
            let api_call = sys::SteamAPI_ISteamUserStats_UploadLeaderboardScore(
                self.user_stats,
                leaderboard.0,
                method,
                score,
                details.as_ptr(),
                details.len() as _,
            );
            register_call_result::<sys::LeaderboardScoreUploaded_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        Ok(if v.m_bSuccess != 0 {
                            Some(LeaderboardScoreUploaded {
                                score: v.m_nScore,
                                was_changed: v.m_bScoreChanged != 0,
                                global_rank_new: v.m_nGlobalRankNew as _,
                                global_rank_previous: v.m_nGlobalRankPrevious as _,
                            })
                        } else {
                            None
                        })
                    })
                },
            );
        }
    }

    pub fn download_leaderboard_entries<F>(
        &self,
        leaderboard: &Leaderboard,
        request: LeaderboardDataRequest,
        start: usize,
        end: usize,
        max_details_len: usize,
        cb: F,
    ) where
        F: FnOnce(Result<Vec<LeaderboardEntry>, SteamError>) + 'static + Send,
    {
        let request = match request {
            LeaderboardDataRequest::Global => {
                sys::ELeaderboardDataRequest::k_ELeaderboardDataRequestGlobal
            }
            LeaderboardDataRequest::GlobalAroundUser => {
                sys::ELeaderboardDataRequest::k_ELeaderboardDataRequestGlobalAroundUser
            }
            LeaderboardDataRequest::Friends => {
                sys::ELeaderboardDataRequest::k_ELeaderboardDataRequestFriends
            }
        };
        let api_call = unsafe {
            sys::SteamAPI_ISteamUserStats_DownloadLeaderboardEntries(
                self.user_stats,
                leaderboard.0,
                request,
                start as _,
                end as _,
            )
        };
        self.finish_downloaded_entries(api_call, max_details_len, cb);
    }

    /// Downloads the leaderboard entries for a specific set of users.
    ///
    /// The request is rejected with [`SteamError::InvalidParameter`] when `users` is empty
    /// a zero-length user list cannot name an entry, and handing it to Steam would produce
    /// an answer that says nothing about the users the caller meant to ask about.
    pub fn download_leaderboard_entries_for_users<F>(
        &self,
        leaderboard: &Leaderboard,
        users: &[SteamId],
        max_details_len: usize,
        cb: F,
    ) where
        F: FnOnce(Result<Vec<LeaderboardEntry>, SteamError>) + 'static + Send,
    {
        let mut raw = match users_to_csteamid(users) {
            Ok(raw) => raw,
            Err(error) => {
                cb(Err(error));
                return;
            }
        };
        let api_call = unsafe {
            sys::SteamAPI_ISteamUserStats_DownloadLeaderboardEntriesForUsers(
                self.user_stats,
                leaderboard.0,
                raw.as_mut_ptr(),
                raw.len() as _,
            )
        };
        self.finish_downloaded_entries(api_call, max_details_len, cb);
    }

    /// Attaches a piece of user-generated content to a leaderboard.
    ///
    /// The callback receives the leaderboard handle written by Steam in
    /// `LeaderboardUGCSet_t` (`isteamuserstats.h:246`, "AttachLeaderboardUGC"), so it can replace a stale handle
    /// the caller may have been holding.
    pub fn attach_leaderboard_ugc<F>(&self, leaderboard: &Leaderboard, ugc: UgcHandle, cb: F)
    where
        F: FnOnce(Result<Leaderboard, SteamError>) + 'static + Send,
    {
        let api_call = unsafe {
            sys::SteamAPI_ISteamUserStats_AttachLeaderboardUGC(
                self.user_stats,
                leaderboard.0,
                ugc.0,
            )
        };
        unsafe {
            register_call_result::<sys::LeaderboardUGCSet_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(attach_leaderboard_ugc_result(
                        v.m_eResult,
                        v.m_hSteamLeaderboard,
                        io_error,
                    ))
                },
            );
        }
    }

    fn finish_downloaded_entries<F>(
        &self,
        api_call: sys::SteamAPICall_t,
        max_details_len: usize,
        cb: F,
    ) where
        F: FnOnce(Result<Vec<LeaderboardEntry>, SteamError>) + 'static + Send,
    {
        let user_stats = self.user_stats as isize;
        unsafe {
            register_call_result::<sys::LeaderboardScoresDownloaded_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        let len = entry_count(v.m_cEntryCount);
                        let mut entries = Vec::new();
                        // Steam's size parameter is a signed `int`, so the number of slots this side is
                        // willing to hand over is clamped before the buffer is built; the cast below
                        // would otherwise be able to offer a negative capacity.
                        let asked = max_details_len.min(::std::os::raw::c_int::MAX as usize);
                        for idx in 0..len {
                            let mut entry: sys::LeaderboardEntry_t = std::mem::zeroed();
                            // Zero-filled rather than `with_capacity`: a length that comes from Steam (or
                            // from the backend behind it) must never be able to expose slots nobody wrote.
                            let mut details = vec![0i32; asked];

                            if !sys::SteamAPI_ISteamUserStats_GetDownloadedLeaderboardEntry(
                                user_stats as *mut _,
                                v.m_hSteamLeaderboardEntries,
                                idx as _,
                                &mut entry,
                                details.as_mut_ptr(),
                                asked as _,
                            ) {
                                continue;
                            }

                            details.truncate(details_len(entry.m_cDetails, asked));

                            entries.push(LeaderboardEntry {
                                user: SteamId(entry.m_steamIDUser.m_steamid.m_unAll64Bits),
                                global_rank: entry.m_nGlobalRank,
                                score: entry.m_nScore,
                                details,
                            })
                        }
                        Ok(entries)
                    })
                },
            );
        }
    }

    /// Returns the display type of a leaderboard handle. Returns `None` if the leaderboard handle is invalid.
    pub fn get_leaderboard_display_type(
        &self,
        leaderboard: &Leaderboard,
    ) -> Option<LeaderboardDisplayType> {
        unsafe {
            match sys::SteamAPI_ISteamUserStats_GetLeaderboardDisplayType(
                self.user_stats,
                leaderboard.0,
            ) {
                sys::ELeaderboardDisplayType::k_ELeaderboardDisplayTypeNumeric => {
                    Some(LeaderboardDisplayType::Numeric)
                }
                sys::ELeaderboardDisplayType::k_ELeaderboardDisplayTypeTimeSeconds => {
                    Some(LeaderboardDisplayType::TimeSeconds)
                }
                sys::ELeaderboardDisplayType::k_ELeaderboardDisplayTypeTimeMilliSeconds => {
                    Some(LeaderboardDisplayType::TimeMilliSeconds)
                }
                _ => None,
            }
        }
    }

    /// Returns the sort method of a leaderboard handle. Returns `None` if the leaderboard handle is invalid.
    pub fn get_leaderboard_sort_method(
        &self,
        leaderboard: &Leaderboard,
    ) -> Option<LeaderboardSortMethod> {
        unsafe {
            match sys::SteamAPI_ISteamUserStats_GetLeaderboardSortMethod(
                self.user_stats,
                leaderboard.0,
            ) {
                sys::ELeaderboardSortMethod::k_ELeaderboardSortMethodAscending => {
                    Some(LeaderboardSortMethod::Ascending)
                }
                sys::ELeaderboardSortMethod::k_ELeaderboardSortMethodDescending => {
                    Some(LeaderboardSortMethod::Descending)
                }
                _ => None,
            }
        }
    }

    /// Returns the name of a leaderboard handle. Returns an empty string if the leaderboard handle is invalid.
    pub fn get_leaderboard_name(&self, leaderboard: &Leaderboard) -> String {
        unsafe {
            let name = CStr::from_ptr(sys::SteamAPI_ISteamUserStats_GetLeaderboardName(
                self.user_stats,
                leaderboard.0,
            ));
            name.to_string_lossy().into()
        }
    }

    /// Returns the total number of entries in a leaderboard. Returns 0 if the leaderboard handle is invalid.
    pub fn get_leaderboard_entry_count(&self, leaderboard: &Leaderboard) -> i32 {
        unsafe {
            sys::SteamAPI_ISteamUserStats_GetLeaderboardEntryCount(self.user_stats, leaderboard.0)
        }
    }
}

/// The C `CSteamID` array `DownloadLeaderboardEntriesForUsers` expects.
///
/// An empty list is rejected before the call: Steam would answer a question about no
/// users, which is not the question an empty slice asks.
fn users_to_csteamid(users: &[SteamId]) -> Result<Vec<sys::CSteamID>, SteamError> {
    if users.is_empty() {
        return Err(SteamError::InvalidParameter);
    }
    Ok(users
        .iter()
        .map(|id| sys::CSteamID {
            m_steamid: sys::CSteamID_SteamID_t {
                m_unAll64Bits: id.raw(),
            },
        })
        .collect())
}

/// The `LeaderboardUGCSet_t` answer: the result code, plus the handle Steam wrote back.
fn attach_leaderboard_ugc_result(
    result: sys::EResult,
    leaderboard: u64,
    io_error: bool,
) -> Result<Leaderboard, SteamError> {
    if io_error {
        Err(SteamError::IOFailure)
    } else {
        crate::to_steam_result(result).map(|()| Leaderboard(leaderboard))
    }
}

#[cfg(test)]
mod tests {
    //! The two counts a downloaded leaderboard is built from, judged without a client. The FFI needs
    //! Steam, but every decision made from its answers does not, and those decisions are what used to
    //! size allocations and lengths.
    use super::*;

    /// `m_cEntryCount` fed `Vec::with_capacity(len as usize)`: a negative value became ~1.8e19 and an
    /// absurd positive one a multi-gigabyte reservation, either of which aborts inside the callback
    /// Steam is dispatching.
    #[test]
    fn a_negative_entry_count_iterates_nothing() {
        assert_eq!(entry_count(-1), 0);
        assert_eq!(entry_count(i32::MIN), 0);
    }

    /// The honest half: a count Steam really means still passes straight through.
    #[test]
    fn a_positive_entry_count_is_not_second_guessed() {
        assert_eq!(entry_count(0), 0);
        assert_eq!(entry_count(1), 1);
        assert_eq!(entry_count(50), 50);
    }

    /// `set_len(min(entry.m_cDetails as usize, capacity))` on a `with_capacity` vector: casting -1 made
    /// `min` select the whole capacity, so uninitialised bytes were handed out as leaderboard details.
    #[test]
    fn a_negative_or_oversized_detail_count_never_exceeds_the_buffer() {
        assert_eq!(details_len(-1, 8), 0);
        assert_eq!(details_len(i32::MIN, 8), 0);
        assert_eq!(details_len(1000, 8), 8);
        assert_eq!(details_len(i32::MAX, 0), 0);
    }

    /// And a truthful count is used exactly, not rounded down to some smaller policy limit.
    #[test]
    fn a_declared_detail_count_is_kept_when_it_fits() {
        assert_eq!(details_len(3, 8), 3);
        assert_eq!(details_len(8, 8), 8);
        assert_eq!(details_len(0, 8), 0);
    }

    #[test]
    fn an_empty_user_list_is_rejected_before_the_ffi() {
        assert!(matches!(
            users_to_csteamid(&[]),
            Err(SteamError::InvalidParameter)
        ));
    }

    #[test]
    fn a_user_list_is_converted_to_the_raw_ids_steam_expects() {
        let users = [SteamId::from_raw(7), SteamId::from_raw(9)];
        let raw = users_to_csteamid(&users).unwrap();
        assert_eq!(raw.len(), 2);
        // Reading the union field is the unsafe half; constructing it above is not.
        assert_eq!(unsafe { raw[0].m_steamid.m_unAll64Bits }, 7);
        assert_eq!(unsafe { raw[1].m_steamid.m_unAll64Bits }, 9);
    }

    #[test]
    fn attach_ugc_maps_the_result_code_and_the_returned_handle() {
        let ok = attach_leaderboard_ugc_result(sys::EResult::k_EResultOK, 42, false).unwrap();
        assert_eq!(ok.raw(), 42);
        assert!(matches!(
            attach_leaderboard_ugc_result(sys::EResult::k_EResultFail, 42, false),
            Err(SteamError::Generic)
        ));
        assert!(matches!(
            attach_leaderboard_ugc_result(sys::EResult::k_EResultOK, 42, true),
            Err(SteamError::IOFailure)
        ));
    }
}

#[test]
#[ignore]
#[serial]
fn test() {
    let client = Client::init().unwrap();

    let stats = client.user_stats();

    stats.find_leaderboard("steamworks_test", |lb| {
        println!("Got: {:?}", lb);
    });
    let c2 = client.clone();
    stats.find_or_create_leaderboard(
        "steamworks_test_created",
        LeaderboardSortMethod::Descending,
        LeaderboardDisplayType::TimeMilliSeconds,
        move |lb| {
            println!("Got: {:?}", lb);

            if let Some(lb) = lb.ok().and_then(|v| v) {
                c2.user_stats().upload_leaderboard_score(
                    &lb,
                    UploadScoreMethod::ForceUpdate,
                    1337,
                    &[1, 2, 3, 4],
                    |v| {
                        println!("Upload: {:?}", v);
                    },
                );
                c2.user_stats().download_leaderboard_entries(
                    &lb,
                    LeaderboardDataRequest::Global,
                    0,
                    200,
                    10,
                    |v| {
                        println!("Download: {:?}", v);
                    },
                );
            }
        },
    );

    for _ in 0..50 {
        client.run_callbacks();
        ::std::thread::sleep(::std::time::Duration::from_millis(100));
    }
}
