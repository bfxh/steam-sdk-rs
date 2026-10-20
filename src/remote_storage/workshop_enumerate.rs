//! The five enumerators of the legacy Workshop surface, and the one judgement they all share: a
//! count Steam writes into a fixed 50-entry array.
//!
//! Every result here has the same shape -- `EResult m_eResult;`, "int32 m_nResultsReturned;" and
//! "int32 m_nTotalResultCount;" over
//! `PublishedFileId_t m_rgPublishedFileId[ k_unEnumeratePublishedFilesMaxResults ];`
//! (`isteamremotestorage.h:389-392`, `:413-417`, `:499-503`, `:598-603`) -- and the capacity is Steam's
//! own constant, "const uint32 k_unEnumeratePublishedFilesMaxResults = 50;" (`:55`). The count and the
//! array come from the same untrusted writer, so the slice length is taken from *both*: a
//! `m_nResultsReturned` larger than 50, or negative, would otherwise index outside the array or read
//! the whole thing as a huge page. [`page_len`] is the one rule for all four pages, and every page
//! slices its parallel arrays by that same number so an id cannot be paired with another item's time
//! or score.
//!
//! `m_nTotalResultCount` is the size of the whole result set, not of this page (`:390-391` are two
//! different numbers), so it is clamped and kept: a caller walks pages until it has seen `total` items.

use super::workshop_answers::{action_from_code, action_to_sys};
use super::workshop_publish::{encode_tags, tag_array};
use super::*;
use std::os::raw::c_int;

/// How many entries of Steam's fixed array this answer actually filled.
///
/// Steam writes the number as "int32 m_nResultsReturned;" (`isteamremotestorage.h:390`) into an array
/// whose capacity is its own "const uint32 k_unEnumeratePublishedFilesMaxResults = 50;" (`:55`), so the
/// bound is the smaller of the two once the signed value is refused: a negative is zero entries, not
/// 1.8e19 of them, and a count above the capacity is the capacity. Nothing here trusts the count more
/// than the array it is supposed to index.
fn page_len(returned: c_int, capacity: usize) -> usize {
    usize::try_from(returned).unwrap_or(0).min(capacity)
}

/// The size of the whole result set, from Steam's "int32 m_nTotalResultCount;"
/// (`isteamremotestorage.h:391`) -- a different number from the page length above (`:390`).
///
/// A negative is not a count of anything, and handing a caller `u64::MAX` would make its pagination
/// loop look infinite rather than failed. Zero is what an empty set already looks like.
fn total_results(raw: c_int) -> u64 {
    u64::try_from(raw).unwrap_or(0)
}

/// The first `returned` ids of a page, with the array's own length as the hard bound.
fn ids_page(returned: c_int, ids: &[u64]) -> Vec<PublishedFileId> {
    ids[..page_len(returned, ids.len())]
        .iter()
        .copied()
        .map(PublishedFileId)
        .collect()
}

/// One page of items, with the total Steam says exists.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkshopPage {
    pub published_file_ids: Vec<PublishedFileId>,
    pub total_results: u64,
}

/// A page of subscribed items. `subscribed_at` is the second array of Steam's
/// "uint32 m_rgRTimeSubscribed" (`isteamremotestorage.h:417`), sized by the same 50-entry capacity as
/// the ids beside it, passed through as written -- that line carries no comment and the
/// header states no epoch or unit, so converting it to a clock here would be a guess; the same
/// passthrough rule already holds for `RTime32` fields in `src/user/answers.rs`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubscribedPage {
    pub entries: Vec<(PublishedFileId, u32)>,
    pub total_results: u64,
}

/// A page of items with the timestamps Steam pairs them with (`m_rgRTimeUpdated`, `:603`), plus the
/// action the page was filtered on -- which is Steam's answer (`m_eAction`, `:599`), not a copy of the
/// argument, and reads back through [`WorkshopFileAction`] including its `Unrecognized` arm.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkshopActionPage {
    pub entries: Vec<(PublishedFileId, u32)>,
    pub action: WorkshopFileAction,
    pub total_results: u64,
}

/// A page from the public view of the Workshop, with Steam's per-item score (`m_rgScore`, `:503`),
/// the app the query ran under (`m_nAppId`, `:504`) and the start index Steam echo back (`:505`).
#[derive(Clone, Debug, PartialEq)]
pub struct WorkshopSearchPage {
    pub entries: Vec<(PublishedFileId, f32)>,
    pub app_id: AppId,
    pub start_index: u32,
    pub total_results: u64,
}

