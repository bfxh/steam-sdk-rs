//! Publishing a brand-new Workshop item: the two calls that create one, and the answer Steam gives
//! when the item exists.
//!
//! Both are annotated in the header with `RemoteStoragePublishFileProgress_t` (`:265`, `:301`), whose
//! only fields are "double m_dPercentFile;" and "bool m_bPreview;" (`isteamremotestorage.h:613-614`) --
//! a progress report, carrying no item id and no completion. The header also declares, just for these
//! calls, "Purpose: The result of a call to PublishFile()" over `RemoteStoragePublishFileResult_t`
//! (`:357-360`), whose `k_iCallback` is `k_iSteamRemoteStorageCallbacks + 9` (`:362`) and whose payload
//! is the result, the new item's id and the legal flag (`:363-365`). This wrapper answers with that
//! struct: a `Purpose` line written for the call, a callback id assigned to it, and a payload with the
//! only thing a caller can act on (the id) outweigh a `STEAM_CALL_RESULT` line that names a progress
//! struct -- and an answer without an id could not be used for anything. The contradiction is recorded
//! in `docs/SuperSteam-Design.md` §6.1d rather than left out, because the annotation is what a reader
//! would check next.
//!
//! The decision that belongs to this wrapper is that a submission is all-or-nothing: every string is
//! converted to a C string, and every path resolved, *before* Steam is asked. One unencodable title
//! must not become a published item with no title.

use super::*;
use std::os::raw::{c_char, c_int};
use std::path::Path;

/// The answer Steam gives once the item exists -- for a new publication and for a committed update.
///
/// Both result structs declare the same three fields: "EResult m_eResult;" with
/// `PublishedFileId_t m_nPublishedFileId;` and `bool m_bUserNeedsToAcceptWorkshopLegalAgreement;`
/// (`isteamremotestorage.h:363-365` for the publish, `:445-447` for the commit). The two FFI reads stay
/// separate (`workshop_update.rs` reads its own struct) -- this is one *safe* type for the same two
/// facts, not a claim that Steam keeps the two structs in step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublishedFileAnswer {
    pub published_file_id: PublishedFileId,
    /// Steam's "bool m_bUserNeedsToAcceptWorkshopLegalAgreement;" (`isteamremotestorage.h:365`,
    /// `:447`): Steam's flag,
    /// read-only. Nothing here can accept the agreement, so a caller that ignores this shows an item as
    /// live when Steam says it is waiting.
    pub needs_legal_acceptance: bool,
}

/// Which site hosts the video Steam should attach, for [`RemoteStorage::publish_video`].
///
/// `enum EWorkshopVideoProvider` (`isteamremotestorage.h:137-141`) names exactly two codes,
/// "k_EWorkshopVideoProviderNone = 0," (`:139`) and "k_EWorkshopVideoProviderYoutube = 1" (`:140`).
/// This vocabulary is write-only and has no `Unrecognized` arm on purpose: the enum appears in the
/// vendored headers only as the parameter of `PublishVideo` (`:302`), and no result struct carries a
/// provider back -- measured across every `.h` in `steamworks-sys/lib/steam/public/steam`. A read side
/// would be code no input can reach, unlike [`WorkshopFileAction`], whose code Steam does answer with
/// (`:592`).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum WorkshopVideoProvider {
    /// Steam's "k_EWorkshopVideoProviderNone = 0," (`isteamremotestorage.h:139`): the identifier is
    /// Steam's own, not an external site's.
    NoProvider,
    /// Steam's `k_EWorkshopVideoProviderYoutube` (`:140`).
    Youtube,
}

/// Writes a provider. Total, because the vocabulary above is exactly Steam's two codes.
fn provider_to_sys(provider: WorkshopVideoProvider) -> sys::EWorkshopVideoProvider {
    match provider {
        WorkshopVideoProvider::NoProvider => {
            sys::EWorkshopVideoProvider::k_EWorkshopVideoProviderNone
        }
        WorkshopVideoProvider::Youtube => {
            sys::EWorkshopVideoProvider::k_EWorkshopVideoProviderYoutube
        }
    }
}

