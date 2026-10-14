//! The query builder: a request handle you configure, then `fetch`.
//!
//! Owns `UGCQueryHandleInvalid` as well, because that sentinel is what a failed query
//! creation answers with -- the accessor in `mod.rs` and every `create_*` here check it.

use super::*;

// TODO: should come from sys, but I don't think its generated.
#[allow(non_upper_case_globals)]
pub(super) const UGCQueryHandleInvalid: u64 = 0xffffffffffffffff;

pub struct QueryHandle {
    pub(super) ugc: *mut sys::ISteamUGC,
    pub(super) inner: Arc<Inner>,

    // Note: this is always filled except in `fetch`, where it must be taken
    // to prevent the handle from being dropped when this query is dropped.
    pub(super) handle: Option<sys::UGCQueryHandle_t>,
}

impl Drop for QueryHandle {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.as_mut() {
            unsafe {
                sys::SteamAPI_ISteamUGC_ReleaseQueryUGCRequest(self.ugc, *handle);
            }
        }
    }
}

impl QueryHandle {
    /// Excludes items with a specific tag.
    ///
    /// Panics if `tag` could not be converted to a `CString`.
    pub fn exclude_tag(self, tag: &str) -> Self {
        let cstr = CString::new(tag)
            .expect("String passed to exclude_tag could not be converted to a c string");
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddExcludedTag(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Only include items with a specific tag.
    ///
    /// Panics if `tag` could not be converted to a `CString`.
    pub fn require_tag(self, tag: &str) -> Self {
        let cstr = CString::new(tag)
            .expect("String passed to require_tag could not be converted to a c string");
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddRequiredTag(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Sets how to match tags added by `require_tag`. If `true`, then any tag may match. If `false`, all required tags must match.
    pub fn any_required(self, any: bool) -> Self {
        let ok =
            unsafe { sys::SteamAPI_ISteamUGC_SetMatchAnyTag(self.ugc, self.handle.unwrap(), any) };
        debug_assert!(ok);
        self
    }

    /// Sets the language to return the title and description in for the items on a pending UGC Query.
    ///
    /// Defaults to "english"
    pub fn language(self, language: &str) -> Self {
        let cstr = CString::new(language)
            .expect("String passed to language could not be converted to a c string");
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetLanguage(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether results will be returned from the cache for the specific period of time on a pending UGC Query.
    ///
    /// Age is in seconds.
    pub fn allow_cached_response(self, max_age_s: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetAllowCachedResponse(
                self.ugc,
                self.handle.unwrap(),
                max_age_s,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Include the full description in results
    pub fn include_long_desc(self, include: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnLongDescription(
                self.ugc,
                self.handle.unwrap(),
                include,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Include children in results
    pub fn include_children(self, include: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnChildren(self.ugc, self.handle.unwrap(), include)
        };
        debug_assert!(ok);
        self
    }

    /// Include metadata in results
    pub fn include_metadata(self, include: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnMetadata(self.ugc, self.handle.unwrap(), include)
        };
        debug_assert!(ok);
        self
    }

    /// Include additional previews in results
    pub fn include_additional_previews(self, include: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnAdditionalPreviews(
                self.ugc,
                self.handle.unwrap(),
                include,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Include key value tags in results
    pub fn include_key_value_tags(self, include: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnKeyValueTags(self.ugc, self.handle.unwrap(), include)
        };
        debug_assert!(ok);
        self
    }

    /// Adds a tag that must be present on all returned items.
    pub fn add_required_tag(self, tag: &str) -> Self {
        let cstr = CString::new(tag).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddRequiredTag(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Adds a tag that must not be present on any returned items.
    pub fn add_excluded_tag(self, tag: &str) -> Self {
        let cstr = CString::new(tag).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddExcludedTag(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to only return the IDs of the items.
    pub fn set_return_only_ids(self, return_only_ids: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnOnlyIDs(
                self.ugc,
                self.handle.unwrap(),
                return_only_ids,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to return key value tags with the items.
    pub fn set_return_key_value_tags(self, return_kv_tags: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnKeyValueTags(
                self.ugc,
                self.handle.unwrap(),
                return_kv_tags,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to return the full description of the items.
    pub fn set_return_long_description(self, return_long_desc: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnLongDescription(
                self.ugc,
                self.handle.unwrap(),
                return_long_desc,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to return metadata with the items.
    pub fn set_return_metadata(self, return_metadata: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnMetadata(
                self.ugc,
                self.handle.unwrap(),
                return_metadata,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to return children with the items.
    pub fn set_return_children(self, return_children: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnChildren(
                self.ugc,
                self.handle.unwrap(),
                return_children,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to return additional previews with the items.
    pub fn set_return_additional_previews(self, return_additional_previews: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnAdditionalPreviews(
                self.ugc,
                self.handle.unwrap(),
                return_additional_previews,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether to only return the total number of items.
    pub fn set_return_total_only(self, return_total_only: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnTotalOnly(
                self.ugc,
                self.handle.unwrap(),
                return_total_only,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets the language to return the title and description in.
    pub fn set_language(self, language: &str) -> Self {
        let cstr = CString::new(language).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetLanguage(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether results will be returned from the cache.
    pub fn set_allow_cached_response(self, max_age_seconds: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetAllowCachedResponse(
                self.ugc,
                self.handle.unwrap(),
                max_age_seconds,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets a filter for the cloud file name.
    pub fn set_cloud_file_name_filter(self, file_name: &str) -> Self {
        let cstr = CString::new(file_name).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetCloudFileNameFilter(
                self.ugc,
                self.handle.unwrap(),
                cstr.as_ptr(),
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sets whether any of the required tags are sufficient for an item to be returned.
    pub fn set_match_any_tag(self, match_any_tag: bool) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetMatchAnyTag(self.ugc, self.handle.unwrap(), match_any_tag)
        };
        debug_assert!(ok);
        self
    }

    /// Sets the full-text search string.
    pub fn set_search_text(self, search_text: &str) -> Self {
        let cstr = CString::new(search_text).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetSearchText(self.ugc, self.handle.unwrap(), cstr.as_ptr())
        };
        debug_assert!(ok);
        self
    }

    /// Sets the number of days to consider for trending items.
    pub fn set_ranked_by_trend_days(self, days: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetRankedByTrendDays(self.ugc, self.handle.unwrap(), days)
        };
        debug_assert!(ok);
        self
    }

    /// Adds a required key-value tag that must be present on all returned items.
    pub fn add_required_key_value_tag(self, key: &str, value: &str) -> Self {
        let key_cstr = CString::new(key).unwrap();
        let value_cstr = CString::new(value).unwrap();
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddRequiredKeyValueTag(
                self.ugc,
                self.handle.unwrap(),
                key_cstr.as_ptr(),
                value_cstr.as_ptr(),
            )
        };
        debug_assert!(ok);
        self
    }

    /// Requires returned items to carry at least one tag from this group: Steam
    /// matches *any* of the tags in the group, where [`QueryHandle::require_tag`]
    /// demands one specific tag.
    ///
    /// A tag with an interior nul is refused as `InvalidParameter` instead of
    /// panicking, and none of the group is applied in that case.
    pub fn add_required_tag_group<S: AsRef<str>>(self, groups: &[S]) -> Result<Self, SteamError> {
        let strings = tag_group_strings(groups)?;
        let mut pointers: Vec<*const c_char> = strings.iter().map(|s| s.as_ptr()).collect();
        let raw = sys::SteamParamStringArray_t {
            m_nNumStrings: pointers.len() as i32,
            m_ppStrings: pointers.as_mut_ptr(),
        };
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_AddRequiredTagGroup(self.ugc, self.handle.unwrap(), &raw)
        };
        if ok {
            Ok(self)
        } else {
            Err(SteamError::InvalidParameter)
        }
    }

    /// Asks Steam to include playtime statistics for the last `days` days in the
    /// results ([`QueryResults::statistic`] then has values for them).
    #[must_use]
    pub fn set_return_playtime_stats(self, days: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetReturnPlaytimeStats(self.ugc, self.handle.unwrap(), days)
        };
        debug_assert!(ok);
        self
    }

    /// Restricts results to items created between two Unix timestamps.
    ///
    /// The bounds go to Steam as given; a reversed range is Steam's to answer
    /// (with nothing), not something this wrapper silently swaps.
    #[must_use]
    pub fn set_time_created_date_range(self, start: u32, end: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetTimeCreatedDateRange(
                self.ugc,
                self.handle.unwrap(),
                start,
                end,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Restricts results to items updated between two Unix timestamps.
    ///
    /// See [`QueryHandle::set_time_created_date_range`] for why the bounds are
    /// passed through unexamined.
    #[must_use]
    pub fn set_time_updated_date_range(self, start: u32, end: u32) -> Self {
        let ok = unsafe {
            sys::SteamAPI_ISteamUGC_SetTimeUpdatedDateRange(
                self.ugc,
                self.handle.unwrap(),
                start,
                end,
            )
        };
        debug_assert!(ok);
        self
    }

    /// Sends the query to Steam and calls the provided callback with the results when completed.
    ///
    /// When the query was created for deep paging, the answer carries
    /// [`QueryResults::next_cursor`], which is what [`UGC::query_all_with_cursor`]
    /// continues from.
    pub fn fetch<F>(mut self, cb: F)
    where
        F: for<'a> FnOnce(Result<QueryResults<'a>, SteamError>) + 'static + Send,
    {
        let ugc = self.ugc;
        let inner = Arc::clone(&self.inner);
        let handle = self.handle.take().unwrap();
        mem::drop(self);

        unsafe {
            let api_call = sys::SteamAPI_ISteamUGC_SendQueryUGCRequest(ugc, handle);
            register_call_result::<sys::SteamUGCQueryCompleted_t, _>(
                &inner,
                api_call,
                move |v, io_error| {
                    let ugc = sys::SteamAPI_SteamUGC_v021();
                    if io_error {
                        sys::SteamAPI_ISteamUGC_ReleaseQueryUGCRequest(ugc, handle);
                        cb(Err(SteamError::IOFailure));
                        return;
                    } else if let Err(error) = to_steam_result(v.m_eResult) {
                        sys::SteamAPI_ISteamUGC_ReleaseQueryUGCRequest(ugc, handle);
                        cb(Err(error));
                        return;
                    }

                    let result = QueryResults {
                        ugc,
                        handle,
                        num_results_returned: v.m_unNumResultsReturned,
                        num_results_total: v.m_unTotalMatchingResults,
                        was_cached: v.m_bCachedData,
                        next_cursor: next_cursor_from(&v.m_rgchNextCursor),
                        _phantom: Default::default(),
                    };
                    cb(Ok(result));
                },
            );
        }
    }

    /// Runs the query, only fetching the total number of results.
    pub fn fetch_total<F>(self, cb: F)
    where
        F: Fn(Result<u32, SteamError>) + 'static + Send,
    {
        unsafe {
            let ok =
                sys::SteamAPI_ISteamUGC_SetReturnTotalOnly(self.ugc, self.handle.unwrap(), true);
            debug_assert!(ok);
        }

        self.fetch(move |res| cb(res.map(|qr| qr.total_results())))
    }

    /// Runs the query, only fetching the IDs.
    pub fn fetch_ids<F>(self, cb: F)
    where
        F: Fn(Result<Vec<PublishedFileId>, SteamError>) + 'static + Send,
    {
        unsafe {
            let ok = sys::SteamAPI_ISteamUGC_SetReturnOnlyIDs(self.ugc, self.handle.unwrap(), true);
            debug_assert!(ok);
        }

        self.fetch(move |res| {
            cb(res.map(|qr| {
                qr.iter()
                    .filter_map(|v| v.map(|v| PublishedFileId(v.published_file_id.0)))
                    .collect::<Vec<_>>()
            }))
        })
    }
}
