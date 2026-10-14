//! The answered query: per-item accessors over a handle Steam already filled.
//!
//! Every reader here cuts its own buffer short rather than trusting Steam's terminator,
//! and the shared helpers for that live in `read`.

use super::*;

pub struct QueryResults<'a> {
    pub(super) ugc: *mut sys::ISteamUGC,
    pub(super) handle: sys::UGCQueryHandle_t,
    pub(super) num_results_returned: u32,
    pub(super) num_results_total: u32,
    pub(super) was_cached: bool,
    pub(super) next_cursor: Option<String>,
    pub(super) _phantom: marker::PhantomData<&'a sys::ISteamUGC>,
}
impl<'a> Drop for QueryResults<'a> {
    fn drop(&mut self) {
        unsafe {
            sys::SteamAPI_ISteamUGC_ReleaseQueryUGCRequest(self.ugc, self.handle);
        }
    }
}
impl<'a> QueryResults<'a> {
    /// Were these results retreived from a cache?
    pub fn was_cached(&self) -> bool {
        self.was_cached
    }

    /// Gets the total number of results in this query, not just the current page
    pub fn total_results(&self) -> u32 {
        self.num_results_total
    }

    /// Gets the number of results in this page.
    pub fn returned_results(&self) -> u32 {
        self.num_results_returned
    }

    /// Gets the preview URL of the published file at the specified index.
    pub fn preview_url(&self, index: u32) -> Option<String> {
        let mut url = [0 as c_char; 4096];

        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCPreviewURL(
                self.ugc,
                self.handle,
                index,
                url.as_mut_ptr(),
                url.len() as _,
            )
        };