/// Which ordering the public view should list items in.
///
/// The header's "enum EWorkshopEnumerationType" (`isteamremotestorage.h:126-135`) names seven codes
/// and no
/// out-of-range member, and nothing in the header answers with this enum --
/// `RemoteStorageEnumerateWorkshopFilesResult_t` (`:496-506`) carries no ordering field -- so this
/// vocabulary is write-only, exactly like [`crate::WorkshopVideoProvider`] and for the same measured
/// reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WorkshopEnumeration {
    /// "k_EWorkshopEnumerationTypeRankedByVote = 0," (`isteamremotestorage.h:128`).
    RankedByVote,
    /// "k_EWorkshopEnumerationTypeRecent = 1," (`:129`).
    Recent,
    /// "k_EWorkshopEnumerationTypeTrending = 2," (`:130`).
    Trending,
    /// "k_EWorkshopEnumerationTypeFavoritesOfFriends = 3," (`:131`).
    FavoritesOfFriends,
    /// "k_EWorkshopEnumerationTypeVotedByFriends = 4," (`:132`).
    VotedByFriends,
    /// "k_EWorkshopEnumerationTypeContentByFriends = 5," (`:133`).
    ContentByFriends,
    /// "k_EWorkshopEnumerationTypeRecentFromFollowedUsers = 6," (`:134`).
    RecentFromFollowedUsers,
}

/// Writes an ordering. Total, because the seven variants above are the seven Steam names.
fn enumeration_to_sys(kind: WorkshopEnumeration) -> sys::EWorkshopEnumerationType {
    match kind {
        WorkshopEnumeration::RankedByVote => {
            sys::EWorkshopEnumerationType::k_EWorkshopEnumerationTypeRankedByVote
        }
        WorkshopEnumeration::Recent => {
            sys::EWorkshopEnumerationType::k_EWorkshopEnumerationTypeRecent
        }
        WorkshopEnumeration::Trending => {
            sys::EWorkshopEnumerationType::k_EWorkshopEnumerationTypeTrending
        }
        WorkshopEnumeration::FavoritesOfFriends => {
            sys::EWorkshopEnumerationType::k_EWorkshopEnumerationTypeFavoritesOfFriends
        }
        WorkshopEnumeration::VotedByFriends => {
            sys::EWorkshopEnumerationType::k_EWorkshopEnumerationTypeVotedByFriends
        }
        WorkshopEnumeration::ContentByFriends => {
            sys::EWorkshopEnumerationType::k_EWorkshopEnumerationTypeContentByFriends
        }
        WorkshopEnumeration::RecentFromFollowedUsers => {
            sys::EWorkshopEnumerationType::k_EWorkshopEnumerationTypeRecentFromFollowedUsers
        }
    }
}

/// What to ask the public view for, as `EnumeratePublishedWorkshopFiles` takes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkshopSearch {
    pub enumeration: WorkshopEnumeration,
    pub start_index: u32,
    /// Steam's `unCount`, bounded by its own "const uint32 k_unEnumeratePublishedFilesMaxResults =
    /// 50;" (`isteamremotestorage.h:55`): asking for more is capped by the result struct, not by this
    /// wrapper, and the page length is clamped again on the way back in (`page_len`).
    pub count: u32,
    pub days: u32,
    pub tags: Vec<String>,
    pub user_tags: Vec<String>,
}

