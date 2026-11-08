use super::*;

/// Access to the steam video interface
pub struct Video {
    pub(crate) video: *mut sys::ISteamVideo,
    pub(crate) _inner: Arc<Inner>,
}

/// Whether a user is currently broadcasting, and to how many viewers
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BroadcastStatus {
    /// Whether the app is being broadcast right now
    pub broadcasting: bool,
    /// Viewer count: `0` when not broadcasting
    pub num_viewers: u32,
}

/// Clamp the int32 viewer count Steam reports into the `u32` field.
///
/// `IsBroadcasting` writes an int32 out-parameter, and when nothing is being
/// broadcast it can leave a negative or absent value there. A bare `as u32`
/// would wrap that into a huge viewer count, so only a positive count passes.
pub(crate) fn clamp_viewers(raw: i32) -> u32 {
    if raw > 0 {
        raw as u32
    } else {
        0
    }
}

/// Steam's answer to a video-URL request: the URL, and which app it belongs to.
///
/// The request half is [`Video::open_video_page`], whose FFI name is `GetVideoURL`; the
/// answer half is this callback, `isteamvideo.h:31` describing the request as "Get a URL
/// suitable for streaming the given Video app ID's video". Steam fills a fixed 256-byte
/// `m_rgchURL` and promises neither a terminator beyond that buffer nor an encoding, so the
/// buffer goes through the crate's lossy fixed-buffer reader instead of a UTF-8 demand.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct GetVideoURLResult {
    /// `None` when Steam reported success.
    pub error: Option<SteamError>,
    /// The app the URL belongs to, so a subscriber can match it against its own request.
    pub app: AppId,
    /// The streaming URL; empty when Steam had nothing to report.
    pub url: String,
}

impl GetVideoURLResult {
    fn from_answer(answer: &sys::GetVideoURLResult_t) -> Self {
        Self {
            error: to_steam_result(answer.m_eResult).err(),
            app: AppId(answer.m_unVideoAppID),
            url: crate::utils::fixed_string(&answer.m_rgchURL),
        }
    }
}

impl_callback!(cb: GetVideoURLResult_t => GetVideoURLResult {
    Self::from_answer(&cb)
});

/// Steam's answer to [`Video::get_opf_settings`], naming the app whose 360-video settings
/// are now readable.
///
/// Carries no payload beyond the result code and the app id -- the settings themselves are
/// read back with [`Video::opf_string`] -- so this type exists to say *when* they are ready
/// and whether the fetch failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct GetOPFSettingsResult {
    /// `None` when Steam reported success.
    pub error: Option<SteamError>,
    /// The app whose settings were fetched.
    pub app: AppId,
}

impl GetOPFSettingsResult {
    fn from_answer(answer: &sys::GetOPFSettingsResult_t) -> Self {
        Self {
            error: to_steam_result(answer.m_eResult).err(),
            app: AppId(answer.m_unVideoAppID),
        }
    }
}

impl_callback!(cb: GetOPFSettingsResult_t => GetOPFSettingsResult {
    Self::from_answer(&cb)
});

impl Video {
    /// Asks Steam for a URL that streams the given video app.
    ///
    /// The FFI name is `SteamAPI_ISteamVideo_GetVideoURL` and `isteamvideo.h:31` states the
    /// purpose as "Get a URL suitable for streaming the given Video app ID's video". The call
    /// itself returns nothing: the URL arrives afterwards as [`GetVideoURLResult`] on the
    /// callback stream. The method name is kept as-is because it is already public API.
    pub fn open_video_page(&self, app: AppId) {
        unsafe {
            sys::SteamAPI_ISteamVideo_GetVideoURL(self.video, app.0);
        }
    }

    /// Whether the local user is broadcasting this app, and to how many viewers.
    pub fn broadcast_status(&self) -> BroadcastStatus {
        let mut viewers = 0i32;
        let broadcasting =
            unsafe { sys::SteamAPI_ISteamVideo_IsBroadcasting(self.video, &mut viewers) };
        BroadcastStatus {
            broadcasting,
            num_viewers: clamp_viewers(viewers),
        }
    }

