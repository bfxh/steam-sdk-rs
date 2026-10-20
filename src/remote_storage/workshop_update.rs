//! The legacy Workshop publish-update surface: the request handle Steam hands out for one
//! published file, and the seven field setters that fill it in before the async commit.
//!
//! Steam introduces the block as "publishing UGC" (`isteamremotestorage.h:264`) and declares the
//! handle-taking calls as plain `virtual bool` -- "virtual bool UpdatePublishedFileFile(
//! PublishedFileUpdateHandle_t updateHandle, const char *pchFile ) = 0;" (`:268`) and its six
//! siblings (`:269-273`, `:292`). Those seven plus `CreatePublishedFileUpdateRequest` (`:267`) are
//! what lives here; the rest of that block returns `SteamAPICall_t` and is registered as unwrapped
//! in `docs/SuperSteam-Design.md` §6.1d, because an answer that arrives as a call result needs a
//! result struct per call, not a wrapper method.
//!
//! Nothing here claims the block is deprecated. `isteamremotestorage.h` carries no deprecation
//! marker for it (measured with a case-insensitive `deprecat` scan: the only two hits in the whole
//! header are `:354` and `:368`, both about callback ids -- "deprecated! Do not reuse"), unlike
//! "The older ISteamController interface has been deprecated in favor of ISteamInput"
//! (`isteamcontroller.h:2`). It is a parallel surface to `ISteamUGC`, not a superseded one.

use super::*;
use crate::remote_storage::workshop_publish::PublishedFileAnswer;
use crate::utils::SteamParamStringArray;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::Path;

/// A published file whose update fields are being filled in.
///
/// Steam's own marker for a request that does not exist is declared next to the type: "const
/// PublishedFileUpdateHandle_t k_PublishedFileUpdateHandleInvalid = 0xffffffffffffffffull;"
/// (`isteamremotestorage.h:46`), which is all ones -- the same shape as `k_UGCHandleInvalid` (`:45`)
/// and not the zero that `k_PublishedFileIdInvalid` uses (`:44`). The handle therefore has no
/// "is valid" helper; the boundary translates the marker and the setters only ever see a request
/// that came from Steam.
///
/// Like the modern [`crate::UpdateHandle`], this keeps the client alive with `Arc<Inner>`: the
/// setters call back into Steam, and a wrapper that outlived `Client` would hand a dangling
/// interface pointer to them.
pub struct PublishedFileUpdate {
    rs: *mut sys::ISteamRemoteStorage,
    _inner: Arc<Inner>,
    handle: sys::PublishedFileUpdateHandle_t,
}

/// A request handle, or nothing when Steam answered with its own marker.
fn request_or_none(raw: u64) -> Option<sys::PublishedFileUpdateHandle_t> {
    if raw == sys::k_PublishedFileUpdateHandleInvalid {
        return None;
    }
    Some(raw)
}

/// A path Steam must read from disk.
///
/// A missing path is `FileNotFound`, another OS answer is `IOFailure`: "I could not stat it" is not
/// the same answer as "it is not there". The rule and the split are the ones the modern sibling
/// already applies to the same kind of argument (`src/ugc/update.rs:22-31`); what matters here is
/// that refusing happens before the FFI call, so a typo never becomes a partial update request.
pub(super) fn readable_path(path: &Path) -> Result<CString, SteamError> {
    let resolved = path.canonicalize().map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => SteamError::FileNotFound,
        _ => SteamError::IOFailure,
    })?;
    cloud_file_name(&resolved.to_string_lossy())
}

/// The `bool` Steam answers a setter with, in this crate's vocabulary.
///
/// Every setter in "publishing UGC" (`isteamremotestorage.h:264`) returns `virtual bool`, and false
/// is Steam saying it did not take the field. `assert!` would have panicked in the caller over
/// something Steam had already reported.
fn setter_accepted(ok: bool) -> Result<(), SteamError> {
    if ok {
        Ok(())
    } else {
        Err(SteamError::InvalidParameter)
    }
}