/// Everything Steam reads as text, already converted.
///
/// Held by the caller of the FFI so the allocations outlive the pointer Steam takes, and so a failure
/// anywhere in here happens while nothing has been sent yet. The tags stay owned `CString` values
/// rather than Steam's own `SteamParamStringArray`: that type hands out `*mut c_char`, which is not
/// `Send`, and the closure waiting on Steam's answer has to be.
#[derive(Debug)]
struct EncodedCopy {
    preview_file: CString,
    title: CString,
    description: CString,
    tags: Vec<CString>,
    visibility: sys::ERemoteStoragePublishedFileVisibility,
}

/// Steam's `{ int32 m_nNumStrings; char **m_ppStrings; }` pair, borrowed out of owned strings.
///
/// The pointer array comes back with the struct because `m_ppStrings` points into its heap buffer:
/// that buffer stays where it is when the `Vec` itself is moved, but it does not survive being dropped
/// or grown, so the caller has to hold both for as long as Steam may read them. An empty list yields a
/// dangling-but-aligned pointer with a count of zero, which is what `Vec::as_mut_ptr` guarantees and
/// what Steam reads as "no tags" -- nothing is dereferenced while the count is zero.
pub(super) fn tag_array(tags: &[CString]) -> (Vec<*const c_char>, sys::SteamParamStringArray_t) {
    let mut pointers: Vec<*const c_char> = tags.iter().map(|tag| tag.as_ptr()).collect();
    let raw = sys::SteamParamStringArray_t {
        m_nNumStrings: c_int::try_from(pointers.len()).unwrap_or(0),
        m_ppStrings: pointers.as_mut_ptr().cast(),
    };
    (pointers, raw)
}

/// Converts a tag list, refusing the whole list on the first entry that cannot become a C string.
///
/// Shared with the enumerators, which take two tag lists in one call: dropping a single bad tag would
/// run the query under a filter the caller never approved.
pub(super) fn encode_tags(tags: &[String]) -> Result<Vec<CString>, SteamError> {
    let mut encoded = Vec::with_capacity(tags.len());
    for tag in tags {
        encoded.push(CString::new(tag.as_str()).map_err(|_| SteamError::InvalidParameter)?);
    }
    Ok(encoded)
}

/// Converts the text both publish calls carry, refusing the whole submission on the first bad piece.
///
/// Both declarations take the copy as plain text and "SteamParamStringArray_t *pTags"
/// (`isteamremotestorage.h:266`, `:302`), so every string is converted here before anything is sent.
/// The tags are built one at a time and the whole submission is refused on the first unencodable one:
/// dropping a single tag would publish the item with a tag list the caller never approved. A
/// visibility read back from a newer client (`Unknown`) has no code to send
/// (`src/remote_storage/visibility.rs`), so it is refused here too.
fn encode_copy(
    preview_file: &Path,
    title: &str,
    description: &str,
    tags: &[String],
    visibility: PublishedFileVisibility,
) -> Result<EncodedCopy, SteamError> {
    let title = cloud_file_name(title)?;
    let description = cloud_file_name(description)?;
    let encoded_tags = encode_tags(tags)?;
    let visibility = sys::ERemoteStoragePublishedFileVisibility::try_from(visibility)?;
    // Checked last because it is the only step that touches the filesystem; the refusals above are
    // the ones a caller can get from copy alone.
    let preview_file = readable_path(preview_file)?;
    Ok(EncodedCopy {
        preview_file,
        title,
        description,
        tags: encoded_tags,
        visibility,
    })
}

/// A new item to publish, as Steam's `PublishWorkshopFile` takes it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewWorkshopItem {
    /// The "const char *pchFile" (`isteamremotestorage.h:266`): the local file Steam uploads.
    pub file: std::path::PathBuf,
    /// `pchPreviewFile` (`:266`).
    pub preview_file: std::path::PathBuf,
    /// `nConsumerAppId` (`:266`): the app the item is for, which need not be the publishing app.
    pub consumer_app_id: AppId,
    pub title: String,
    pub description: String,
    pub visibility: PublishedFileVisibility,
    pub tags: Vec<String>,
    /// `eWorkshopFileType` (`:266`), the vocabulary `crate::FileType` already carries.
    pub file_type: FileType,
}

/// A video already hosted elsewhere, offered as a Workshop item.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VideoWorkshopItem {
    /// The "EWorkshopVideoProvider eVideoProvider" (`isteamremotestorage.h:302`).
    pub provider: WorkshopVideoProvider,
    /// `pchVideoAccount` (`:302`).
    pub video_account: String,
    /// `pchVideoIdentifier` (`:302`).
    pub video_identifier: String,
    pub preview_file: std::path::PathBuf,
    pub consumer_app_id: AppId,
    pub title: String,
    pub description: String,
    pub visibility: PublishedFileVisibility,
    pub tags: Vec<String>,
}

