//! The seven Workshop entries whose answer names the item back: delete, subscribe, unsubscribe,
//! the two vote-detail reads, the user's own vote, and the recorded action.
//!
//! All seven sit inside the block Steam introduces as "publishing UGC" (`isteamremotestorage.h:264`)
//! and all seven are annotated with a `STEAM_CALL_RESULT`, so each answers through the call-result
//! map rather than a return value. What is left of that block -- the two publish calls, the five
//! enumerators and `GetPublishedFileDetails` -- is registered in `docs/SuperSteam-Design.md` §6.1d:
//! those carry arrays, tag lists and a completion struct that the header does not annotate with, so
//! they get their own unit instead of being squeezed in here.
//!
//! Two readings of Steam's own text are load-bearing and are written where the code uses them:
//! the two vote-detail calls are annotated with the *same* struct (`:293` and `:297`, both
//! `RemoteStorageGetPublishedItemVoteDetailsResult_t`), and the vote struct itself carries no comment
//! on its count fields (`:517-519`), so whether a negative is possible is this wrapper's judgement,
//! not Steam's.

use super::*;
use std::os::raw::c_int;

/// What Steam reports about the votes on one published file.
///
/// The fields are "struct RemoteStorageGetPublishedItemVoteDetailsResult_t"
/// (`isteamremotestorage.h:512-520`) read as declared: the item id arrives as `m_unPublishedFileId`
/// (`:516`, the one field in this block named with the `un` prefix), the three counts as
/// `int32 m_nVotesFor;` (`:517`), `int32 m_nVotesAgainst;` (`:518`) and `int32 m_nReports;` (`:519`),
/// and the ranking as `float m_fScore;` (`:520`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PublishedItemVoteDetails {
    pub published_file_id: PublishedFileId,
    pub votes_up: u32,
    pub votes_down: u32,
    pub reports: u32,
    /// Steam's "float m_fScore;" (`isteamremotestorage.h:520`), passed through as written: the
    /// header says nothing about its range, so this wrapper neither rescales nor clamps it.
    pub score: f32,
}

/// The action a caller asks Steam to record, and the one Steam says it recorded.
///
/// `enum EWorkshopFileAction` (`isteamremotestorage.h:120-124`) holds exactly two named codes --
/// "k_EWorkshopFileActionPlayed = 0," (`:122`) and "k_EWorkshopFileActionCompleted = 1," (`:123`) --
/// and declares no out-of-range member, so a code outside those two has no name to send back.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WorkshopFileAction {
    /// "k_EWorkshopFileActionPlayed = 0," (`isteamremotestorage.h:122`).
    Played,
    /// "k_EWorkshopFileActionCompleted = 1," (`:123`).
    Completed,
    /// A code this two-value table does not name, kept as the number Steam wrote.
    ///
    /// Read-only, like the other `Unrecognized` shapes in this crate: sending it back refuses with
    /// [`SteamError::InvalidParameter`] because the vendored enum offers no legal value to build --
    /// measured against `steamworks-sys/src/windows_bindings.rs`, the enum's variants are exactly 0
    /// and 1.
    Unrecognized(i32),
}

/// Which action the number in Steam's field names.
pub(super) fn action_from_code(raw: c_int) -> WorkshopFileAction {
    match raw {
        code if code == sys::EWorkshopFileAction::k_EWorkshopFileActionPlayed as c_int => {
            WorkshopFileAction::Played
        }
        code if code == sys::EWorkshopFileAction::k_EWorkshopFileActionCompleted as c_int => {
            WorkshopFileAction::Completed
        }
        code => WorkshopFileAction::Unrecognized(code),
    }
}

/// Writes an action to Steam; the unnameable reading is refused here, not sent as a guess.
pub(super) fn action_to_sys(
    action: WorkshopFileAction,
) -> Result<sys::EWorkshopFileAction, SteamError> {
    Ok(match action {
        WorkshopFileAction::Played => sys::EWorkshopFileAction::k_EWorkshopFileActionPlayed,
        WorkshopFileAction::Completed => sys::EWorkshopFileAction::k_EWorkshopFileActionCompleted,
        WorkshopFileAction::Unrecognized(_) => return Err(SteamError::InvalidParameter),
    })
}