/// The metadata Steam reports for one downloaded piece of content.
///
/// "Gets metadata for a file after it has been downloaded. This is the same metadata given in the
/// RemoteStorageDownloadUGCResult_t call result" (`isteamremotestorage.h:249`), which is why this
/// struct exists even though the async result that carries the same four fields is not wrapped: the
/// synchronous read is available to a caller that already has a handle.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UgcDetails {
    pub app_id: AppId,
    /// Steam's "char **ppchName" (`isteamremotestorage.h:250`), or nothing when the pointer is null
    /// or the bytes are not UTF-8 -- the same collapse the friend history face makes.
    pub name: Option<String>,
    /// Steam's "int32 *pnFileSizeInBytes" (`isteamremotestorage.h:250`), clamped so a negative cannot become a huge
    /// length.
    pub size: u64,
    /// Steam's "CSteamID *pSteamIDOwner" (`isteamremotestorage.h:250`), passed through as written: an all-zero value is
    /// what a default-constructed `CSteamID` looks like (`steamclientpublic.h:491-497`), and
    /// [`SteamId::is_invalid`] is the predicate for that, not this wrapper's silence.
    pub owner: SteamId,
}

/// One name Steam offered by out-parameter, or nothing.
///
/// The declaration is Steam's "char **ppchName" with `STEAM_OUT_STRING()` (`isteamremotestorage.h:250`),
/// so a null is a name Steam chose not to write, and a non-UTF-8 buffer is not a name either --
/// converting lossily would hand back replacement characters that look like a title and compare
/// against nothing a caller could check. Both shapes read as `None`, the same collapse the friend
/// history face already makes (`src/friends/history.rs:25-31`).
fn details_name(raw: *mut c_char) -> Option<String> {
    if raw.is_null() {
        return None;
    }
    let text = unsafe { CStr::from_ptr(raw) }.to_str().ok()?;
    if text.is_empty() {
        return None;
    }
    Some(text.to_owned())
}

/// The size Steam wrote, from its signed `int32` (`isteamremotestorage.h:250`).
///
/// A negative is not a number of bytes: `as u64` would sign-extend it into something about 1.8e19
/// large, which is the same defect this crate already closed on "virtual int32 GetFileSize( const char
/// *pchFile ) = 0;" (`:223`) and on the download-progress pair, whose counters Steam declares as
/// "int32 *pnBytesDownloaded" (`:247`). Zero is what no size written already looks like.
pub(super) fn details_size(raw: ::std::os::raw::c_int) -> u64 {
    u64::try_from(raw).unwrap_or(0)
}

impl RemoteStorage {
    /// Begin an update request for one published file, or `None` when Steam refuses.
    ///
    /// Steam's declaration is "virtual PublishedFileUpdateHandle_t CreatePublishedFileUpdateRequest(
    /// PublishedFileId_t unPublishedFileId ) = 0;" (`isteamremotestorage.h:267`) and its refusal is
    /// the all-ones marker (`:46`), not zero -- so a `None` here means no request was created, and
    /// the caller has no handle to fill in.
    pub fn create_published_file_update_request(
        &self,
        published_file_id: PublishedFileId,
    ) -> Option<PublishedFileUpdate> {
        let raw = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_CreatePublishedFileUpdateRequest(
                self.rs,
                published_file_id.0,
            )
        };
        request_or_none(raw).map(|handle| PublishedFileUpdate {
            rs: self.rs,
            _inner: self.inner.clone(),
            handle,
        })
    }

    /// The metadata Steam has for a downloaded piece of content, or `None` when it says it has none.
    ///
    /// "Gets metadata for a file after it has been downloaded." (`isteamremotestorage.h:249`) -- the
    /// after is load-bearing: the call answers false for content that is not downloaded yet, and
    /// false is the whole of Steam's answer, so it cannot be told apart from a handle this wrapper
    /// does not know. The four out-parameters are read only when it answers true.
    pub fn ugc_details(&self, handle: UgcHandle) -> Option<UgcDetails> {
        let mut app_id: sys::AppId_t = 0;
        let mut name: *mut c_char = std::ptr::null_mut();
        let mut size: ::std::os::raw::c_int = 0;
        let mut owner: sys::CSteamID = sys::CSteamID {
            m_steamid: sys::CSteamID_SteamID_t { m_unAll64Bits: 0 },
        };
        let ok = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_GetUGCDetails(
                self.rs,
                handle.0,
                &mut app_id,
                &mut name,
                &mut size,
                &mut owner,
            )
        };
        if !ok {
            return None;
        }
        // SAFETY: the union was constructed through `m_unAll64Bits`, so the 64-bit arm is the one
        // holding a value -- initialized by us at zero, then written by Steam or left alone.
        let owner_raw = unsafe { owner.m_steamid.m_unAll64Bits };
        Some(UgcDetails {
            app_id: AppId(app_id),
            name: details_name(name),
            size: details_size(size),
            owner: SteamId::from_raw(owner_raw),
        })
    }
}

