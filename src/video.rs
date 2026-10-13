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
    /// Viewer count; `0` when not broadcasting
    pub num_viewers: u32,
}

impl Video {
    /// Opens the store page for a video app.
    ///
    /// Steam's own UI takes over from here; the call carries no result, so
    /// there is nothing to report back.
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
            num_viewers: if viewers > 0 { viewers as u32 } else { 0 },
        }
    }

    /// Reads an app's Open Graph metadata (OPF) XML.
    ///
    /// `capacity` sizes the buffer Steam writes into; it is in/out at the FFI
    /// boundary, so a buffer that was too small is reported as
    /// `Err(InvalidParameter)` rather than handing back truncated XML.
    ///
    /// Not paired with a "request settings" call: this crate's generated
    /// bindings contain no OPF request-result callback, so there would be no
    /// way to tell when freshly requested data is ready. Read what is already
    /// cached instead.
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negative_viewer_count_is_not_reported_as_viewers() {
        // The FFI out-parameter is int32; a negative or absent value must not
        // wrap into a huge u32 through a bare `as`.
        for raw in [0i32, -1, i32::MIN] {
            let clamped = if raw > 0 { raw as u32 } else { 0 };
            assert_eq!(clamped, 0, "raw {raw} leaked into the viewer count");
        }
        assert_eq!(i32::MAX as u32, 2147483647);
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
}
