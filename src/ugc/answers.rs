//! The asynchronous answers of the Workshop consumer API: votes, favourites,
//! dependencies and the Workshop EULA.
//!
//! Each of these is a request Steam completes later, so the wrappers hand the answer to a
//! `FnOnce` registered against that call instead of pretending to know it now. Every answer
//! here is one callback for one call -- `register_call_result` fires once per
//! `SteamAPICall_t`, which is what makes `Result<P, SteamError>` the honest shape.
//!
//! `ISteamUGC::GetAppDependencies` is deliberately absent as a one-shot wrapper. Its header
//! note says the callback "may be called multiple times until all app dependencies have been
//! returned" (`isteamugc.h:380-381`), and handing one answer to a `FnOnce` would report a
//! partial list as the whole answer. Its pages are delivered as
//! [`GetAppDependenciesResult`], which the callback stream carries.

use super::*;

/// Turns Steam's one-shot answer into the shape every wrapper here uses: an I/O failure is
/// `IOFailure`, a refused call is Steam's own error, and anything else is the payload.
fn answered<P>(
    io_error: bool,
    result: sys::EResult,
    payload: impl FnOnce() -> P,
) -> Result<P, SteamError> {
    if io_error {
        Err(SteamError::IOFailure)
    } else {
        to_steam_result(result).map(|_| payload())
    }
}

/// The item-to-item dependency answer (`ISteamUGC::AddDependency`).
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct AddUGCDependencyResult {
    pub error: Option<SteamError>,
    pub published_file_id: PublishedFileId,
    pub child_published_file_id: PublishedFileId,
}

impl AddUGCDependencyResult {
    fn from_answer(answer: &sys::AddUGCDependencyResult_t) -> Self {
        Self {
            error: to_steam_result(answer.m_eResult).err(),
            published_file_id: PublishedFileId(answer.m_nPublishedFileId),
            child_published_file_id: PublishedFileId(answer.m_nChildPublishedFileId),
        }
    }
}

impl_callback!(cb: AddUGCDependencyResult_t => AddUGCDependencyResult {
    Self::from_answer(&cb)
});

/// The item-to-item dependency removal answer (`ISteamUGC::RemoveDependency`).
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct RemoveUGCDependencyResult {
    pub error: Option<SteamError>,
    pub published_file_id: PublishedFileId,
    pub child_published_file_id: PublishedFileId,
}

impl RemoveUGCDependencyResult {
    fn from_answer(answer: &sys::RemoveUGCDependencyResult_t) -> Self {
        Self {
            error: to_steam_result(answer.m_eResult).err(),
            published_file_id: PublishedFileId(answer.m_nPublishedFileId),
            child_published_file_id: PublishedFileId(answer.m_nChildPublishedFileId),
        }
    }
}

impl_callback!(cb: RemoveUGCDependencyResult_t => RemoveUGCDependencyResult {
    Self::from_answer(&cb)
});

/// The app-dependency (usually DLC) answer for `ISteamUGC::AddAppDependency`.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct AddAppDependencyResult {
    pub error: Option<SteamError>,
    pub published_file_id: PublishedFileId,
    pub app_id: AppId,
}

impl AddAppDependencyResult {
    fn from_answer(answer: &sys::AddAppDependencyResult_t) -> Self {
        Self {
            error: to_steam_result(answer.m_eResult).err(),
            published_file_id: PublishedFileId(answer.m_nPublishedFileId),
            app_id: AppId(answer.m_nAppID),
        }
    }
}

impl_callback!(cb: AddAppDependencyResult_t => AddAppDependencyResult {
    Self::from_answer(&cb)
});

/// The app-dependency removal answer for `ISteamUGC::RemoveAppDependency`.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct RemoveAppDependencyResult {
    pub error: Option<SteamError>,
    pub published_file_id: PublishedFileId,
    pub app_id: AppId,
}

impl RemoveAppDependencyResult {
    fn from_answer(answer: &sys::RemoveAppDependencyResult_t) -> Self {
        Self {
            error: to_steam_result(answer.m_eResult).err(),
            published_file_id: PublishedFileId(answer.m_nPublishedFileId),
            app_id: AppId(answer.m_nAppID),
        }
    }
}