/// One of the three vote counters, from lines Steam declares as `int32 m_nVotesFor;`,
/// "int32 m_nVotesAgainst;" and "int32 m_nReports;" (`isteamremotestorage.h:517-519`).
///
/// Those three lines carry no comment at all, so nothing in the header promises a non-negative
/// answer -- and a count is exactly the place where a negative becomes a huge number if it is cast
/// rather than clamped. This is this wrapper's judgement, written down rather than hidden: a number
/// that cannot be a count reads as zero votes, which is what an item with no votes already looks
/// like, and never as `u32::MAX`.
fn vote_count(raw: c_int) -> u32 {
    u32::try_from(raw).unwrap_or(0)
}

/// The vote answer, built from the fields as Steam wrote them.
fn vote_details(
    id: u64,
    up: c_int,
    down: c_int,
    reports: c_int,
    score: f32,
) -> PublishedItemVoteDetails {
    PublishedItemVoteDetails {
        published_file_id: PublishedFileId(id),
        votes_up: vote_count(up),
        votes_down: vote_count(down),
        reports: vote_count(reports),
        score,
    }
}

/// Reads Steam's status and the item it acted on out of one of the four id-answering result structs.
///
/// Written four times because the structs are four types, each holding "EResult m_eResult;" and
/// "PublishedFileId_t m_nPublishedFileId;" (`isteamremotestorage.h:378-379`, `:402-403`, `:434-435`,
/// `:562-563`); a helper that claimed all four as one type would assert that Steam keeps them in step.
fn item_and_status(
    v: &sys::RemoteStorageDeletePublishedFileResult_t,
) -> (sys::EResult, PublishedFileId) {
    (v.m_eResult, PublishedFileId(v.m_nPublishedFileId))
}

fn item_and_status_subscribe(
    v: &sys::RemoteStorageSubscribePublishedFileResult_t,
) -> (sys::EResult, PublishedFileId) {
    (v.m_eResult, PublishedFileId(v.m_nPublishedFileId))
}

fn item_and_status_unsubscribe(
    v: &sys::RemoteStorageUnsubscribePublishedFileResult_t,
) -> (sys::EResult, PublishedFileId) {
    (v.m_eResult, PublishedFileId(v.m_nPublishedFileId))
}

fn item_and_status_vote(
    v: &sys::RemoteStorageUpdateUserPublishedItemVoteResult_t,
) -> (sys::EResult, PublishedFileId) {
    (v.m_eResult, PublishedFileId(v.m_nPublishedFileId))
}