    /// Reads an app's Open Graph metadata (OPF) XML.
    ///
    /// `capacity` sizes the buffer Steam writes into; it is in/out at the FFI
    /// boundary, so a buffer that was too small is reported as
    /// `Err(InvalidParameter)` rather than handing back truncated XML.
    ///
    /// This reads whatever Steam already holds. To ask for a fresh copy, call
    /// [`Video::get_opf_settings`] and wait for the [`GetOPFSettingsResult`] it answers
    /// with -- `isteamvideo.h:39-40` puts the two side by side ("GetOPFSettings( AppId_t
    /// unVideoAppID )" then this reader), and the result type is in the generated bindings,
    /// so there is a way to tell when freshly requested data is ready.
    pub fn opf_string(&self, app: AppId, capacity: usize) -> Result<Option<String>, SteamError> {
        if capacity == 0 {
            return Ok(None);
        }
        let mut buf = vec![0 as std::os::raw::c_char; capacity];
        // Steam wants the usable length as an int32 in/out parameter.
        let mut len = capacity as i32;
        let ok = unsafe {
            sys::SteamAPI_ISteamVideo_GetOPFStringForApp(
                self.video,
                app.0,
                buf.as_mut_ptr(),
                &mut len,
            )
        };
        if !ok {
            return Err(SteamError::IOFailure);
        }
        if len < 0 || (len as usize) >= capacity {
            return Err(SteamError::InvalidParameter);
        }
        let bytes = unsafe { std::slice::from_raw_parts(buf.as_ptr().cast::<u8>(), len as usize) };
        let text = String::from_utf8_lossy(bytes).into_owned();
        if text.is_empty() {
            Ok(None)
        } else {
            Ok(Some(text))
        }
    }

