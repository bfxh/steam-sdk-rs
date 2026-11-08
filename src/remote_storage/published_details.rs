//! One published file, described: the details call whose answer is twenty fields wide.
//!
//! Steam introduces it as "Gets published file details for the given publishedfileid.  If
//! unMaxSecondsOld is greater than 0, cached data may be returned, depending on how long ago it was
//! cached.  A value of 0 will force a refresh." (`isteamremotestorage.h:276-277`) and declares the
//! call as "virtual SteamAPICall_t	GetPublishedFileDetails( PublishedFileId_t unPublishedFileId,
//! uint32 unMaxSecondsOld ) = 0;" (`:280`) under `STEAM_CALL_RESULT(
//! RemoteStorageGetPublishedFileDetailsResult_t )` (`:279`).
//!
//! The answer (`:469-493`) is where this crate's judgements concentrate: five fixed-size character
//! buffers read inside their own bounds, two handles translated through Steam's invalid marker, two
//! `int32` sizes clamped, and a comma-separated tag list whose own sibling field says whether it fit.

use super::download::fixed_name;
use super::*;

/// How fresh the details must be before Steam may answer from its cache.
///
/// The two arms are Steam's two documented cases (`isteamremotestorage.h:276-277`). A third case the
/// same comment mentions -- "A value of k_WorkshopForceLoadPublishedFileDetailsFromCache will use
/// cached data if it exists, no matter how old it is" (`:278`) -- is **not** offered here: that
/// constant is named once in the vendored SDK and declared nowhere in it (measured: a recursive search
/// of `steamworks-sys/` finds exactly that one comment line), so there is no number this wrapper could
/// send without inventing it. A caller who needs the newest data uses [`PublishedFileFreshness::ForceRefresh`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublishedFileFreshness {
    /// "A value of 0 will force a refresh" (`isteamremotestorage.h:277`).
    ForceRefresh,
    /// "cached data may be returned, depending on how long ago it was cached" (`:276-277`), bounded by
    /// the caller's own age in seconds.
    CachedAtMost(u32),
}

/// The number Steam's `unMaxSecondsOld` receives.
fn max_seconds_old(freshness: PublishedFileFreshness) -> u32 {
    match freshness {
        PublishedFileFreshness::ForceRefresh => 0,
        PublishedFileFreshness::CachedAtMost(seconds) => seconds,
    }
}

/// The tag list Steam writes as one comma-separated buffer
/// ("comma separated list of all tags associated with this file", `isteamremotestorage.h:485`).
///
/// Empty pieces are dropped, so the separators around `a,,b` and a trailing comma do not become a tag
/// with no name. A list Steam could not fit is its "whether the list of tags was too long to be
/// returned in the provided buffer" flag (`isteamremotestorage.h:486`), which
/// [`PublishedFileDetails`] keeps beside the list: this function cannot tell a short list from a cut
/// one, so it does not pretend to.
fn split_tags(tags: Option<&str>) -> Vec<String> {
    tags.map(|list| {
        list.split(',')
            .filter(|tag| !tag.is_empty())
            .map(String::from)
            .collect()
    })
    .unwrap_or_default()
}

/// One published file, as Steam described it.
///
/// The fields are Steam's
/// "struct RemoteStorageGetPublishedFileDetailsResult_t"
/// (`isteamremotestorage.h:469-493`) read as declared, with the conversions the rest of this crate
/// already applies: the two handles through Steam's own invalid marker (`:478-479` with `:45`), the
/// two sizes clamped (`:488-489`), and the five text buffers bounded by their own lengths
/// (`:476-477`, `:485`, `:487`, `:490`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedFileDetails {
    pub published_file_id: PublishedFileId,
    /// "ID of the app that created this file." (`isteamremotestorage.h:474`).
    pub creator_app_id: AppId,
    /// "ID of the app that will consume this file." (`:475`).
    pub consumer_app_id: AppId,
    pub title: Option<String>,
    pub description: Option<String>,
    /// "The handle of the primary file" (`isteamremotestorage.h:478`), or nothing when Steam answers
    /// with its own marker (`:45`) -- which is normal for an item with no file behind it, such as a
    /// collection, not an error.
    pub file: Option<UgcHandle>,
    /// "The handle of the preview file" (`isteamremotestorage.h:479`), same rule as [`Self::file`].
    pub preview_file: Option<UgcHandle>,
    /// "Steam ID of the user who created this content." (`:480`), passed through as written.
    pub owner: SteamId,
    /// `m_rtimeCreated` (`isteamremotestorage.h:481`) as Steam wrote it: the declaration says "time
    /// when the published file was created" and gives no unit, so no clock is invented here -- the
    /// same passthrough rule as `RTime32` in `src/user/answers.rs` and the subscribe page.
    pub created_at: u32,
    /// Steam's "time when the published file was last updated" (`isteamremotestorage.h:482`), same
    /// rule as [`Self::created_at`].
    pub updated_at: u32,
    pub visibility: PublishedFileVisibility,
    /// Steam's "bool m_bBanned;" (`isteamremotestorage.h:484`). The line carries no comment, so this
    /// wrapper passes the flag through without reading anything into what Steam bans.
    pub banned: bool,
    /// Steam's "comma separated list of all tags associated with this file"
    /// (`isteamremotestorage.h:485`), split as `split_tags` describes. An empty vector means either no tags or a buffer that was not readable as text --
    /// Steam's `m_bTagsTruncated` (`:486`) is kept beside it, but nothing here can undo a cut list.
    pub tags: Vec<String>,
    /// Steam's "bool m_bTagsTruncated;" (`isteamremotestorage.h:486`), kept because dropping it would
    /// let a cut-off tag list read like a complete one.
    pub tags_truncated: bool,
    /// "The name of the primary file" (`isteamremotestorage.h:487`), bounded by that 260-entry array.
    pub file_name: Option<String>,
    /// "Size of the primary file" (`isteamremotestorage.h:488`) and "Size of the preview file" (`:489`),
    /// both from Steam's signed `int32`: see `details_size`.
    pub file_size: u64,
    pub preview_file_size: u64,
    /// "URL (for a video or a website)" (`isteamremotestorage.h:490`).
    pub url: Option<String>,
    /// "Type of the file" (`isteamremotestorage.h:491`), read through the crate's `FileType`, whose
    /// `Unknown` arm exists precisely because `k_EWorkshopFileTypeMax` is named by neither side.
    pub file_type: FileType,
    /// "developer has specifically flagged this item as accepted in the Workshop" (`:492`).
    pub accepted_for_use: bool,
}