/// The seven field setters, on the request they fill in.
///
/// Each one is a `virtual bool` in "publishing UGC" (`isteamremotestorage.h:264`), so each takes the
/// request by value and hands it back on success: a caller that ignores a refusal gets the chain
/// stopping there rather than a half-filled request committed later.
impl PublishedFileUpdate {
    /// The content file Steam should upload for this request.
    ///
    /// "virtual bool UpdatePublishedFileFile( PublishedFileUpdateHandle_t updateHandle, const char
    /// *pchFile ) = 0;" (`isteamremotestorage.h:268`). A path that cannot be resolved, or that
    /// cannot become a C string, is refused before the call; Steam's own refusal is
    /// `InvalidParameter`.
    #[must_use]
    pub fn file(self, path: &Path) -> Result<Self, SteamError> {
        let path = readable_path(path)?;
        setter_accepted(unsafe {
            sys::SteamAPI_ISteamRemoteStorage_UpdatePublishedFileFile(
                self.rs,
                self.handle,
                path.as_ptr(),
            )
        })?;
        Ok(self)
    }

    /// The preview image Steam should show for this request.
    ///
    /// "virtual bool UpdatePublishedFilePreviewFile( PublishedFileUpdateHandle_t updateHandle, const
    /// char *pchPreviewFile ) = 0;" (`isteamremotestorage.h:269`), with the same two refusals as
    /// [`Self::file`].
    #[must_use]
    pub fn preview_file(self, path: &Path) -> Result<Self, SteamError> {
        let path = readable_path(path)?;
        setter_accepted(unsafe {
            sys::SteamAPI_ISteamRemoteStorage_UpdatePublishedFilePreviewFile(
                self.rs,
                self.handle,
                path.as_ptr(),
            )
        })?;
        Ok(self)
    }

    /// The title for this request.
    ///
    /// "virtual bool UpdatePublishedFileTitle( PublishedFileUpdateHandle_t updateHandle, const char
    /// *pchTitle ) = 0;" (`isteamremotestorage.h:270`). An embedded NUL has no Steam code to send
    /// and fails with `InvalidParameter`, as does a title Steam itself refuses.
    #[must_use]
    pub fn title(self, title: &str) -> Result<Self, SteamError> {
        let title = cloud_file_name(title)?;
        setter_accepted(unsafe {
            sys::SteamAPI_ISteamRemoteStorage_UpdatePublishedFileTitle(
                self.rs,
                self.handle,
                title.as_ptr(),
            )
        })?;
        Ok(self)
    }