impl RemoteStorage {
    /// List the items this user published, one page at a time, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t EnumerateUserPublishedFiles( uint32 unStartIndex ) = 0;"
    /// (`isteamremotestorage.h:285`); the header annotates it with
    /// `RemoteStorageEnumerateUserPublishedFilesResult_t` (`:284`), whose shape the module note
    /// describes. `EnumerateUserSharedWorkshopFiles` answers with the same struct (`:299`).
    pub fn enumerate_user_published_files<F>(&self, start_index: u32, cb: F)
    where
        F: FnOnce(Result<WorkshopPage, SteamError>) + 'static + Send,
    {
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_EnumerateUserPublishedFiles(self.rs, start_index)
        };
        unsafe {
            register_call_result::<sys::RemoteStorageEnumerateUserPublishedFilesResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    if io_error {
                        return cb(Err(SteamError::IOFailure));
                    }
                    // Copied before being sliced: `m_rgPublishedFileId` is a field of a struct that
                    // the vendored bindings declare `#[repr(C, packed(4))]` for Linux and macOS, and a
                    // `&field` there is an unaligned reference -- E0793, which only those targets raise.
                    let ids = v.m_rgPublishedFileId;
                    cb(to_steam_result(v.m_eResult).map(|()| WorkshopPage {
                        published_file_ids: ids_page(v.m_nResultsReturned, &ids),
                        total_results: total_results(v.m_nTotalResultCount),
                    }))
                },
            );
        }
    }

    /// List the items this user is subscribed to, with Steam's subscribe timestamps, through `cb`.
    ///
    /// "virtual SteamAPICall_t EnumerateUserSubscribedFiles( uint32 unStartIndex ) = 0;"
    /// (`isteamremotestorage.h:289`) answering `RemoteStorageEnumerateUserSubscribedFilesResult_t`
    /// (`:288`). Both arrays are sliced by the same page length, so an id never pairs with another
    /// item's timestamp.
    pub fn enumerate_user_subscribed_files<F>(&self, start_index: u32, cb: F)
    where
        F: FnOnce(Result<SubscribedPage, SteamError>) + 'static + Send,
    {
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_EnumerateUserSubscribedFiles(self.rs, start_index)
        };
        unsafe {
            register_call_result::<sys::RemoteStorageEnumerateUserSubscribedFilesResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    if io_error {
                        return cb(Err(SteamError::IOFailure));
                    }
                    cb(to_steam_result(v.m_eResult).map(|()| {
                        // Both arrays are copied out before slicing, for the packed-field reason above.
                        let ids = v.m_rgPublishedFileId;
                        let times = v.m_rgRTimeSubscribed;
                        let len = page_len(v.m_nResultsReturned, ids.len());
                        SubscribedPage {
                            entries: ids[..len]
                                .iter()
                                .copied()
                                .map(PublishedFileId)
                                .zip(times[..len].iter().copied())
                                .collect(),
                            total_results: total_results(v.m_nTotalResultCount),
                        }
                    }))
                },
            );
        }
    }

    /// List the items another user has shared, filtered by tag, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t EnumerateUserSharedWorkshopFiles( CSteamID steamId, uint32
    /// unStartIndex, SteamParamStringArray_t *pRequiredTags, SteamParamStringArray_t *pExcludedTags )
    /// = 0;" (`isteamremotestorage.h:300`). Both tag lists are encoded before the call, so one
    /// unencodable tag refuses the whole query instead of running it with a filter the caller did not
    /// ask for.
    pub fn enumerate_user_shared_workshop_files<F>(
        &self,
        user: SteamId,
        start_index: u32,
        required_tags: &[String],
        excluded_tags: &[String],
        cb: F,
    ) where
        F: FnOnce(Result<WorkshopPage, SteamError>) + 'static + Send,
    {
        let required = match encode_tags(required_tags) {
            Ok(required) => required,
            Err(error) => return cb(Err(error)),
        };
        let excluded = match encode_tags(excluded_tags) {
            Ok(excluded) => excluded,
            Err(error) => return cb(Err(error)),
        };
        let (_required_pointers, mut required_raw) = tag_array(&required);
        let (_excluded_pointers, mut excluded_raw) = tag_array(&excluded);
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_EnumerateUserSharedWorkshopFiles(
                self.rs,
                user.raw(),
                start_index,
                &mut required_raw,
                &mut excluded_raw,
            )
        };
        unsafe {
            register_call_result::<sys::RemoteStorageEnumerateUserPublishedFilesResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    // Steam marshals its string arguments during the call -- the same assumption the
                    // crate already makes at src/ugc/update.rs:319-321, where a change note is dropped
                    // when `submit` returns -- so nothing here is captured by the closure: the C strings
                    // and their pointer array live in this frame across the call and go when it ends.
                    if io_error {
                        return cb(Err(SteamError::IOFailure));
                    }
                    // Copied before being sliced: `m_rgPublishedFileId` is a field of a struct that
                    // the vendored bindings declare `#[repr(C, packed(4))]` for Linux and macOS, and a
                    // `&field` there is an unaligned reference -- E0793, which only those targets raise.
                    let ids = v.m_rgPublishedFileId;
                    cb(to_steam_result(v.m_eResult).map(|()| WorkshopPage {
                        published_file_ids: ids_page(v.m_nResultsReturned, &ids),
                        total_results: total_results(v.m_nTotalResultCount),
                    }))
                },
            );
        }
    }

    /// List the items this user took a given action on, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t EnumeratePublishedFilesByUserAction( EWorkshopFileAction eAction,
    /// uint32 unStartIndex ) = 0;" (`isteamremotestorage.h:306`); an action code this crate cannot name
    /// is refused before the call (see [`WorkshopFileAction`]), and the answer carries back the action
    /// Steam filtered on (`:599`).
    pub fn enumerate_published_files_by_user_action<F>(
        &self,
        action: WorkshopFileAction,
        start_index: u32,
        cb: F,
    ) where
        F: FnOnce(Result<WorkshopActionPage, SteamError>) + 'static + Send,
    {
        let action_sys = match action_to_sys(action) {
            Ok(action_sys) => action_sys,
            Err(error) => return cb(Err(error)),
        };
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_EnumeratePublishedFilesByUserAction(
                self.rs,
                action_sys,
                start_index,
            )
        };
        unsafe {
            register_call_result::<sys::RemoteStorageEnumeratePublishedFilesByUserActionResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    if io_error {
                        return cb(Err(SteamError::IOFailure));
                    }
                    cb(to_steam_result(v.m_eResult).map(|()| {
                        let ids = v.m_rgPublishedFileId;
                        let times = v.m_rgRTimeUpdated;
                        let len = page_len(v.m_nResultsReturned, ids.len());
                        WorkshopActionPage {
                            entries: ids[..len]
                                .iter()
                                .copied()
                                .map(PublishedFileId)
                                .zip(times[..len].iter().copied())
                                .collect(),
                            action: action_from_code(v.m_eAction as c_int),
                            total_results: total_results(v.m_nTotalResultCount),
                        }
                    }))
                },
            );
        }
    }

    /// Search the public view of the Workshop, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t EnumeratePublishedWorkshopFiles( EWorkshopEnumerationType
    /// eEnumerationType, uint32 unStartIndex, uint32 unCount, uint32 unDays, SteamParamStringArray_t
    /// *pTags, SteamParamStringArray_t *pUserTags ) = 0;" (`isteamremotestorage.h:309`), under the
    /// header's own "this method enumerates the public view of workshop files" (`:307`). A bad tag in
    /// either list refuses the search before anything is sent.
    pub fn enumerate_published_workshop_files<F>(&self, query: &WorkshopSearch, cb: F)
    where
        F: FnOnce(Result<WorkshopSearchPage, SteamError>) + 'static + Send,
    {
        let tags = match encode_tags(&query.tags) {
            Ok(tags) => tags,
            Err(error) => return cb(Err(error)),
        };
        let user_tags = match encode_tags(&query.user_tags) {
            Ok(user_tags) => user_tags,
            Err(error) => return cb(Err(error)),
        };
        let (_tag_pointers, mut tags_raw) = tag_array(&tags);
        let (_user_pointers, mut users_raw) = tag_array(&user_tags);
        let kind = enumeration_to_sys(query.enumeration);
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_EnumeratePublishedWorkshopFiles(
                self.rs,
                kind,
                query.start_index,
                query.count,
                query.days,
                &mut tags_raw,
                &mut users_raw,
            )
        };
        unsafe {
            register_call_result::<sys::RemoteStorageEnumerateWorkshopFilesResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    // Steam marshals its string arguments during the call -- the same assumption the
                    // crate already makes at src/ugc/update.rs:319-321, where a change note is dropped
                    // when `submit` returns -- so nothing here is captured by the closure: the C strings
                    // and their pointer array live in this frame across the call and go when it ends.
                    if io_error {
                        return cb(Err(SteamError::IOFailure));
                    }
                    cb(to_steam_result(v.m_eResult).map(|()| {
                        let ids = v.m_rgPublishedFileId;
                        let scores = v.m_rgScore;
                        let len = page_len(v.m_nResultsReturned, ids.len());
                        WorkshopSearchPage {
                            entries: ids[..len]
                                .iter()
                                .copied()
                                .map(PublishedFileId)
                                .zip(scores[..len].iter().copied())
                                .collect(),
                            app_id: AppId(v.m_nAppId),
                            start_index: v.m_unStartIndex,
                            total_results: total_results(v.m_nTotalResultCount),
                        }
                    }))
                },
            );
        }
    }
}

