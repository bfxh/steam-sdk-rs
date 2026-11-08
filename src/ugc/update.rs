//! The item-update builder: a Steam-side handle that accumulates changes until
//! [`UpdateHandle::submit`] sends them, plus the watch handle that reports progress.
//!
//! Split out of `mod.rs` on the same rule as the other pieces: one file per thing the
//! workshop surface lets you *do*, so a new update-side method has an obvious home and
//! the size ceiling on each file stays meaningful.

use super::*;

pub struct UpdateHandle {
    pub(super) ugc: *mut sys::ISteamUGC,
    pub(super) inner: Arc<Inner>,

    pub(super) handle: sys::UGCUpdateHandle_t,
}

/// Steam takes these as C strings, so an embedded NUL is a bad argument. It used to be a panic
/// (`CString::new(..).unwrap()`) inside the caller's process.
pub(super) fn steam_cstr(value: &str) -> Result<CString, SteamError> {
    CString::new(value).map_err(|_| SteamError::InvalidParameter)
}

/// A path Steam must read. A missing one is `FileNotFound`; anything else the OS reports is
/// `IOFailure`, because "I could not stat it" is not the same answer as "it is not there".
pub(super) fn resolved_path(path: &Path) -> Result<std::path::PathBuf, SteamError> {
    path.canonicalize().map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => SteamError::FileNotFound,
        _ => SteamError::IOFailure,
    })
}

/// The `Set*` / `Add*` / `Remove*` calls answer with false. Refusing with Steam's own vocabulary
/// beats `assert!`, which panicked in the caller over something Steam already reported.
pub(super) fn check(ok: bool) -> Result<(), SteamError> {
    if ok {
        Ok(())
    } else {
        Err(SteamError::InvalidParameter)
    }
}
impl UpdateHandle {
    #[must_use]
    /// Sets the item's title.
    ///
    /// A title with an embedded NUL has no Steam code to send and fails with
    /// `InvalidParameter`, as does a title Steam itself refuses.
    pub fn title(self, title: &str) -> Result<Self, SteamError> {
        let title = steam_cstr(title)?;
        check(unsafe {
            sys::SteamAPI_ISteamUGC_SetItemTitle(self.ugc, self.handle, title.as_ptr())
        })?;
        Ok(self)
    }

    #[must_use]
    /// Sets the item's description. Fails on an embedded NUL or a refusal by Steam.
    pub fn description(self, description: &str) -> Result<Self, SteamError> {
        let description = steam_cstr(description)?;
        check(unsafe {
            sys::SteamAPI_ISteamUGC_SetItemDescription(self.ugc, self.handle, description.as_ptr())
        })?;
        Ok(self)
    }

    #[must_use]
    /// Sets the language of this update. Fails on an embedded NUL or a refusal by Steam.
    pub fn language(self, language: &str) -> Result<Self, SteamError> {
        let language = steam_cstr(language)?;
        check(unsafe {
            sys::SteamAPI_ISteamUGC_SetItemUpdateLanguage(self.ugc, self.handle, language.as_ptr())
        })?;
        Ok(self)
    }

    #[must_use]
    /// Sets the preview file. A path that cannot be resolved fails (`FileNotFound` if it is
    /// missing) instead of `canonicalize().unwrap()` panicking in the caller's process.
    pub fn preview_path(self, path: &Path) -> Result<Self, SteamError> {
        let path = resolved_path(path)?;
        let preview_path = steam_cstr(&path.to_string_lossy())?;
        check(unsafe {
            sys::SteamAPI_ISteamUGC_SetItemPreview(self.ugc, self.handle, preview_path.as_ptr())
        })?;
        Ok(self)
    }

    #[must_use]
    /// Sets the folder holding the item's content. Same failure shape as
    /// [`Self::preview_path`]: an unresolvable path is an error, not a panic.
    pub fn content_path(self, path: &Path) -> Result<Self, SteamError> {
        let path = resolved_path(path)?;
        let content_path = steam_cstr(&path.to_string_lossy())?;
        check(unsafe {
            sys::SteamAPI_ISteamUGC_SetItemContent(self.ugc, self.handle, content_path.as_ptr())
        })?;
        Ok(self)
    }

