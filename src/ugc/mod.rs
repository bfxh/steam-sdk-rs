use super::*;
use crate::app::c_string_arg;

use std::error;
use std::ffi::{CStr, CString};
use std::fmt;
use std::marker;
use std::mem;
use std::os::raw::c_char;
use std::path::Path;

mod answers;
mod query;
mod read;
mod results;
mod types;
#[cfg(test)]
mod ugc_tests;
mod update;

pub use answers::*;
pub use query::*;
pub use read::*;
pub use results::*;
pub use types::*;
pub use update::*;

/// Sentinel Steam answers for a query handle it did not create.
impl UGC {
    /// Suspends or resumes all workshop downloads
    pub fn suspend_downloads(&self, suspend: bool) {
        unsafe {
            sys::SteamAPI_ISteamUGC_SuspendDownloads(self.ugc, suspend);
        }
    }

    /// Creates a workshop item
    pub fn create_item<F>(&self, app_id: AppId, file_type: FileType, cb: F)
    where
        F: FnOnce(Result<(PublishedFileId, bool), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_CreateItem(self.ugc, app_id.0, file_type.into());
            register_call_result::<sys::CreateItemResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        crate::to_steam_result(v.m_eResult).map(|_| {
                            (
                                PublishedFileId(v.m_nPublishedFileId),
                                v.m_bUserNeedsToAcceptWorkshopLegalAgreement,
                            )
                        })
                    })
                },
            );
        }
    }

    /// Starts an item update process
    #[must_use]
    pub fn start_item_update(&self, app_id: AppId, file_id: PublishedFileId) -> UpdateHandle {
        unsafe {
            let handle = sys::SteamAPI_ISteamUGC_StartItemUpdate(self.ugc, app_id.0, file_id.0);
            UpdateHandle {
                ugc: self.ugc,
                inner: self.inner.clone(),

                handle,
            }
        }
    }

    /// Starts an item update process using the panic-free
    /// [`SafeUgcUpdate`] builder.
    ///
    /// Each setter on the returned value validates its input and returns
    /// `Result` instead of panicking on invalid paths or interior nul bytes.
    #[must_use]
    pub fn safe_item_update(&self, app_id: AppId, file_id: PublishedFileId) -> SafeUgcUpdate {
        SafeUgcUpdate::new(self.start_item_update(app_id, file_id))
    }

    /// Subscribes to a workshop item
    pub fn subscribe_item<F>(&self, published_file_id: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_SubscribeItem(self.ugc, published_file_id.0);
            register_call_result::<sys::RemoteStorageSubscribePublishedFileResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        crate::to_steam_result(v.m_eResult)
                    })
                },
            );
        }
    }

    pub fn unsubscribe_item<F>(&self, published_file_id: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_UnsubscribeItem(self.ugc, published_file_id.0);
            register_call_result::<sys::RemoteStorageUnsubscribePublishedFileResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        crate::to_steam_result(v.m_eResult)
                    })
                },
            );
        }
    }

    /// Gets the publisher file IDs of all currently subscribed items.
    ///
    /// Set `include_locally_disabled` to `true` to include items that are
    /// locally disabled.
    pub fn subscribed_items(&self, include_locally_disabled: bool) -> Vec<PublishedFileId> {
        unsafe {
            let count =
                sys::SteamAPI_ISteamUGC_GetNumSubscribedItems(self.ugc, include_locally_disabled);
            let mut data: Vec<sys::PublishedFileId_t> = vec![0; count as usize];
            let gotten_count = sys::SteamAPI_ISteamUGC_GetSubscribedItems(
                self.ugc,
                data.as_mut_ptr(),
                count,
                include_locally_disabled,
            );
            debug_assert!(count == gotten_count);
            data.into_iter().map(|v| PublishedFileId(v)).collect()
        }
    }

    pub fn item_state(&self, item: PublishedFileId) -> ItemState {
        unsafe {
            let state = sys::SteamAPI_ISteamUGC_GetItemState(self.ugc, item.0);
            ItemState::from_bits_truncate(state)
        }
    }

    pub fn item_download_info(&self, item: PublishedFileId) -> Option<(u64, u64)> {
        unsafe {
            let mut current = 0u64;
            let mut total = 0u64;
            if sys::SteamAPI_ISteamUGC_GetItemDownloadInfo(
                self.ugc,
                item.0,
                &mut current,
                &mut total,
            ) {
                Some((current, total))
            } else {
                None
            }
        }
    }

    pub fn item_install_info(&self, item: PublishedFileId) -> Option<InstallInfo> {
        unsafe {
            let mut size_on_disk = 0u64;
            let mut folder = [0 as c_char; 4096];
            let mut timestamp = 0u32;
            if sys::SteamAPI_ISteamUGC_GetItemInstallInfo(
                self.ugc,
                item.0,
                &mut size_on_disk,
                folder.as_mut_ptr(),
                folder.len() as _,
                &mut timestamp,
            ) {
                Some(InstallInfo {
                    folder: CStr::from_ptr(folder.as_ptr())
                        .to_string_lossy()
                        .into_owned(),
                    size_on_disk,
                    timestamp,
                })
            } else {
                None
            }
        }
    }

    pub fn download_item(&self, item: PublishedFileId, high_priority: bool) -> bool {
        unsafe { sys::SteamAPI_ISteamUGC_DownloadItem(self.ugc, item.0, high_priority) }
    }

    /// Queries a paged list of all workshop items.
    pub fn query_all(
        &self,
        query_type: UGCQueryType,
        item_type: UGCType,
        appids: AppIDs,
        page: u32,
    ) -> Result<QueryHandle, CreateQueryError> {
        // Call the external function with the correct parameters
        let handle = unsafe {
            sys::SteamAPI_ISteamUGC_CreateQueryAllUGCRequestPage(
                self.ugc,
                query_type.into(),
                item_type.into(),
                appids.creator_app_id().unwrap_or(AppId(0)).0,
                appids.consumer_app_id().unwrap_or(AppId(0)).0,
                page,
            )
        };

        // Check for an invalid handle
        if handle == UGCQueryHandleInvalid {
            return Err(CreateQueryError);
        }

        // Return a new AllQuery instance
        Ok(QueryHandle {
            ugc: self.ugc,
            inner: Arc::clone(&self.inner),
            handle: Some(handle),
        })
    }

    /// Queries every matching item with Steam's deep paging interface.
    ///
    /// `cursor` continues a previous page; Steam's header says it "should be set
    /// to NULL or `\"*\"` to get the first result set", which is what `None`
    /// sends. The answer carries [`QueryResults::next_cursor`] while more results
    /// remain. A cursor with an interior nul never reaches Steam and is refused as
    /// `CreateQueryError`.
    pub fn query_all_with_cursor(
        &self,
        query_type: UGCQueryType,
        item_type: UGCType,
        appids: AppIDs,
        cursor: Option<&str>,
    ) -> Result<QueryHandle, CreateQueryError> {
        let cursor = match cursor {
            Some(cursor) => Some(CString::new(cursor).map_err(|_| CreateQueryError)?),
            None => None,
        };
        let cursor_ptr = cursor
            .as_ref()
            .map_or(std::ptr::null(), |cursor| cursor.as_ptr());
        let handle = unsafe {
            sys::SteamAPI_ISteamUGC_CreateQueryAllUGCRequestCursor(
                self.ugc,
                query_type.into(),
                item_type.into(),
                appids.creator_app_id().unwrap_or(AppId(0)).0,
                appids.consumer_app_id().unwrap_or(AppId(0)).0,
                cursor_ptr,
            )
        };
        if handle == UGCQueryHandleInvalid {
            return Err(CreateQueryError);
        }
        Ok(QueryHandle {
            ugc: self.ugc,
            inner: Arc::clone(&self.inner),
            handle: Some(handle),
        })
    }

    /// Queries a list of workshop itmes, related to a user in some way (Ex. user's subscriptions, favorites, upvoted, ...)
    pub fn query_user(
        &self,
        account: AccountId,
        list_type: UserList,
        item_type: UGCType,
        sort_order: UserListOrder,
        appids: AppIDs,
        page: u32,
    ) -> Result<QueryHandle, CreateQueryError> {
        let res = unsafe {
            sys::SteamAPI_ISteamUGC_CreateQueryUserUGCRequest(
                self.ugc,
                account.0,
                list_type.into(),
                item_type.into(),
                sort_order.into(),
                appids.creator_app_id().unwrap_or(AppId(0)).0,
                appids.consumer_app_id().unwrap_or(AppId(0)).0,
                page,
            )
        };

        if res == UGCQueryHandleInvalid {
            return Err(CreateQueryError);
        }

        Ok(QueryHandle {
            ugc: self.ugc,
            inner: Arc::clone(&self.inner),
            handle: Some(res),
        })
    }

    pub fn query_items(
        &self,
        mut items: Vec<PublishedFileId>,
    ) -> Result<QueryHandle, CreateQueryError> {
        debug_assert!(items.len() > 0);

        let res = unsafe {
            sys::SteamAPI_ISteamUGC_CreateQueryUGCDetailsRequest(
                self.ugc,
                items.as_mut_ptr() as _,
                items.len() as _,
            )
        };

        if res == UGCQueryHandleInvalid {
            return Err(CreateQueryError);
        }

        Ok(QueryHandle {
            ugc: self.ugc,
            inner: Arc::clone(&self.inner),
            handle: Some(res),
        })
    }

    pub fn query_item(&self, item: PublishedFileId) -> Result<QueryHandle, CreateQueryError> {
        let mut items = vec![item];

        let res = unsafe {
            sys::SteamAPI_ISteamUGC_CreateQueryUGCDetailsRequest(
                self.ugc,
                items.as_mut_ptr() as _,
                1 as _,
            )
        };

        if res == UGCQueryHandleInvalid {
            return Err(CreateQueryError);
        }

        Ok(QueryHandle {
            ugc: self.ugc,
            inner: Arc::clone(&self.inner),
            handle: Some(res),
        })
    }

    /// **DELETES** the item from the Steam Workshop.
    pub fn delete_item<F>(&self, published_file_id: PublishedFileId, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_DeleteItem(self.ugc, published_file_id.0);
            register_call_result::<sys::DeleteItemResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else if let Err(error) = to_steam_result(v.m_eResult) {
                        Err(error)
                    } else {
                        Ok(())
                    })
                },
            );
        }
    }

    /// Start tracking playtime on a set of workshop items.
    pub fn start_playtime_tracking<F>(&self, published_file_ids: &[PublishedFileId], cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_StartPlaytimeTracking(
                self.ugc,
                published_file_ids.as_ptr() as *mut u64,
                published_file_ids.len() as u32,
            );
            register_call_result::<sys::StartPlaytimeTrackingResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else if let Err(error) = to_steam_result(v.m_eResult) {
                        Err(error)
                    } else {
                        Ok(())
                    })
                },
            );
        }
    }

    /// Stop tracking playtime on a set of workshop items.
    pub fn stop_playtime_tracking<F>(&self, published_file_ids: &[PublishedFileId], cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_StopPlaytimeTracking(
                self.ugc,
                published_file_ids.as_ptr() as *mut u64,
                published_file_ids.len() as u32,
            );
            register_call_result::<sys::StopPlaytimeTrackingResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else if let Err(error) = to_steam_result(v.m_eResult) {
                        Err(error)
                    } else {
                        Ok(())
                    })
                },
            );
        }
    }

    /// Stop tracking playtime of all workshop items.
    pub fn stop_playtime_tracking_for_all_items<F>(&self, cb: F)
    where
        F: FnOnce(Result<(), SteamError>) + 'static + Send,
    {
        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_StopPlaytimeTrackingForAllItems(self.ugc);
            register_call_result::<sys::StopPlaytimeTrackingResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else if let Err(error) = to_steam_result(v.m_eResult) {
                        Err(error)
                    } else {
                        Ok(())
                    })
                },
            );
        }
    }
}