/// The answer, field by field. Steam's status is checked by the caller of this function, which owns
/// the "EResult m_eResult;" (`isteamremotestorage.h:472`) the same struct carries.
fn details_of(v: &sys::RemoteStorageGetPublishedFileDetailsResult_t) -> PublishedFileDetails {
    let tags = fixed_name(&v.m_rgchTags);
    PublishedFileDetails {
        published_file_id: PublishedFileId(v.m_nPublishedFileId),
        creator_app_id: AppId(v.m_nCreatorAppID),
        consumer_app_id: AppId(v.m_nConsumerAppID),
        title: fixed_name(&v.m_rgchTitle),
        description: fixed_name(&v.m_rgchDescription),
        file: handle_or_none(v.m_hFile),
        preview_file: handle_or_none(v.m_hPreviewFile),
        owner: SteamId::from_raw(v.m_ulSteamIDOwner),
        created_at: v.m_rtimeCreated,
        updated_at: v.m_rtimeUpdated,
        visibility: PublishedFileVisibility::from(v.m_eVisibility),
        banned: v.m_bBanned,
        tags_truncated: v.m_bTagsTruncated,
        tags: split_tags(tags.as_deref()),
        file_name: fixed_name(&v.m_pchFileName),
        file_size: details_size(v.m_nFileSize),
        preview_file_size: details_size(v.m_nPreviewFileSize),
        url: fixed_name(&v.m_rgchURL),
        file_type: FileType::from(v.m_eFileType),
        accepted_for_use: v.m_bAcceptedForUse,
    }
}

impl RemoteStorage {
    /// Ask Steam to describe one published file, answering through `cb`.
    ///
    /// The `freshness` argument is Steam's `unMaxSecondsOld`: "If unMaxSecondsOld is greater than 0,
    /// cached data may be returned, depending on how long ago it was cached.  A value of 0 will force a
    /// refresh." (`isteamremotestorage.h:276-277`) -- see [`PublishedFileFreshness`] for the third case
    /// the header names and this wrapper does not offer.
    ///
    /// A failure to find the item is Steam's `m_eResult` and arrives as `Err`; the fields are then not
    /// read, because an unwritten struct is exactly what this function would otherwise report as an
    /// item with an empty title, zero sizes and no tags.
    pub fn get_published_file_details<F>(
        &self,
        published_file_id: PublishedFileId,
        freshness: PublishedFileFreshness,
        cb: F,
    ) where
        F: FnOnce(Result<PublishedFileDetails, SteamError>) + 'static + Send,
    {
        let api_call = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_GetPublishedFileDetails(
                self.rs,
                published_file_id.0,
                max_seconds_old(freshness),
            )
        };
        unsafe {
            register_call_result::<sys::RemoteStorageGetPublishedFileDetailsResult_t, _>(
                &self.inner,
                api_call,
                move |v, io_error| {
                    if io_error {
                        return cb(Err(SteamError::IOFailure));
                    }
                    cb(to_steam_result(v.m_eResult).map(|()| details_of(v)))
                },
            );
        }
    }
}

#[cfg(test)]
mod published_details_tests {
    //! The freshness number, the tag list, and the two helpers the mapping leans on. Whether Steam's
    //! answer matches the Workshop page for a given item needs a live client and is registered as
    //! unverified -- as is what the two handle fields hold for an item that has no file.

    use super::{max_seconds_old, split_tags, PublishedFileFreshness};

    /// Steam's own sentence fixes the number for a forced refresh; everything else is the caller's.
    #[test]
    fn a_refresh_is_steam_s_zero_and_an_age_is_the_callers() {
        assert_eq!(max_seconds_old(PublishedFileFreshness::ForceRefresh), 0);
        assert_eq!(max_seconds_old(PublishedFileFreshness::CachedAtMost(0)), 0);
        assert_eq!(
            max_seconds_old(PublishedFileFreshness::CachedAtMost(3600)),
            3600
        );
        assert_eq!(
            max_seconds_old(PublishedFileFreshness::CachedAtMost(u32::MAX)),
            u32::MAX,
            "no clamping: u32 is what the parameter holds"
        );
    }

    /// The separators are Steam's; an empty piece between two of them is not a tag.
    #[test]
    fn the_comma_separated_list_splits_without_inventing_tags() {
        assert_eq!(split_tags(None), Vec::<String>::new());
        assert_eq!(split_tags(Some("")), Vec::<String>::new());
        assert_eq!(
            split_tags(Some("fun,map")),
            vec!["fun".to_owned(), "map".to_owned()]
        );
        assert_eq!(
            split_tags(Some("a,,b,")),
            vec!["a".to_owned(), "b".to_owned()],
            "the doubled and trailing separators carry no tag"
        );
        assert_eq!(
            split_tags(Some("solo")),
            vec!["solo".to_owned()],
            "a one-tag list keeps its single entry"
        );
    }
}