    /// The description for this request.
    ///
    /// "virtual bool UpdatePublishedFileDescription( PublishedFileUpdateHandle_t updateHandle, const
    /// char *pchDescription ) = 0;" (`isteamremotestorage.h:271`).
    #[must_use]
    pub fn description(self, description: &str) -> Result<Self, SteamError> {
        let description = cloud_file_name(description)?;
        setter_accepted(unsafe {
            sys::SteamAPI_ISteamRemoteStorage_UpdatePublishedFileDescription(
                self.rs,
                self.handle,
                description.as_ptr(),
            )
        })?;
        Ok(self)
    }

    /// The visibility for this request.
    ///
    /// "virtual bool UpdatePublishedFileVisibility( PublishedFileUpdateHandle_t updateHandle,
    /// ERemoteStoragePublishedFileVisibility eVisibility ) = 0;" (`isteamremotestorage.h:272`). A
    /// visibility read back from a newer client (`Unknown`) is refused here before the call, because
    /// the vendored enum names only 0..3 -- the same boundary the modern sibling holds
    /// (`src/ugc/update.rs:107-124`).
    #[must_use]
    pub fn visibility(self, visibility: PublishedFileVisibility) -> Result<Self, SteamError> {
        let visibility = sys::ERemoteStoragePublishedFileVisibility::try_from(visibility)?;
        setter_accepted(unsafe {
            sys::SteamAPI_ISteamRemoteStorage_UpdatePublishedFileVisibility(
                self.rs,
                self.handle,
                visibility,
            )
        })?;
        Ok(self)
    }

    /// The tag list for this request.
    ///
    /// "virtual bool UpdatePublishedFileTags( PublishedFileUpdateHandle_t updateHandle,
    /// SteamParamStringArray_t *pTags ) = 0;" (`isteamremotestorage.h:273`). One tag that cannot
    /// become a C string refuses the whole call: `SteamParamStringArray::new` hands back `None` and
    /// frees what it had already converted, so a partial tag list is never sent. An empty list is
    /// not refused here; what Steam does with zero tags is not said in the header, and is registered
    /// as unverified rather than guessed at.
    #[must_use]
    pub fn tags<S: AsRef<str>>(self, tags: &[S]) -> Result<Self, SteamError> {
        let mut array =
            SteamParamStringArray::new(tags).ok_or_else(|| SteamError::InvalidParameter)?;
        let mut raw = array.as_raw();
        setter_accepted(unsafe {
            sys::SteamAPI_ISteamRemoteStorage_UpdatePublishedFileTags(
                self.rs,
                self.handle,
                &mut raw,
            )
        })?;
        Ok(self)
    }

    /// The change note Steam should show for this request.
    ///
    /// "virtual bool UpdatePublishedFileSetChangeDescription( PublishedFileUpdateHandle_t
    /// updateHandle, const char *pchChangeDescription ) = 0;" (`isteamremotestorage.h:292`) -- the
    /// one setter in the block that Steam put after the vote calls rather than beside the other six
    /// (`:268-273`), which is why its line number is not in that run.
    #[must_use]
    pub fn change_description(self, change_description: &str) -> Result<Self, SteamError> {
        let change_description = cloud_file_name(change_description)?;
        setter_accepted(unsafe {
            sys::SteamAPI_ISteamRemoteStorage_UpdatePublishedFileSetChangeDescription(
                self.rs,
                self.handle,
                change_description.as_ptr(),
            )
        })?;
        Ok(self)
    }

    /// Send the filled-in request, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t CommitPublishedFileUpdate( PublishedFileUpdateHandle_t updateHandle ) = 0;"
    /// (`isteamremotestorage.h:275`), under `STEAM_CALL_RESULT( RemoteStorageUpdatePublishedFileResult_t )`
    /// (`:274`). This is the far end of the eight synchronous entries above: without it a request can
    /// be filled in but never sent.
    ///
    /// The request is consumed, because Steam takes it away at this call -- keeping a live handle to
    /// fill in afterwards would be editing an update that is already on its way.
    pub fn commit<F>(self, cb: F)
    where
        F: FnOnce(Result<PublishedFileAnswer, SteamError>) + 'static + Send,
    {
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_CommitPublishedFileUpdate(self.rs, self.handle)
        };
        unsafe {
            register_call_result::<sys::RemoteStorageUpdatePublishedFileResult_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    if io_error {
                        return cb(Err(SteamError::IOFailure));
                    }
                    cb(to_steam_result(v.m_eResult).map(|()| PublishedFileAnswer {
                        published_file_id: PublishedFileId(v.m_nPublishedFileId),
                        needs_legal_acceptance: v.m_bUserNeedsToAcceptWorkshopLegalAgreement,
                    }))
                },
            );
        }
    }
}