/// The pair of status and item that both publish answers carry: "EResult m_eResult;", the new id
/// and the legal flag (`isteamremotestorage.h:363-365`).
fn publication(
    status: sys::EResult,
    id: u64,
    needs_acceptance: bool,
) -> Result<PublishedFileAnswer, SteamError> {
    to_steam_result(status).map(|()| PublishedFileAnswer {
        published_file_id: PublishedFileId(id),
        needs_legal_acceptance: needs_acceptance,
    })
}

impl RemoteStorage {
    /// Publish a new Workshop item, answering through `cb` when Steam has created it.
    ///
    /// "virtual SteamAPICall_t PublishWorkshopFile( const char *pchFile, const char *pchPreviewFile,
    /// AppId_t nConsumerAppId, const char *pchTitle, const char *pchDescription,
    /// ERemoteStoragePublishedFileVisibility eVisibility, SteamParamStringArray_t *pTags,
    /// EWorkshopFileType eWorkshopFileType ) = 0;" (`isteamremotestorage.h:266`). Every text field is
    /// converted and both paths resolved before the call, so a bad piece of copy is an error rather
    /// than a published item with something else in its place -- see the module note for which struct
    /// the answer is read from, and why.
    pub fn publish_workshop_file<F>(&self, item: &NewWorkshopItem, cb: F)
    where
        F: FnOnce(Result<PublishedFileAnswer, SteamError>) + 'static + Send,
    {
        let copy = match encode_copy(
            &item.preview_file,
            &item.title,
            &item.description,
            &item.tags,
            item.visibility,
        ) {
            Ok(copy) => copy,
            Err(error) => return cb(Err(error)),
        };
        let file = match readable_path(&item.file) {
            Ok(file) => file,
            Err(error) => return cb(Err(error)),
        };
        let file_type: sys::EWorkshopFileType = item.file_type.into();
        let tags = copy.tags;
        let (_tag_pointers, mut raw) = tag_array(&tags);
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_PublishWorkshopFile(
                self.rs,
                file.as_ptr(),
                copy.preview_file.as_ptr(),
                item.consumer_app_id.0,
                copy.title.as_ptr(),
                copy.description.as_ptr(),
                copy.visibility,
                &mut raw,
                file_type,
            )
        };
        unsafe {
            register_call_result::<sys::RemoteStoragePublishFileResult_t, _>(
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
                    cb(publication(
                        v.m_eResult,
                        v.m_nPublishedFileId,
                        v.m_bUserNeedsToAcceptWorkshopLegalAgreement,
                    ))
                },
            );
        }
    }

    /// Offer an externally hosted video as a Workshop item, answering through `cb`.
    ///
    /// "virtual SteamAPICall_t PublishVideo( EWorkshopVideoProvider eVideoProvider, const char
    /// *pchVideoAccount, const char *pchVideoIdentifier, const char *pchPreviewFile, AppId_t
    /// nConsumerAppId, const char *pchTitle, const char *pchDescription,
    /// ERemoteStoragePublishedFileVisibility eVisibility, SteamParamStringArray_t *pTags ) = 0;"
    /// (`isteamremotestorage.h:302`). Any unencodable account, identifier or copy field is refused
    /// before the call, so a video is never offered with part of its description missing.
    pub fn publish_video<F>(&self, item: &VideoWorkshopItem, cb: F)
    where
        F: FnOnce(Result<PublishedFileAnswer, SteamError>) + 'static + Send,
    {
        let provider = provider_to_sys(item.provider);
        let account = match cloud_file_name(&item.video_account) {
            Ok(account) => account,
            Err(error) => return cb(Err(error)),
        };
        let identifier = match cloud_file_name(&item.video_identifier) {
            Ok(identifier) => identifier,
            Err(error) => return cb(Err(error)),
        };
        let copy = match encode_copy(
            &item.preview_file,
            &item.title,
            &item.description,
            &item.tags,
            item.visibility,
        ) {
            Ok(copy) => copy,
            Err(error) => return cb(Err(error)),
        };
        let tags = copy.tags;
        let (_tag_pointers, mut raw) = tag_array(&tags);
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_PublishVideo(
                self.rs,
                provider,
                account.as_ptr(),
                identifier.as_ptr(),
                copy.preview_file.as_ptr(),
                item.consumer_app_id.0,
                copy.title.as_ptr(),
                copy.description.as_ptr(),
                copy.visibility,
                &mut raw,
            )
        };
        unsafe {
            register_call_result::<sys::RemoteStoragePublishFileResult_t, _>(
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
                    cb(publication(
                        v.m_eResult,
                        v.m_nPublishedFileId,
                        v.m_bUserNeedsToAcceptWorkshopLegalAgreement,
                    ))
                },
            );
        }
    }
}