impl_callback!(cb: RemoveAppDependencyResult_t => RemoveAppDependencyResult {
    Self::from_answer(&cb)
});

/// Which side of the favourites list Steam moved an item to, and whether it accepted.
///
/// `was_add_request` is Steam's echo of the call that produced the answer, so a caller that
/// registered both requests can tell from the payload which one fired.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct UserFavoriteItemsListChanged {
    pub error: Option<SteamError>,
    pub published_file_id: PublishedFileId,
    pub was_add_request: bool,
}

impl UserFavoriteItemsListChanged {
    fn from_answer(answer: &sys::UserFavoriteItemsListChanged_t) -> Self {
        Self {
            error: to_steam_result(answer.m_eResult).err(),
            published_file_id: PublishedFileId(answer.m_nPublishedFileId),
            was_add_request: answer.m_bWasAddRequest,
        }
    }
}

impl_callback!(cb: UserFavoriteItemsListChanged_t => UserFavoriteItemsListChanged {
    Self::from_answer(&cb)
});

/// The vote Steam recorded for this user on one item.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct SetUserItemVoteResult {
    pub error: Option<SteamError>,
    pub published_file_id: PublishedFileId,
    pub vote_up: bool,
}

impl SetUserItemVoteResult {
    fn from_answer(answer: &sys::SetUserItemVoteResult_t) -> Self {
        Self {
            error: to_steam_result(answer.m_eResult).err(),
            published_file_id: PublishedFileId(answer.m_nPublishedFileId),
            vote_up: answer.m_bVoteUp,
        }
    }
}

impl_callback!(cb: SetUserItemVoteResult_t => SetUserItemVoteResult {
    Self::from_answer(&cb)
});

/// The three vote states Steam reports for this user on one item.
///
/// They stay separate rather than collapsing into one value because Steam can answer with
/// `vote_skipped` set, which is neither an up vote nor a down vote.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct GetUserItemVoteResult {
    pub error: Option<SteamError>,
    pub published_file_id: PublishedFileId,
    pub voted_up: bool,
    pub voted_down: bool,
    pub vote_skipped: bool,
}

impl GetUserItemVoteResult {
    fn from_answer(answer: &sys::GetUserItemVoteResult_t) -> Self {
        Self {
            error: to_steam_result(answer.m_eResult).err(),
            published_file_id: PublishedFileId(answer.m_nPublishedFileId),
            voted_up: answer.m_bVotedUp,
            voted_down: answer.m_bVotedDown,
            vote_skipped: answer.m_bVoteSkipped,
        }
    }
}

impl_callback!(cb: GetUserItemVoteResult_t => GetUserItemVoteResult {
    Self::from_answer(&cb)
});

/// The state of the app's Workshop EULA for the current user.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct WorkshopEULAStatus {
    pub error: Option<SteamError>,
    pub app_id: AppId,
    pub version: u32,
    /// When the user accepted or declined, in the Unix seconds Steam reports (`RTime32`).
    /// Steam gives 0 for "no action recorded"; this wrapper does not turn that into a time.
    pub action_time: u32,
    pub accepted: bool,
    pub needs_action: bool,
}

impl WorkshopEULAStatus {
    fn from_answer(answer: &sys::WorkshopEULAStatus_t) -> Self {
        Self {
            error: to_steam_result(answer.m_eResult).err(),
            app_id: AppId(answer.m_nAppID),
            version: answer.m_unVersion,
            action_time: answer.m_rtAction,
            accepted: answer.m_bAccepted,
            needs_action: answer.m_bNeedsAction,
        }
    }
}

impl_callback!(cb: WorkshopEULAStatus_t => WorkshopEULAStatus {
    Self::from_answer(&cb)
});