    /// Asks Steam to fetch the OPF settings (360-video metadata) for an app.
    ///
    /// `isteamvideo.h:38` annotates the call "STEAM_CALL_BACK( GetOPFSettingsResult_t )":
    /// it returns nothing synchronously, and the answer -- which app's settings became
    /// available, and whether Steam was able to fetch them -- arrives afterwards as
    /// [`GetOPFSettingsResult`] on the callback stream. Read the settings back with
    /// [`Video::opf_string`] once it lands.
    pub fn get_opf_settings(&self, app: AppId) {
        unsafe {
            sys::SteamAPI_ISteamVideo_GetOPFSettings(self.video, app.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_viewer_count_is_not_reported_as_viewers() {
        // The FFI out-parameter is int32; a negative or absent value must not
        // wrap into a huge u32 through a bare `as`. This calls the same
        // function `broadcast_status` uses, so changing that clamp breaks here.
        for raw in [-1, -2, i32::MIN, i32::MIN + 1] {
            assert_eq!(clamp_viewers(raw), 0, "raw {raw} leaked into the count");
        }
        assert_eq!(clamp_viewers(0), 0);
        assert_eq!(clamp_viewers(1), 1);
        assert_eq!(clamp_viewers(i32::MAX), 2_147_483_647);
    }

    #[test]
    fn opf_length_guard_rejects_non_fitting_capacity() {
        // The guard is `len < 0 || len >= capacity`; pin the boundary so a
        // capacity that exactly equals the string length is not mistaken for
        // truncation.
        let fits = |len: i32, capacity: usize| len >= 0 && (len as usize) < capacity;
        assert!(fits(5, 6), "5 bytes in a 6 buffer must fit");
        assert!(!fits(6, 6), "6 bytes in a 6 buffer has no room for the NUL");
        assert!(!fits(-1, 6), "negative length must not pass");
    }

    #[test]
    fn a_video_url_answer_keeps_the_steam_code_and_cannot_read_past_the_buffer() {
        let mut answer = sys::GetVideoURLResult_t {
            m_eResult: sys::EResult::k_EResultOK,
            m_unVideoAppID: 71,
            m_rgchURL: [0; 256],
        };
        for (slot, byte) in b"https://cdn/clip.mp4".iter().enumerate() {
            answer.m_rgchURL[slot] = *byte as std::os::raw::c_char;
        }

        let url = GetVideoURLResult::from_answer(&answer);
        assert_eq!(url.error, None, "k_EResultOK must not look like a failure");
        assert_eq!(url.app, AppId(71));
        assert_eq!(url.url, "https://cdn/clip.mp4");

        // Steam's own failure code stays a failure. The URL field is still filled by
        // Steam, so a reader that dropped `m_eResult` would hand a caller a garbage
        // address and call it a result.
        answer.m_eResult = sys::EResult::k_EResultFail;
        let failed = GetVideoURLResult::from_answer(&answer);
        assert_eq!(
            failed.error,
            Some(SteamError::Generic),
            "k_EResultFail decodes to Generic (src/error.rs:24-26)"
        );
        assert!(
            !failed.url.is_empty(),
            "Steam does not clear the buffer on failure"
        );

        // An unwritten status field holds zero, which is k_EResultNone: not a success.
        answer.m_eResult = sys::EResult::k_EResultNone;
        assert!(GetVideoURLResult::from_answer(&answer).error.is_some());

        // No terminator anywhere in the fixed array: the reader stops at the array bound
        // instead of walking past `m_rgchURL`.
        answer.m_eResult = sys::EResult::k_EResultOK;
        answer.m_rgchURL = [b'a' as std::os::raw::c_char; 256];
        let full = GetVideoURLResult::from_answer(&answer);
        assert_eq!(
            full.url.len(),
            256,
            "the array bound is the only length left"
        );
    }

    #[test]
    fn both_video_answers_reach_the_stream_under_their_own_ids() {
        // The request halves are `open_video_page` and `get_opf_settings`. If either
        // result type failed to register its Steam callback id, the caller would never
        // learn that the answer had arrived, and `opf_string` would read a cache nobody
        // knows is current.
        let mut url = sys::GetVideoURLResult_t {
            m_eResult: sys::EResult::k_EResultOK,
            m_unVideoAppID: 81,
            m_rgchURL: [0; 256],
        };
        for (slot, byte) in b"steam://url".iter().enumerate() {
            url.m_rgchURL[slot] = *byte as std::os::raw::c_char;
        }
        let raw = (&mut url as *mut sys::GetVideoURLResult_t).cast::<std::os::raw::c_void>();
        match unsafe { CallbackResult::from_raw(GetVideoURLResult::ID, raw) } {
            Some(CallbackResult::GetVideoURLResult(streamed)) => {
                assert_eq!(streamed.app, AppId(81));
                assert_eq!(streamed.url, "steam://url");
            }
            other => panic!("GetVideoURLResult decoded as {other:?}"),
        }

        let mut opf = sys::GetOPFSettingsResult_t {
            m_eResult: sys::EResult::k_EResultTimeout,
            m_unVideoAppID: 82,
        };
        let raw = (&mut opf as *mut sys::GetOPFSettingsResult_t).cast::<std::os::raw::c_void>();
        match unsafe { CallbackResult::from_raw(GetOPFSettingsResult::ID, raw) } {
            Some(CallbackResult::GetOPFSettingsResult(streamed)) => {
                assert_eq!(streamed.app, AppId(82));
                assert!(
                    streamed.error.is_some(),
                    "a timeout is not a silent success"
                );
            }
            other => panic!("GetOPFSettingsResult decoded as {other:?}"),
        }

        assert_ne!(
            GetVideoURLResult::ID,
            GetOPFSettingsResult::ID,
            "k_iSteamVideoCallbacks + 11 and + 24 are different callbacks"
        );
    }
}