    #[must_use]
    /// Sets the item's metadata string. Fails on an embedded NUL or a refusal by Steam.
    pub fn metadata(self, metadata: &str) -> Result<Self, SteamError> {
        let metadata = steam_cstr(metadata)?;
        check(unsafe {
            sys::SteamAPI_ISteamUGC_SetItemMetadata(self.ugc, self.handle, metadata.as_ptr())
        })?;
        Ok(self)
    }

    /// Sets the item's visibility.
    ///
    /// A visibility Steam was not told about (`Unknown`, read back from a newer client) fails
    /// with `SteamError::InvalidParameter` before the call is made, so the item keeps the
    /// visibility it already has instead of getting a substituted one.
    pub fn visibility(
        self,
        visibility: remote_storage::PublishedFileVisibility,
    ) -> Result<Self, SteamError> {
        let visibility = sys::ERemoteStoragePublishedFileVisibility::try_from(visibility)?;
        let ok =
            unsafe { sys::SteamAPI_ISteamUGC_SetItemVisibility(self.ugc, self.handle, visibility) };
        if !ok {
            return Err(SteamError::InvalidParameter);
        }
        Ok(self)
    }

    /// Sets the item's tag list.
    ///
    /// One unencodable tag refuses the whole call: `SteamParamStringArray::new` gives back
    /// `None` and frees what it had already converted, so a partial tag list is never sent.
    pub fn tags<S: AsRef<str>>(
        self,
        tags: Vec<S>,
        allow_admin_tags: bool,
    ) -> Result<Self, SteamError> {
        let mut array = SteamParamStringArray::new(&tags).ok_or(SteamError::InvalidParameter)?;
        let raw = array.as_raw();
        check(unsafe {
            sys::SteamAPI_ISteamUGC_SetItemTags(self.ugc, self.handle, &raw, allow_admin_tags)
        })?;
        Ok(self)
    }

    /// Adds one key/value tag. Either half may refuse: a NUL in key or value, or Steam.
    pub fn add_key_value_tag(self, key: &str, value: &str) -> Result<Self, SteamError> {
        let key = steam_cstr(key)?;
        let value = steam_cstr(value)?;
        check(unsafe {
            sys::SteamAPI_ISteamUGC_AddItemKeyValueTag(
                self.ugc,
                self.handle,
                key.as_ptr(),
                value.as_ptr(),
            )
        })?;
        Ok(self)
    }

    /// Removes the key/value tags under one key. Fails on an embedded NUL or a refusal by Steam.
    pub fn remove_key_value_tag(self, key: &str) -> Result<Self, SteamError> {
        let key = steam_cstr(key)?;
        check(unsafe {
            sys::SteamAPI_ISteamUGC_RemoveItemKeyValueTags(self.ugc, self.handle, key.as_ptr())
        })?;
        Ok(self)
    }

    /// Adds a content descriptor to the item.
    ///
    /// `Unknown` (a descriptor this binding does not name) fails with
    /// `SteamError::InvalidParameter` before the call rather than being tagged under some other
    /// descriptor's number, and a descriptor Steam refuses fails with the same error after it.
    pub fn add_content_descriptor(
        self,
        desc_id: UGCContentDescriptorID,
    ) -> Result<Self, SteamError> {
        let desc_id = sys::EUGCContentDescriptorID::try_from(desc_id)?;
        let ok =
            unsafe { sys::SteamAPI_ISteamUGC_AddContentDescriptor(self.ugc, self.handle, desc_id) };
        if !ok {
            return Err(SteamError::InvalidParameter);
        }
        Ok(self)
    }