impl UGC {
    /// Initialize this UGC interface for a Steam game server.
    ///
    /// You should pass in the Workshop depot, you can find this on SteamDB. It's usually just the app ID.
    ///
    /// The folder is a path to the directory where you wish for this game server to store UGC content.
    ///
    /// `true` upon success; otherwise, `false` if the calling user is not a game server or if the workshop is currently updating its content.
    pub fn init_for_game_server(&self, workshop_depot: sys::DepotId_t, folder: &str) -> bool {
        unsafe {
            let folder = CString::new(folder).unwrap();
            sys::SteamAPI_ISteamUGC_BInitWorkshopForGameServer(
                self.ugc,
                workshop_depot,
                folder.as_ptr(),
            )
        }
    }

    /// Shows the app's latest Workshop EULA in an overlay window, where the user can
    /// accept or decline it.
    ///
    /// The return value is Steam's acceptance of the request to show the window, not
    /// the user's decision. Reading that decision back is the asynchronous
    /// [`UGC::get_workshop_eula_status`].
    pub fn show_workshop_eula(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUGC_ShowWorkshopEULA(self.ugc) }
    }

    /// How many workshop items are actually downloaded on this machine.
    ///
    /// This is not the number of subscriptions: an item can be subscribed and not
    /// downloaded, and [`UGC::subscribed_items`] answers that other question.
    pub fn downloaded_item_count(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamUGC_GetNumDownloadedItems(self.ugc) }
    }

    /// The items Steam has downloaded locally, as it reports them right now.
    ///
    /// The list is sized from [`UGC::downloaded_item_count`] and then truncated to
    /// however many entries Steam actually wrote, so a count that changes between
    /// the two calls cannot read past what was filled.
    pub fn downloaded_items(&self) -> Vec<PublishedFileId> {
        let count = self.downloaded_item_count();
        if count == 0 {
            return Vec::new();
        }
        let mut ids: Vec<sys::PublishedFileId_t> = vec![0; count as usize];
        let written = unsafe {
            sys::SteamAPI_ISteamUGC_GetDownloadedItems(
                self.ugc,
                ids.as_mut_ptr(),
                ids.len() as sys::uint32,
            )
        };
        ids.truncate(clamped_entries(written as usize, ids.len()));
        ids.into_iter()
            .filter(|id| *id != 0)
            .map(PublishedFileId)
            .collect()
    }

    /// Tells Steam to stop keeping this item in its local cache. The item stays
    /// subscribed; Steam just stops trying to keep it on disk, unless another user
    /// on this machine subscribes to it too (its header's own caveat).
    pub fn mark_downloaded_item_as_unused(&self, item: PublishedFileId) -> bool {
        unsafe { sys::SteamAPI_ISteamUGC_MarkDownloadedItemAsUnused(self.ugc, item.0) }
    }

    /// Disables or re-enables items locally. A disabled item is left out of
    /// [`UGC::subscribed_items`] unless that call asks for disabled ones.
    ///
    /// An empty list asks for no change at all, so this answers `false` without
    /// calling Steam: there is nothing there for Steam to have accepted.
    pub fn set_items_disabled_locally(&self, items: &[PublishedFileId], disabled: bool) -> bool {
        if items.is_empty() {
            return false;
        }
        let mut ids: Vec<sys::PublishedFileId_t> = items.iter().map(|id| id.0).collect();
        unsafe {
            sys::SteamAPI_ISteamUGC_SetItemsDisabledLocally(
                self.ugc,
                ids.as_mut_ptr(),
                ids.len() as sys::uint32,
                disabled,
            )
        }
    }

    /// Sets the order items load in locally. Items left out of `items` sort by the
    /// time they were subscribed, which is Steam's stated behaviour rather than
    /// something this wrapper adds.
    pub fn set_subscriptions_load_order(&self, items: &[PublishedFileId]) -> bool {
        if items.is_empty() {
            return false;
        }
        let mut ids: Vec<sys::PublishedFileId_t> = items.iter().map(|id| id.0).collect();
        unsafe {
            sys::SteamAPI_ISteamUGC_SetSubscriptionsLoadOrder(
                self.ugc,
                ids.as_mut_ptr(),
                ids.len() as sys::uint32,
            )
        }
    }

    /// Which kinds of community content the user has chosen to see.
    ///
    /// Steam writes into a fixed array here, so this returns at most eight entries
    /// -- five descriptor codes exist today, and the count Steam reports is trusted
    /// only up to the buffer's length.
    pub fn user_content_descriptor_preferences(&self) -> Vec<UGCContentDescriptorID> {
        let mut descriptors: [sys::EUGCContentDescriptorID; UGC_CONTENT_DESCRIPTORS_MAX] =
            [sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_NudityOrSexualContent;
                UGC_CONTENT_DESCRIPTORS_MAX];
        let written = unsafe {
            sys::SteamAPI_ISteamUGC_GetUserContentDescriptorPreferences(
                self.ugc,
                descriptors.as_mut_ptr(),
                descriptors.len() as sys::uint32,
            )
        };
        descriptors[..clamped_entries(written as usize, descriptors.len())]
            .iter()
            .map(|&descriptor| descriptor.into())
            .collect()
    }
}

/// A handle to update a published item
#[derive(Debug, Clone, Copy)]
pub struct CreateQueryError;
impl fmt::Display for CreateQueryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Could not create workshop query")
    }
}
impl error::Error for CreateQueryError {}