/// One page of an item's app-dependency list.
///
/// Steam answers `GetAppDependencies` **more than once** when the list does not fit in one
/// callback: `isteamugc.h:596-598` says "the result of a call to GetAppDependencies. Callback
/// may be called multiple times until all app dependencies have been returned", and the struct
/// carries both `m_nNumAppDependencies` ("number returned in this struct") and
/// `m_nTotalNumAppDependencies` ("total found"). Accumulate pages until they add up.
///
/// That is why `GetAppDependencies` itself is not wrapped as a one-shot closure here -- the
/// `FnOnce(Result<P, SteamError>)` shape used by every other method in this file would hand over
/// the first page as though it were the whole list. Register this type with
/// `Client::register_callback`, or read it off the callback stream, to see every page.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct GetAppDependenciesResult {
    pub error: Option<SteamError>,
    pub published_file_id: PublishedFileId,
    /// The dependencies on this page, at most the 32 Steam's struct can hold.
    pub app_ids: Vec<AppId>,
    /// How many entries this page actually carries -- Steam's own count, clamped to what the
    /// fixed array could hold, so an over-reported number cannot read past it.
    pub dependencies_returned: u32,
    pub total_dependencies: u32,
}

impl GetAppDependenciesResult {
    fn from_answer(answer: &sys::GetAppDependenciesResult_t) -> Self {
        let reported = answer.m_nNumAppDependencies as usize;
        let taken = clamped_entries(reported, answer.m_rgAppIDs.len());
        Self {
            error: to_steam_result(answer.m_eResult).err(),
            published_file_id: PublishedFileId(answer.m_nPublishedFileId),
            app_ids: answer.m_rgAppIDs[..taken]
                .iter()
                .copied()
                .map(AppId)
                .collect(),
            dependencies_returned: taken as u32,
            total_dependencies: answer.m_nTotalNumAppDependencies,
        }
    }
}

impl_callback!(cb: GetAppDependenciesResult_t => GetAppDependenciesResult {
    Self::from_answer(&cb)
});

impl UGC {
    /// Makes one workshop item depend on another (a parent/child pair on the Workshop).
    pub fn add_dependency<F>(&self, parent: PublishedFileId, child: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_AddDependency(self.ugc, parent.0, child.0);
            register_call_result::<sys::AddUGCDependencyResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| cb(answered(io_error, v.m_eResult, || ())),
            );
        }
    }

    /// Removes a parent/child dependency between two workshop items.
    pub fn remove_dependency<F>(&self, parent: PublishedFileId, child: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_RemoveDependency(self.ugc, parent.0, child.0);
            register_call_result::<sys::RemoveUGCDependencyResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| cb(answered(io_error, v.m_eResult, || ())),
            );
        }
    }

    /// Requires another app (usually DLC) before a workshop item can be used.
    pub fn add_app_dependency<F>(&self, item: PublishedFileId, app: AppId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_AddAppDependency(self.ugc, item.0, app.0);
            register_call_result::<sys::AddAppDependencyResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| cb(answered(io_error, v.m_eResult, || ())),
            );
        }
    }

    /// Removes an app requirement from a workshop item.
    pub fn remove_app_dependency<F>(&self, item: PublishedFileId, app: AppId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_RemoveAppDependency(self.ugc, item.0, app.0);
            register_call_result::<sys::RemoveAppDependencyResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| cb(answered(io_error, v.m_eResult, || ())),
            );
        }
    }

    /// Adds an item to the current user's favourites.
    pub fn add_item_to_favorites<F>(&self, app: AppId, item: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_AddItemToFavorites(self.ugc, app.0, item.0);
            register_call_result::<sys::UserFavoriteItemsListChanged_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| cb(answered(io_error, v.m_eResult, || ())),
            );
        }
    }

    /// Removes an item from the current user's favourites.
    pub fn remove_item_from_favorites<F>(&self, app: AppId, item: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_RemoveItemFromFavorites(self.ugc, app.0, item.0);
            register_call_result::<sys::UserFavoriteItemsListChanged_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| cb(answered(io_error, v.m_eResult, || ())),
            );
        }
    }

    /// Votes an item up or down and receives the direction Steam recorded.
    ///
    /// The `bool` is Steam's echo, which can disagree with the request when the user already
    /// held the opposite vote.
    pub fn set_user_item_vote<F>(&self, item: PublishedFileId, vote_up: bool, cb: F)
    where
        F: FnOnce(Result<bool, SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_SetUserItemVote(self.ugc, item.0, vote_up);
            register_call_result::<sys::SetUserItemVoteResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| cb(answered(io_error, v.m_eResult, || v.m_bVoteUp)),
            );
        }
    }

    /// Reads this user's vote on an item, in all three states Steam reports.
    pub fn get_user_item_vote<F>(&self, item: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<GetUserItemVoteResult, SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_GetUserItemVote(self.ugc, item.0);
            register_call_result::<sys::GetUserItemVoteResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(answered(io_error, v.m_eResult, || {
                        GetUserItemVoteResult::from_answer(v)
                    }))
                },
            );
        }
    }

    /// Reads the current user's acceptance of the app's Workshop EULA.
    ///
    /// This is the answer [`UGC::show_workshop_eula`] points at: showing the window is only
    /// Steam's acceptance of the *request*, and the user's decision arrives here.
    pub fn get_workshop_eula_status<F>(&self, cb: F)
    where
        F: FnOnce(Result<WorkshopEULAStatus, SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_GetWorkshopEULAStatus(self.ugc);
            register_call_result::<sys::WorkshopEULAStatus_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(answered(io_error, v.m_eResult, || {
                        WorkshopEULAStatus::from_answer(v)
                    }))
                },
            );
        }
    }
}