#[cfg(test)]
mod workshop_update_tests {
    //! The handle marker, the path/text refusals that run before any FFI call, and the two
    //! out-parameter reads. Which published file a request is actually for, and what Steam writes
    //! into the four metadata fields, need a live client and are registered as unverified.

    use super::{
        details_name, details_size, request_or_none, setter_accepted, PublishedFileVisibility,
        SteamError,
    };
    use std::ffi::CString;
    use std::os::raw::{c_char, c_int};
    use steamworks_sys as sys;

    /// Steam's marker for "no request" is all ones, and zero is an ordinary handle value.
    /// Reading them the wrong way round throws away the requests that exist.
    #[test]
    fn the_invalid_update_handle_is_all_ones_and_not_zero() {
        assert_eq!(
            sys::k_PublishedFileUpdateHandleInvalid,
            u64::MAX,
            "isteamremotestorage.h:46 declares 0xffffffffffffffffull; a `0` here would be a different claim"
        );
        assert_eq!(
            request_or_none(sys::k_PublishedFileUpdateHandleInvalid),
            None
        );
        assert_eq!(request_or_none(0), Some(0));
        assert_eq!(
            request_or_none(18446744073709551614),
            Some(18446744073709551614)
        );
    }

    /// A setter that Steam refused is an error, not a panic and not silence.
    #[test]
    fn a_refused_setter_is_an_error_and_an_accepted_one_is_not() {
        assert!(setter_accepted(true).is_ok());
        assert_eq!(
            setter_accepted(false).unwrap_err(),
            SteamError::InvalidParameter,
            "false from a `virtual bool` setter is Steam's refusal, and must not look like success"
        );
    }

    /// A negative size is not a number of bytes, and zero keeps its own meaning.
    #[test]
    fn a_signed_size_never_becomes_a_huge_one() {
        assert_eq!(details_size(0), 0);
        assert_eq!(details_size(4096), 4096);
        assert_eq!(details_size(-1), 0);
        assert_eq!(details_size(c_int::MIN), 0);
        assert_ne!(details_size(-1), u64::MAX);
    }

    /// Three shapes, one answer: a name Steam did not write is not a name the caller can show.
    #[test]
    fn a_name_is_readable_only_when_steam_writes_one() {
        assert_eq!(details_name(std::ptr::null_mut()), None);
        let empty = CString::new("").unwrap();
        assert_eq!(details_name(empty.as_ptr() as *mut c_char), None);
        let named = CString::new("Workshop Map 3").unwrap();
        assert_eq!(
            details_name(named.as_ptr() as *mut c_char),
            Some("Workshop Map 3".to_owned())
        );
        let not_utf8 = CString::new(vec![0xff, 0xfe]).unwrap();
        assert_eq!(details_name(not_utf8.as_ptr() as *mut c_char), None);
    }

    /// The visibility that can only be read is refused on the way in, before any call -- the
    /// boundary the modern sibling holds for the same enum.
    #[test]
    fn an_unnameable_visibility_is_refused_before_steam_is_asked() {
        assert!(sys::ERemoteStoragePublishedFileVisibility::try_from(
            PublishedFileVisibility::Public
        )
        .is_ok());
        assert!(sys::ERemoteStoragePublishedFileVisibility::try_from(
            PublishedFileVisibility::Unknown
        )
        .is_err());
    }
}