impl RemoteStorage {
    /// Delete one published file, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t DeletePublishedFile( PublishedFileId_t unPublishedFileId ) = 0;"
    /// (`isteamremotestorage.h:282`) annotated with `RemoteStorageDeletePublishedFileResult_t`
    /// (`:281`), whose whole payload is "The result of the operation." (`:378`) plus
    /// `PublishedFileId_t m_nPublishedFileId;` (`:379`) -- the item Steam acted on.
    pub fn delete_published_file<F>(&self, published_file_id: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<PublishedFileId, SteamError>) + 'static + Send,
    {
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_DeletePublishedFile(self.rs, published_file_id.0)
        };
        self.answer_item_id(api_call, item_and_status, cb)
    }

    /// Subscribe to one published file, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t SubscribePublishedFile( PublishedFileId_t unPublishedFileId ) = 0;"
    /// (`isteamremotestorage.h:287`) with `RemoteStorageSubscribePublishedFileResult_t` (`:286`),
    /// which answers the same two fields as the delete result (`:402-403`).
    pub fn subscribe_published_file<F>(&self, published_file_id: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<PublishedFileId, SteamError>) + 'static + Send,
    {
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_SubscribePublishedFile(self.rs, published_file_id.0)
        };
        self.answer_item_id(api_call, item_and_status_subscribe, cb)
    }

    /// Unsubscribe from one published file, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t UnsubscribePublishedFile( PublishedFileId_t unPublishedFileId ) = 0;"
    /// (`isteamremotestorage.h:291`) with `RemoteStorageUnsubscribePublishedFileResult_t` (`:290`),
    /// again just the result and the item (`:434-435`).
    pub fn unsubscribe_published_file<F>(&self, published_file_id: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<PublishedFileId, SteamError>) + 'static + Send,
    {
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_UnsubscribePublishedFile(self.rs, published_file_id.0)
        };
        self.answer_item_id(api_call, item_and_status_unsubscribe, cb)
    }

    /// Record that this user voted on an item, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t UpdateUserPublishedItemVote( PublishedFileId_t unPublishedFileId,
    /// bool bVoteUp ) = 0;" (`isteamremotestorage.h:296`) with `:295`; the answer carries the item id
    /// and nothing about the vote (`:562-563`), so `cb` gets the item, not a restatement of `vote_up`.
    pub fn update_user_published_item_vote<F>(
        &self,
        published_file_id: PublishedFileId,
        vote_up: bool,
        cb: F,
    ) where
        F: FnOnce(Result<PublishedFileId, SteamError>) + 'static + Send,
    {
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_UpdateUserPublishedItemVote(
                self.rs,
                published_file_id.0,
                vote_up,
            )
        };
        self.answer_item_id(api_call, item_and_status_vote, cb)
    }

    /// Record what this user did with an item, answering with the action Steam says it recorded.
    ///
    /// "virtual SteamAPICall_t SetUserPublishedFileAction( PublishedFileId_t unPublishedFileId,
    /// EWorkshopFileAction eAction ) = 0;" (`isteamremotestorage.h:304`); its result carries
    /// "the action that was attempted" (`:592`) alongside the item (`:591`), so the answer is the
    /// echo rather than a copy of the argument: Steam's `m_eAction` is what got recorded, and a code
    /// outside the two named ones reads back as [`WorkshopFileAction::Unrecognized`] instead of
    /// panicking or being folded into `Played`.
    pub fn set_user_published_file_action<F>(
        &self,
        published_file_id: PublishedFileId,
        action: WorkshopFileAction,
        cb: F,
    ) where
        F: FnOnce(Result<(PublishedFileId, WorkshopFileAction), SteamError>) + 'static + Send,
    {
        let action = match action_to_sys(action) {
            Ok(action) => action,
            Err(error) => return cb(Err(error)),
        };
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_SetUserPublishedFileAction(
                self.rs,
                published_file_id.0,
                action,
            )
        };
        unsafe {
            register_call_result::<sys::RemoteStorageSetUserPublishedFileActionResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    if io_error {
                        return cb(Err(SteamError::IOFailure));
                    }
                    cb(to_steam_result(v.m_eResult).map(|()| {
                        (
                            PublishedFileId(v.m_nPublishedFileId),
                            action_from_code(v.m_eAction as c_int),
                        )
                    }))
                },
            );
        }
    }

    /// The vote tally on an item, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t GetPublishedItemVoteDetails( PublishedFileId_t unPublishedFileId ) = 0;"
    /// (`isteamremotestorage.h:294`), annotated `RemoteStorageGetPublishedItemVoteDetailsResult_t`
    /// (`:293`).
    pub fn published_item_vote_details<F>(&self, published_file_id: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<PublishedItemVoteDetails, SteamError>) + 'static + Send,
    {
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_GetPublishedItemVoteDetails(
                self.rs,
                published_file_id.0,
            )
        };
        self.answer_vote_details(api_call, cb)
    }

    /// The same tally structure Steam uses to answer what this user voted, through `cb`.
    ///
    /// "virtual SteamAPICall_t GetUserPublishedItemVoteDetails( PublishedFileId_t unPublishedFileId ) = 0;"
    /// (`isteamremotestorage.h:298`) is annotated with the *same* struct as its neighbour
    /// (`:297`, `RemoteStorageGetPublishedItemVoteDetailsResult_t`), and that struct has no field for
    /// the caller's own vote (`:512-520`) -- so this method's answer is the aggregate, and nothing here
    /// can report which way this user voted. That gap is Steam's annotation, not a wrapper shortcut,
    /// and it is written here so a caller does not read the name as more than the answer can be.
    pub fn user_published_item_vote_details<F>(&self, published_file_id: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<PublishedItemVoteDetails, SteamError>) + 'static + Send,
    {
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_GetUserPublishedItemVoteDetails(
                self.rs,
                published_file_id.0,
            )
        };
        self.answer_vote_details(api_call, cb)
    }

    /// The one shape the four item-naming answers share: Steam's status, then the item it acted on.
    ///
    /// The reader arrives as a plain `fn` pointer because the four result structs are distinct types
    /// with the same two fields, spelled as "EResult m_eResult;" and
    /// "PublishedFileId_t m_nPublishedFileId;" (`isteamremotestorage.h:378-379`, `:402-403`,
    /// `:434-435`, `:562-563`); naming the pair keeps the FFI read in one place per struct.
    fn answer_item_id<T: Copy + 'static, F>(
        &self,
        api_call: sys::SteamAPICall_t,
        item: fn(&T) -> (sys::EResult, PublishedFileId),
        cb: F,
    ) where
        F: FnOnce(Result<PublishedFileId, SteamError>) + 'static + Send,
    {
        unsafe {
            register_call_result::<T, _>(&self.inner, api_call, move |v, io_error| {
                if io_error {
                    return cb(Err(SteamError::IOFailure));
                }
                let (status, published_file_id) = item(v);
                cb(to_steam_result(status).map(|()| published_file_id))
            });
        }
    }

    /// The one shape both vote-detail calls answer with.
    fn answer_vote_details<F>(&self, api_call: sys::SteamAPICall_t, cb: F)
    where
        F: FnOnce(Result<PublishedItemVoteDetails, SteamError>) + 'static + Send,
    {
        unsafe {
            register_call_result::<sys::RemoteStorageGetPublishedItemVoteDetailsResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    if io_error {
                        return cb(Err(SteamError::IOFailure));
                    }
                    cb(to_steam_result(v.m_eResult).map(|()| {
                        vote_details(
                            v.m_unPublishedFileId,
                            v.m_nVotesFor,
                            v.m_nVotesAgainst,
                            v.m_nReports,
                            v.m_fScore,
                        )
                    }))
                },
            );
        }
    }
}