#[cfg(test)]
mod workshop_enumerate_tests {
    //! The page rules: a count that exceeds the array, a count below zero, and the pairing of parallel
    //! arrays by one length. Which items a live query actually returns, and whether Steam ever writes
    //! a count above its own capacity, need a running client and are registered as unverified.

    use super::{enumeration_to_sys, ids_page, page_len, total_results, WorkshopEnumeration};
    use crate::PublishedFileId;
    use std::os::raw::c_int;
    use steamworks_sys as sys;

    const CAPACITY: usize = 50;

    /// Steam's count is bounded by the array it indexes, in both directions.
    #[test]
    fn a_page_count_never_outruns_the_array_it_indexes() {
        assert_eq!(page_len(0, CAPACITY), 0);
        assert_eq!(page_len(3, CAPACITY), 3);
        assert_eq!(page_len(50, CAPACITY), 50);
        assert_eq!(
            page_len(51, CAPACITY),
            50,
            "the array holds 50 (isteamremotestorage.h:55)"
        );
        assert_eq!(page_len(CAPACITY as c_int + 1000, CAPACITY), CAPACITY);
        assert_eq!(
            page_len(-1, CAPACITY),
            0,
            "a negative is no entries, not the whole range"
        );
        assert_eq!(page_len(c_int::MAX, CAPACITY), CAPACITY);
        assert_eq!(page_len(c_int::MIN, CAPACITY), 0);
    }