        if ok {
            Some(unsafe { CStr::from_ptr(url.as_ptr()).to_string_lossy().into_owned() })
        } else {
            None
        }
    }

    /// Gets a UGC statistic about the published file at the specified index.
    pub fn statistic(&self, index: u32, stat_type: UGCStatisticType) -> Option<u64> {
        let mut value = 0u64;

        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCStatistic(
                self.ugc,
                self.handle,
                index,
                stat_type.into(),
                &mut value,
            )
        };

        debug_assert!(ok);

        if ok {
            Some(value)
        } else {
            None
        }
    }

    /// Gets UGCContentDescriptors of the published file at the specified index.
    pub fn content_descriptor(&self, index: u32) -> Vec<UGCContentDescriptorID> {
        // A valid variant rather than `mem::zeroed()`: Steam's descriptor codes
        // start at 1, so an all-zero array is not a value this enum is allowed to
        // hold, and building one would be undefined on its own.
        let mut descriptors: [sys::EUGCContentDescriptorID; 10] =
            [sys::EUGCContentDescriptorID::k_EUGCContentDescriptor_NudityOrSexualContent; 10];
        let max_entries = descriptors.len() as std::ffi::c_uint;

        let num_descriptors = clamped_entries(
            unsafe {
                sys::SteamAPI_ISteamUGC_GetQueryUGCContentDescriptors(
                    self.ugc,
                    self.handle,
                    index,
                    descriptors.as_mut_ptr(),
                    max_entries,
                )
            } as usize,
            descriptors.len(),
        );

        Vec::from(&descriptors[..num_descriptors])
            .iter()
            .map(|&x| x.into())
            .collect()
    }

    /// Gets a result.
    ///
    /// Returns None if index was out of bounds.
    pub fn get(&self, index: u32) -> Option<QueryResult> {
        if index >= self.num_results_returned {
            return None;
        }

        unsafe {
            let mut raw_details: sys::SteamUGCDetails_t = mem::zeroed();
            let ok = sys::SteamAPI_ISteamUGC_GetQueryUGCResult(
                self.ugc,
                self.handle,
                index,
                &mut raw_details,
            );
            debug_assert!(ok);

            if to_steam_result(raw_details.m_eResult).is_err() {
                return None;
            }

            let tags = CStr::from_ptr(raw_details.m_rgchTags.as_ptr())
                .to_string_lossy()
                .split(',')
                .map(|s| String::from(s))
                .collect::<Vec<_>>();

            Some(QueryResult {
                published_file_id: PublishedFileId(raw_details.m_nPublishedFileId),
                creator_app_id: if raw_details.m_nCreatorAppID != 0 {
                    Some(AppId(raw_details.m_nCreatorAppID))
                } else {
                    None
                },
                consumer_app_id: if raw_details.m_nConsumerAppID != 0 {
                    Some(AppId(raw_details.m_nConsumerAppID))
                } else {
                    None
                },
                title: CStr::from_ptr(raw_details.m_rgchTitle.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
                description: CStr::from_ptr(raw_details.m_rgchDescription.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
                owner: SteamId(raw_details.m_ulSteamIDOwner),
                time_created: raw_details.m_rtimeCreated,
                time_updated: raw_details.m_rtimeUpdated,
                time_added_to_user_list: raw_details.m_rtimeAddedToUserList,
                visibility: raw_details.m_eVisibility.into(),
                banned: raw_details.m_bBanned,
                accepted_for_use: raw_details.m_bAcceptedForUse,
                url: CStr::from_ptr(raw_details.m_rgchURL.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
                num_upvotes: raw_details.m_unVotesUp,
                num_downvotes: raw_details.m_unVotesDown,
                score: raw_details.m_flScore,
                num_children: raw_details.m_unNumChildren,
                tags,
                tags_truncated: raw_details.m_bTagsTruncated,
                file_name: CStr::from_ptr(raw_details.m_pchFileName.as_ptr())
                    .to_string_lossy()
                    .into_owned(),
                file_type: raw_details.m_eFileType.into(),
                file_size: raw_details.m_nFileSize.max(0) as u32,
            })
        }
    }

    /// Returns an iterator that runs over all the fetched results
    pub fn iter<'b>(&'b self) -> impl Iterator<Item = Option<QueryResult>> + 'b {
        (0..self.returned_results()).map(move |i| self.get(i))
    }

    /// Returns the given index's children as a list of PublishedFileId.
    ///
    /// You must call `include_children(true)` before fetching the query for this to work.
    ///
    /// Returns None if the index was out of bounds.
    pub fn get_children(&self, index: u32) -> Option<Vec<PublishedFileId>> {
        let num_children = self.get(index)?.num_children;
        let mut children: Vec<sys::PublishedFileId_t> = vec![0; num_children as usize];

        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCChildren(
                self.ugc,
                self.handle,
                index,
                children.as_mut_ptr(),
                num_children,
            )
        };

        if ok {
            Some(children.into_iter().map(Into::into).collect())
        } else {
            None
        }
    }

    /// Returns the number of key value tags associated with the item at the specified index.
    pub fn key_value_tags(&self, index: u32) -> u32 {
        unsafe { sys::SteamAPI_ISteamUGC_GetQueryUGCNumKeyValueTags(self.ugc, self.handle, index) }
    }

    /// Gets the key value pair of a specified key value tag associated with the item at the specified index.
    pub fn get_key_value_tag(&self, index: u32, kv_tag_index: u32) -> Option<(String, String)> {
        let mut key = [0 as c_char; 256];
        let mut value = [0 as c_char; 256];

        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCKeyValueTag(
                self.ugc,
                self.handle,
                index,
                kv_tag_index,
                key.as_mut_ptr(),
                256,
                value.as_mut_ptr(),
                256,
            )
        };

        if ok {
            Some(unsafe {
                (
                    CStr::from_ptr(key.as_ptr()).to_string_lossy().into_owned(),
                    CStr::from_ptr(value.as_ptr())
                        .to_string_lossy()
                        .into_owned(),
                )
            })
        } else {
            None
        }
    }

    /// The tags Steam reports for the item at `index`.
    ///
    /// Steam answers a tag count and then one call per tag, so this reads the
    /// pair itself. A tag Steam refuses (an index past the end, or a query built
    /// without `include_key_value_tags`-style preview data) comes back as `None`
    /// rather than as an empty tag.
    pub fn tag(&self, index: u32, tag_index: u32) -> Option<String> {
        self.read_tag(index, tag_index, sys::SteamAPI_ISteamUGC_GetQueryUGCTag)
    }

    /// The display name of the tag at `tag_index`, which Steam keeps separate
    /// from the tag's machine name ([`QueryResults::tag`]).
    pub fn tag_display_name(&self, index: u32, tag_index: u32) -> Option<String> {
        self.read_tag(
            index,
            tag_index,
            sys::SteamAPI_ISteamUGC_GetQueryUGCTagDisplayName,
        )
    }

    /// How many tags Steam reports for the item at `index`.
    pub fn tag_count(&self, index: u32) -> u32 {
        unsafe { sys::SteamAPI_ISteamUGC_GetQueryUGCNumTags(self.ugc, self.handle, index) }
    }

    fn read_tag(
        &self,
        index: u32,
        tag_index: u32,
        read: unsafe extern "C" fn(
            *mut sys::ISteamUGC,
            sys::UGCQueryHandle_t,
            u32,
            u32,
            *mut c_char,
            u32,
        ) -> bool,
    ) -> Option<String> {
        let mut buffer = vec![0 as c_char; sys::k_cchTagListMax as usize];
        let ok = unsafe {
            read(
                self.ugc,
                self.handle,
                index,
                tag_index,
                buffer.as_mut_ptr(),
                sys::k_cchTagListMax,
            )
        };
        ok.then(|| steam_out_string(&buffer))
    }

    /// How many previews besides the main one Steam reports for `index`.
    pub fn additional_preview_count(&self, index: u32) -> u32 {
        unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCNumAdditionalPreviews(self.ugc, self.handle, index)
        }
    }

    /// One of the item's extra previews: a URL or YouTube id, the original file
    /// name Steam recorded, and what kind of preview it is.
    ///
    /// Requires the query to have been built with
    /// [`QueryHandle::set_return_additional_previews`] (or
    /// [`QueryHandle::include_additional_previews`]); Steam answers `None`
    /// otherwise, which is a missing request, not an empty preview.
    pub fn additional_preview(&self, index: u32, preview_index: u32) -> Option<AdditionalPreview> {
        let mut url = vec![0 as c_char; sys::k_cchPublishedFileURLMax as usize];
        let mut filename = vec![0 as c_char; sys::k_cchFilenameMax as usize];
        // Steam writes a 4-byte EItemPreviewType here. This wrapper deliberately
        // does not read it as `sys::EItemPreviewType`: that enum is `#[repr(i32)]`
        // on Windows and `#[repr(u32)]` on the other platforms, and both are
        // `#[non_exhaustive]`, so a code Steam has not named in this binding must
        // not be turned into one of its variants.
        let mut kind: i32 = 0;
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCAdditionalPreview(
                self.ugc,
                self.handle,
                index,
                preview_index,
                url.as_mut_ptr(),
                sys::k_cchPublishedFileURLMax,
                filename.as_mut_ptr(),
                sys::k_cchFilenameMax,
                &mut kind as *mut i32 as *mut sys::EItemPreviewType,
            )
        };
        ok.then(|| AdditionalPreview {
            url_or_video_id: steam_out_string(&url),
            original_filename: steam_out_string(&filename),
            kind: preview_kind_from(kind),
        })
    }

    /// The first value stored under `key` for the item at `index`.
    ///
    /// Steam's own answer is "false if there was an error or no matching value
    /// was found", so `None` covers both. A key containing an interior nul can
    /// never match a C string, and is answered the same way instead of panicking.
    /// A key may map to several values; [`QueryResults::get_key_value_tag`] walks
    /// them by index.
    pub fn first_key_value_tag(&self, index: u32, key: &str) -> Option<String> {
        let key = CString::new(key).ok()?;
        let mut value = vec![0 as c_char; UGC_KEY_VALUE_TAG_MAX as usize];
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryFirstUGCKeyValueTag(
                self.ugc,
                self.handle,
                index,
                key.as_ptr(),
                value.as_mut_ptr(),
                UGC_KEY_VALUE_TAG_MAX,
            )
        };
        ok.then(|| steam_out_string(&value))
    }

    /// How many game-version (Steam branch) ranges the item at `index` declares.
    pub fn supported_game_version_count(&self, index: u32) -> u32 {
        unsafe { sys::SteamAPI_ISteamUGC_GetNumSupportedGameVersions(self.ugc, self.handle, index) }
    }

    /// One declared game-version range, as the two branch names Steam recorded.
    ///
    /// The ends stay exactly as Steam answers them, including an empty one: this
    /// SDK's header only defines the empty-string meaning for the *setter*
    /// (`SetRequiredGameVersions`), so reading one back as "unbounded" would be
    /// this wrapper's invention.
    pub fn supported_game_version(
        &self,
        index: u32,
        version_index: u32,
    ) -> Option<GameVersionRange> {
        let mut min_branch = vec![0 as c_char; UGC_GAME_BRANCH_MAX as usize];
        let mut max_branch = vec![0 as c_char; UGC_GAME_BRANCH_MAX as usize];
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetSupportedGameVersionData(
                self.ugc,
                self.handle,
                index,
                version_index,
                min_branch.as_mut_ptr(),
                max_branch.as_mut_ptr(),
                UGC_GAME_BRANCH_MAX,
            )
        };
        ok.then(|| GameVersionRange {
            min_branch: steam_out_string(&min_branch),
            max_branch: steam_out_string(&max_branch),
        })
    }

    /// The cursor for the next page of a deep-paged query, or `None` when Steam
    /// gave none (a page-size query, or the last page).
    ///
    /// Feed it to [`UGC::query_all_with_cursor`] to continue.
    pub fn next_cursor(&self) -> Option<&str> {
        self.next_cursor.as_deref()
    }

    /// Gets the developer-set metadata associated with the item at the specified index.
    ///
    /// This is returned as a vector of raw bytes.
    pub fn get_metadata(&self, index: u32) -> Option<Vec<u8>> {
        let mut metadata = [0 as c_char; sys::k_cchDeveloperMetadataMax as usize];

        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_GetQueryUGCMetadata(
                self.ugc,
                self.handle,
                index,
                metadata.as_mut_ptr(),
                sys::k_cchDeveloperMetadataMax,
            )
        };

        if ok {
            let metadata = unsafe { CStr::from_ptr(metadata.as_ptr()).to_bytes() };
            if metadata.is_empty() {
                None
            } else {
                Some(metadata.to_vec())
            }
        } else {
            None
        }
    }
}

/// Query result
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub published_file_id: PublishedFileId,
    pub creator_app_id: Option<AppId>,
    pub consumer_app_id: Option<AppId>,
    pub title: String,
    pub description: String,
    pub owner: SteamId,
    /// Time created in unix epoch seconds format
    pub time_created: u32,
    /// Time updated in unix epoch seconds format
    pub time_updated: u32,
    /// Time when the user added the published item to their list (not always applicable), provided in Unix epoch format (time since Jan 1st, 1970).
    pub time_added_to_user_list: u32,
    pub visibility: PublishedFileVisibility,
    pub banned: bool,
    pub accepted_for_use: bool,
    pub tags: Vec<String>,
    pub tags_truncated: bool,
    /// Original file name of the workshop item. Used in old games, like Total War: Shogun 2.
    pub file_name: String,
    pub file_type: FileType,
    pub file_size: u32,

    pub url: String,
    pub num_upvotes: u32,
    pub num_downvotes: u32,
    /// The bayesian average for up votes / total votes, between \[0,1\].
    pub score: f32,
    pub num_children: u32,
    // TODO: Add missing fields as needed
}