#[cfg(test)]
mod workshop_answers_tests {
    //! The action table in both directions, the three vote counters, and the aggregate answer's
    //! field names. Which item Steam actually deletes, subscribes or votes on needs a live client and
    //! is registered as unverified -- as is what `m_eAction` holds for an action Steam rejected.

    use super::{action_from_code, action_to_sys, vote_count, vote_details, WorkshopFileAction};
    use crate::{PublishedFileId, SteamError};
    use std::os::raw::c_int;
    use steamworks_sys as sys;

    /// Steam's two named codes round-trip, and a third code is kept instead of being guessed at.
    #[test]
    fn the_two_named_actions_round_trip_and_a_third_code_stays_a_third_code() {
        assert_eq!(
            action_from_code(sys::EWorkshopFileAction::k_EWorkshopFileActionPlayed as c_int),
            WorkshopFileAction::Played
        );
        assert_eq!(
            action_from_code(sys::EWorkshopFileAction::k_EWorkshopFileActionCompleted as c_int),
            WorkshopFileAction::Completed
        );
        assert_eq!(action_from_code(2), WorkshopFileAction::Unrecognized(2));
        assert_eq!(action_from_code(-1), WorkshopFileAction::Unrecognized(-1));
    }

    /// A code this enum cannot name is refused on the way out, never sent as `Played`.
    #[test]
    fn an_unnameable_action_is_refused_rather_than_substituted() {
        assert_eq!(
            action_to_sys(WorkshopFileAction::Played),
            Ok(sys::EWorkshopFileAction::k_EWorkshopFileActionPlayed)
        );
        assert_eq!(
            action_to_sys(WorkshopFileAction::Completed),
            Ok(sys::EWorkshopFileAction::k_EWorkshopFileActionCompleted)
        );
        assert_eq!(
            action_to_sys(WorkshopFileAction::Unrecognized(7)),
            Err(SteamError::InvalidParameter),
            "substituting a named action would record something the caller never asked for"
        );
    }

    /// A count that cannot be a count reads as no counts, never as a wrapped huge number.
    #[test]
    fn vote_counters_never_wrap_into_huge_numbers() {
        assert_eq!(vote_count(0), 0);
        assert_eq!(vote_count(17), 17);
        assert_eq!(vote_count(-1), 0);
        assert_eq!(vote_count(c_int::MIN), 0);
        assert_ne!(vote_count(-1), u32::MAX);
    }

    /// The answer keeps Steam's own field identities: the item id arrives as
    /// "PublishedFileId_t m_unPublishedFileId;" (`isteamremotestorage.h:516`), the three counters are
    /// the clamped ones, and the score is written through unchanged.
    #[test]
    fn the_answer_carries_the_item_steam_named_and_the_score_it_wrote() {
        let got = vote_details(1001, 30, -1, 4, 0.25);
        assert_eq!(
            got.published_file_id,
            PublishedFileId(1001),
            "the item is Steam's, echoed back -- not the argument this wrapper still holds"
        );
        assert_eq!(got.votes_up, 30);
        assert_eq!(
            got.votes_down, 0,
            "a negative counter never reaches a caller as a huge number"
        );
        assert_eq!(got.reports, 4);
        assert_eq!(got.score, 0.25);
    }
}