    /// The slice handed back is the page, element for element, and never longer than the array.
    #[test]
    fn the_page_reads_the_entries_steam_filled_and_stops_there() {
        let ids: Vec<u64> = (1..=CAPACITY as u64).collect();
        assert_eq!(ids_page(3, &ids).len(), 3);
        assert_eq!(
            ids_page(3, &ids),
            vec![PublishedFileId(1), PublishedFileId(2), PublishedFileId(3)]
        );
        assert_eq!(ids_page(-1, &ids), Vec::<PublishedFileId>::new());
        assert_eq!(ids_page(99, &ids).len(), CAPACITY);
        assert_eq!(
            ids_page(5, &[]),
            Vec::<PublishedFileId>::new(),
            "an empty array bounds to zero"
        );
    }

    /// The total is a separate number from the page size, and is clamped the same way.
    #[test]
    fn a_total_that_cannot_be_a_count_is_zero_not_a_huge_number() {
        assert_eq!(total_results(0), 0);
        assert_eq!(total_results(1200), 1200);
        assert_eq!(total_results(-1), 0);
        assert_ne!(total_results(-1), u64::MAX);
    }

    /// The seven orderings land on the binding's own variants, and the binding's numbers are the ones
    /// the header spells. Both halves are asserted: a re-sorted list cannot pass here by relabelling,
    /// and the wrapper is never graded against a literal this crate typed itself.
    #[test]
    fn the_seven_orderings_map_to_the_codes_the_header_lists_them_by() {
        use sys::EWorkshopEnumerationType as Sys;
        let cases = [
            (
                WorkshopEnumeration::RankedByVote,
                Sys::k_EWorkshopEnumerationTypeRankedByVote,
                0,
            ),
            (
                WorkshopEnumeration::Recent,
                Sys::k_EWorkshopEnumerationTypeRecent,
                1,
            ),
            (
                WorkshopEnumeration::Trending,
                Sys::k_EWorkshopEnumerationTypeTrending,
                2,
            ),
            (
                WorkshopEnumeration::FavoritesOfFriends,
                Sys::k_EWorkshopEnumerationTypeFavoritesOfFriends,
                3,
            ),
            (
                WorkshopEnumeration::VotedByFriends,
                Sys::k_EWorkshopEnumerationTypeVotedByFriends,
                4,
            ),
            (
                WorkshopEnumeration::ContentByFriends,
                Sys::k_EWorkshopEnumerationTypeContentByFriends,
                5,
            ),
            (
                WorkshopEnumeration::RecentFromFollowedUsers,
                Sys::k_EWorkshopEnumerationTypeRecentFromFollowedUsers,
                6,
            ),
        ];
        for (kind, sys_kind, header_code) in cases {
            assert_eq!(
                enumeration_to_sys(kind) as c_int,
                sys_kind as c_int,
                "this wrapper must send the variant the header numbers {}",
                header_code
            );
            assert_eq!(
                sys_kind as c_int, header_code,
                "isteamremotestorage.h:128-134 spells these numbers"
            );
        }
    }
}