#[cfg(test)]
mod answer_tests {
    use super::*;
    use crate::callback::{Callback, CallbackResult};

    fn fail() -> sys::EResult {
        sys::EResult::k_EResultFail
    }

    #[test]
    fn answered_prefers_the_io_failure_over_steam_s_result() {
        // Steam can fail to deliver the callback at all; that is not the same as the item
        // being refused, and the caller must not see `Ok` for either.
        assert!(matches!(
            answered(true, sys::EResult::k_EResultOK, || 7),
            Err(SteamError::IOFailure)
        ));
        assert!(matches!(
            answered(false, fail(), || 7),
            Err(SteamError::Generic)
        ));
        assert_eq!(answered(false, sys::EResult::k_EResultOK, || 7), Ok(7));
    }

    #[test]
    fn dependency_answers_decode_both_ids_and_their_own_callback_numbers() {
        let mut add = sys::AddUGCDependencyResult_t {
            m_eResult: sys::EResult::k_EResultOK,
            m_nPublishedFileId: 11,
            m_nChildPublishedFileId: 12,
        };
        let raw = (&mut add as *mut sys::AddUGCDependencyResult_t).cast::<std::os::raw::c_void>();
        match unsafe { CallbackResult::from_raw(AddUGCDependencyResult::ID, raw) } {
            Some(CallbackResult::AddUGCDependencyResult(answer)) => {
                assert_eq!(answer.published_file_id, PublishedFileId(11));
                assert_eq!(answer.child_published_file_id, PublishedFileId(12));
                assert_eq!(answer.error, None);
            }
            other => panic!("AddUGCDependencyResult decoded as {other:?}"),
        }

        let mut remove = sys::RemoveAppDependencyResult_t {
            m_eResult: fail(),
            m_nPublishedFileId: 21,
            m_nAppID: 22,
        };
        let raw =
            (&mut remove as *mut sys::RemoveAppDependencyResult_t).cast::<std::os::raw::c_void>();
        match unsafe { CallbackResult::from_raw(RemoveAppDependencyResult::ID, raw) } {
            Some(CallbackResult::RemoveAppDependencyResult(answer)) => {
                assert_eq!(answer.app_id, AppId(22));
                assert_eq!(answer.error, Some(SteamError::Generic));
            }
            other => panic!("RemoveAppDependencyResult decoded as {other:?}"),
        }
    }