#[cfg(test)]
mod workshop_publish_tests {
    //! What is refused before Steam is asked, and how the provider codes read back. Whether a
    //! published item appears on the Workshop, and which fields Steam fills in for an item it
    //! rejected, need a live client and are registered as unverified -- as is the choice of answer
    //! struct argued in the module note, which only a running client can settle. The provider table is
    //! write-only for the measured reason in its own doc, so it is tested only for the mapping it
    //! claims, not for a read path no call can produce.

    use super::{
        encode_copy, provider_to_sys, publication, PublishedFileAnswer, WorkshopVideoProvider,
    };
    use crate::{PublishedFileId, PublishedFileVisibility, SteamError};
    use std::os::raw::c_int;
    use std::path::Path;
    use steamworks_sys as sys;

    /// One bad piece of copy refuses the whole submission -- no partial item is ever sent.
    #[test]
    fn any_unencodable_field_refuses_the_whole_submission() {
        let missing = Path::new("qoder-workshop-publish-no-such-file.dat");
        let good = PublishedFileVisibility::Public;
        assert_eq!(
            encode_copy(missing, "title\0", "desc", &[], good).unwrap_err(),
            SteamError::InvalidParameter,
            "a NUL in the title must not become an item published without one"
        );
        assert_eq!(
            encode_copy(missing, "title", "desc\0", &[], good).unwrap_err(),
            SteamError::InvalidParameter
        );
        assert_eq!(
            encode_copy(missing, "title", "desc", &["\0tag".to_owned()], good).unwrap_err(),
            SteamError::InvalidParameter,
            "one unencodable tag refuses the list rather than dropping just that tag"
        );
        assert_eq!(
            encode_copy(
                missing,
                "title",
                "desc",
                &[],
                PublishedFileVisibility::Unknown
            )
            .unwrap_err(),
            SteamError::InvalidParameter,
            "a visibility Steam was never told about cannot be sent as some other visibility"
        );
        assert_eq!(
            encode_copy(missing, "title", "desc", &[], good).unwrap_err(),
            SteamError::FileNotFound,
            "the copy above all encodable, so the missing preview is what stops this one"
        );
    }

    /// The two variants map onto Steam's two codes -- and nothing else is claimed about the mapping.
    #[test]
    fn each_provider_variant_maps_to_the_code_steam_names() {
        assert_eq!(
            provider_to_sys(WorkshopVideoProvider::NoProvider),
            sys::EWorkshopVideoProvider::k_EWorkshopVideoProviderNone,
            "isteamremotestorage.h:139 declares None = 0"
        );
        assert_eq!(
            provider_to_sys(WorkshopVideoProvider::Youtube),
            sys::EWorkshopVideoProvider::k_EWorkshopVideoProviderYoutube,
            "isteamremotestorage.h:140 declares Youtube = 1"
        );
        assert_ne!(
            provider_to_sys(WorkshopVideoProvider::NoProvider) as c_int,
            provider_to_sys(WorkshopVideoProvider::Youtube) as c_int,
            "collapsing the two would attach a video to the wrong host"
        );
    }

    /// The answer keeps both halves of Steam's payload: the id it minted and the legal flag.
    #[test]
    fn the_answer_carries_the_new_id_and_the_legal_flag() {
        assert_eq!(
            publication(sys::EResult::k_EResultOK, 9001, true),
            Ok(PublishedFileAnswer {
                published_file_id: PublishedFileId(9001),
                needs_legal_acceptance: true,
            })
        );
        assert_eq!(
            publication(sys::EResult::k_EResultNone, 9001, false).unwrap_err(),
            SteamError::Generic,
            "a status nobody wrote must not read as a publication that worked"
        );
    }
}