    /// Removes a content descriptor from the item.
    ///
    /// Fails for `Unknown` and for a removal Steam refuses, both as
    /// `SteamError::InvalidParameter` -- the same reason as [`Self::add_content_descriptor`].
    pub fn remove_content_descriptor(
        self,
        desc_id: UGCContentDescriptorID,
    ) -> Result<Self, SteamError> {
        let desc_id = sys::EUGCContentDescriptorID::try_from(desc_id)?;
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_RemoveContentDescriptor(self.ugc, self.handle, desc_id)
        };
        if !ok {
            return Err(SteamError::InvalidParameter);
        }
        Ok(self)
    }

    /// Removes every key/value tag. Steam's refusal is an error rather than a panic.
    pub fn remove_all_key_value_tags(self) -> Result<Self, SteamError> {
        check(unsafe { sys::SteamAPI_ISteamUGC_RemoveAllItemKeyValueTags(self.ugc, self.handle) })?;
        Ok(self)
    }

    /// Adds one of the item's extra previews from a file on disk, with the kind of
    /// preview it is.
    ///
    /// Unlike the older `preview_path`, nothing here panics: a path this machine
    /// cannot resolve is `Err(FileNotFound)`, and asking for
    /// [`PreviewKind::Unknown`] is `Err(InvalidParameter)` -- Steam is not sent a
    /// type nobody named. `false` from Steam becomes `Err(InvalidParameter)` too,
    /// which is what an unusable update handle answers as.
    pub fn add_preview_file(self, path: &Path, kind: PreviewKind) -> Result<Self, SteamError> {
        let kind = preview_kind_to_sys(kind)?;
        let path = preview_file_path(path)?;
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddItemPreviewFile(self.ugc, self.handle, path.as_ptr(), kind)
        };
        ok.then_some(self).ok_or(SteamError::InvalidParameter)
    }

    /// Adds a YouTube video as one of the item's previews.
    pub fn add_preview_video(self, video_id: &str) -> Result<Self, SteamError> {
        let video_id = c_string_arg(video_id)?;
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddItemPreviewVideo(self.ugc, self.handle, video_id.as_ptr())
        };
        ok.then_some(self).ok_or(SteamError::InvalidParameter)
    }

    /// Replaces the preview at `index` with a different file.
    ///
    /// Steam's previews are sorted and indexed from zero, so an index Steam does
    /// not have answers `Err(InvalidParameter)` rather than adding a preview.
    pub fn update_preview_file(self, index: u32, path: &Path) -> Result<Self, SteamError> {
        let path = preview_file_path(path)?;
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_UpdateItemPreviewFile(
                self.ugc,
                self.handle,
                index,
                path.as_ptr(),
            )
        };
        ok.then_some(self).ok_or(SteamError::InvalidParameter)
    }

    /// Replaces the preview video at `index`.
    pub fn update_preview_video(self, index: u32, video_id: &str) -> Result<Self, SteamError> {
        let video_id = c_string_arg(video_id)?;
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_UpdateItemPreviewVideo(
                self.ugc,
                self.handle,
                index,
                video_id.as_ptr(),
            )
        };
        ok.then_some(self).ok_or(SteamError::InvalidParameter)
    }

    /// Removes the preview at `index`.
    pub fn remove_preview(self, index: u32) -> Result<Self, SteamError> {
        let ok = unsafe { sys::SteamAPI_ISteamUGC_RemoveItemPreview(self.ugc, self.handle, index) };
        ok.then_some(self).ok_or(SteamError::InvalidParameter)
    }

    /// Restricts the item to a range of the game's branches (its versions).
    ///
    /// Steam's header is explicit about the empty string here: "an empty string
    /// for either parameter means that it will match any version on that end of
    /// the range", and the restriction only applies once the content has actually
    /// changed.
    pub fn set_required_game_versions(
        self,
        min_branch: &str,
        max_branch: &str,
    ) -> Result<Self, SteamError> {
        let min_branch = c_string_arg(min_branch)?;
        let max_branch = c_string_arg(max_branch)?;
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetRequiredGameVersions(
                self.ugc,
                self.handle,
                min_branch.as_ptr(),
                max_branch.as_ptr(),
            )
        };
        ok.then_some(self).ok_or(SteamError::InvalidParameter)
    }

    /// Marks this request as an admin query, which returns items the workshop has
    /// hidden. Setting the flag is not a permission check: Steam answers nothing
    /// unless the logged-in account actually has the right.
    pub fn set_admin_query(self, admin: bool) -> Result<Self, SteamError> {
        let ok = unsafe { sys::SteamAPI_ISteamUGC_SetAdminQuery(self.ugc, self.handle, admin) };
        ok.then_some(self).ok_or(SteamError::InvalidParameter)
    }

    /// Allows the legacy single-small-file upload path (Steam's header: the content
    /// must be one file under 10 MB). Modern multi-file items leave this off.
    pub fn set_allow_legacy_upload(self, allow: bool) -> Result<Self, SteamError> {
        let ok =
            unsafe { sys::SteamAPI_ISteamUGC_SetAllowLegacyUpload(self.ugc, self.handle, allow) };
        ok.then_some(self).ok_or(SteamError::InvalidParameter)
    }

    pub fn submit<F>(self, change_note: Option<&str>, cb: F) -> UpdateWatchHandle
    where
        F: FnOnce(Result<(PublishedFileId, bool), SteamError>) + 'static + Send,
    {
        use std::ptr;
        unsafe {
            let change_note = change_note.and_then(|v| CString::new(v).ok());
            let note = change_note.as_ref().map_or(ptr::null(), |v| v.as_ptr());
            let api_call = sys::SteamAPI_ISteamUGC_SubmitItemUpdate(self.ugc, self.handle, note);
            register_call_result::<sys::SubmitItemUpdateResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    cb(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult).map(|()| {
                            (
                                PublishedFileId(v.m_nPublishedFileId),
                                v.m_bUserNeedsToAcceptWorkshopLegalAgreement,
                            )
                        })
                    })
                },
            );
        }
        UpdateWatchHandle {
            ugc: self.ugc,
            _inner: self.inner,
            handle: self.handle,
        }
    }
}