    #[test]
    fn a_skipped_vote_is_neither_up_nor_down() {
        let mut skipped = sys::GetUserItemVoteResult_t {
            m_nPublishedFileId: 31,
            m_eResult: sys::EResult::k_EResultOK,
            m_bVotedUp: false,
            m_bVotedDown: false,
            m_bVoteSkipped: true,
        };
        let answer = GetUserItemVoteResult::from_answer(&skipped);
        assert!(answer.vote_skipped);
        assert!(!answer.voted_up);
        assert!(!answer.voted_down);
        // Each output must come from its own bit. Copying one source field into two
        // outputs -- the shape this mapping could easily take -- is what these three
        // asserts are for.
        skipped.m_bVotedUp = true;
        skipped.m_bVotedDown = false;
        let answer = GetUserItemVoteResult::from_answer(&skipped);
        assert!(answer.voted_up);
        assert!(!answer.voted_down);
        assert!(answer.vote_skipped);

        skipped.m_bVotedUp = false;
        skipped.m_bVotedDown = true;
        skipped.m_bVoteSkipped = false;
        let answer = GetUserItemVoteResult::from_answer(&skipped);
        assert!(!answer.voted_up);
        assert!(answer.voted_down);
        assert!(!answer.vote_skipped);
    }

    #[test]
    fn eula_status_keeps_version_and_action_time_apart() {
        let mut raw = sys::WorkshopEULAStatus_t {
            m_eResult: sys::EResult::k_EResultOK,
            m_nAppID: 40,
            m_unVersion: 3,
            m_rtAction: 1_700_000_000,
            m_bAccepted: true,
            m_bNeedsAction: false,
        };
        let status = WorkshopEULAStatus::from_answer(&raw);
        assert_eq!(status.app_id, AppId(40));
        assert_eq!(status.version, 3);
        assert_eq!(status.action_time, 1_700_000_000);
        assert!(status.accepted);
        assert!(!status.needs_action);

        // The interesting shape: asked to act, and nothing recorded yet.
        raw.m_bAccepted = false;
        raw.m_bNeedsAction = true;
        raw.m_rtAction = 0;
        let status = WorkshopEULAStatus::from_answer(&raw);
        assert!(status.needs_action);
        assert!(!status.accepted);
        assert_eq!(status.action_time, 0);
    }

    #[test]
    fn favourites_answer_reports_which_request_fired() {
        let mut add_request = sys::UserFavoriteItemsListChanged_t {
            m_nPublishedFileId: 51,
            m_eResult: sys::EResult::k_EResultOK,
            m_bWasAddRequest: true,
        };
        let answer = UserFavoriteItemsListChanged::from_answer(&add_request);
        assert!(answer.was_add_request);
        assert_eq!(answer.published_file_id, PublishedFileId(51));
        add_request.m_bWasAddRequest = false;
        assert!(!UserFavoriteItemsListChanged::from_answer(&add_request).was_add_request);
    }

    #[test]
    fn a_dependency_page_never_reads_past_the_array_steam_filled() {
        let mut page = sys::GetAppDependenciesResult_t {
            m_eResult: sys::EResult::k_EResultOK,
            m_nPublishedFileId: 61,
            m_rgAppIDs: [0; 32],
            m_nNumAppDependencies: 2,
            m_nTotalNumAppDependencies: 40,
        };
        page.m_rgAppIDs[0] = 7;
        page.m_rgAppIDs[1] = 8;
        let answer = GetAppDependenciesResult::from_answer(&page);
        assert_eq!(answer.app_ids, vec![AppId(7), AppId(8)]);
        assert_eq!(answer.dependencies_returned, 2);
        assert_eq!(
            answer.total_dependencies, 40,
            "the page count and the total are different numbers"
        );

        // Steam's own count is not a bound: a report larger than the fixed array has to be
        // clamped, or the slice reads past what the struct holds.
        page.m_nNumAppDependencies = 33;
        let answer = GetAppDependenciesResult::from_answer(&page);
        assert_eq!(answer.app_ids.len(), 32);
        assert_eq!(answer.dependencies_returned, 32);

        // And the page reaches the stream, since that is the only way a caller sees the
        // second and later pages.
        let raw =
            (&mut page as *mut sys::GetAppDependenciesResult_t).cast::<std::os::raw::c_void>();
        match unsafe { CallbackResult::from_raw(GetAppDependenciesResult::ID, raw) } {
            Some(CallbackResult::GetAppDependenciesResult(streamed)) => {
                assert_eq!(streamed.published_file_id, PublishedFileId(61));
                assert_eq!(streamed.app_ids.len(), 32);
            }
            other => panic!("GetAppDependenciesResult decoded as {other:?}"),
        }
    }
}