/// A handle to watch an update of a published item
pub struct UpdateWatchHandle {
    pub(super) ugc: *mut sys::ISteamUGC,
    _inner: Arc<Inner>,

    pub(super) handle: sys::UGCUpdateHandle_t,
}

// SAFETY: nothing in this crate's tree needs `Send` for this type today (measured: this
// line deleted, `cargo check --all-targets` stays green). The promise is kept because
// the type is public and a caller may move it to the thread that pumps callbacks.
// The type holds a raw Steam interface pointer, plain-copy handles, and the owning `Arc<Inner>`;
// it carries no non-atomic reference count, which is the one hazard a `Send` claim here cannot
// see -- `scripts/ci/callback_threads.py` counts `Rc` tokens beside every promise and ships at
// zero for exactly that reason. Moving the value therefore moves sole ownership of the pointer;
// nothing reads the same state through two threads because of the move itself.
unsafe impl Send for UpdateWatchHandle {}
unsafe impl Sync for UpdateWatchHandle {}

impl UpdateWatchHandle {
    pub fn progress(&self) -> (UpdateStatus, u64, u64) {
        unsafe {
            let mut progress = 0;
            let mut total = 0;
            let status = sys::SteamAPI_ISteamUGC_GetItemUpdateProgress(
                self.ugc,
                self.handle,
                &mut progress,
                &mut total,
            );
            let status = match status {
                sys::EItemUpdateStatus::k_EItemUpdateStatusInvalid => UpdateStatus::Invalid,
                sys::EItemUpdateStatus::k_EItemUpdateStatusPreparingConfig => {
                    UpdateStatus::PreparingConfig
                }
                sys::EItemUpdateStatus::k_EItemUpdateStatusPreparingContent => {
                    UpdateStatus::PreparingContent
                }
                sys::EItemUpdateStatus::k_EItemUpdateStatusUploadingContent => {
                    UpdateStatus::UploadingContent
                }
                sys::EItemUpdateStatus::k_EItemUpdateStatusUploadingPreviewFile => {
                    UpdateStatus::UploadingPreviewFile
                }
                sys::EItemUpdateStatus::k_EItemUpdateStatusCommittingChanges => {
                    UpdateStatus::CommittingChanges
                }
                _ => UpdateStatus::Unknown,
            };
            (status, progress, total)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStatus {
    Invalid,
    PreparingConfig,
    PreparingContent,
    UploadingContent,
    UploadingPreviewFile,
    CommittingChanges,
    /// Steam reported an update status this binding does not name; see
    /// [`AuthSessionError::Unknown`] for why the arm exists.
    Unknown,
}
