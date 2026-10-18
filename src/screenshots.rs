use std::path::Path;

pub use sys::ScreenshotHandle;

use super::*;
use crate::utils::try_c_string;

/// Access to the steam screenshots interface
pub struct Screenshots {
    pub(crate) screenshots: *mut sys::ISteamScreenshots,
    pub(crate) _inner: Arc<Inner>,
}

impl Screenshots {
    /// Toggles whether the overlay handles screenshots when the user presses the screenshot hotkey, or if the game handles them.
    ///
    /// Hooking is disabled by default, and only ever enabled if you do so with this function.
    ///
    /// If the hooking is enabled, then the [`ScreenshotRequested`] callback will be sent if the user presses the hotkey or when [`Self::trigger_screenshot`] is called,
    /// and then the game is expected to call `WriteScreenshot` or [`Self::add_screenshot_to_library`] in response.
    ///
    /// You can check if hooking is enabled with [`Self::is_screenshots_hooked`].
    pub fn hook_screenshots(&self, hook: bool) {
        unsafe {
            sys::SteamAPI_ISteamScreenshots_HookScreenshots(self.screenshots, hook);
        }
    }

    /// Checks if the app is hooking screenshots, or if the Steam Overlay is handling them.
    ///
    /// This can be toggled with [`Self::hook_screenshots`].
    ///
    /// Returns
    /// - `true` if the game is hooking screenshots and is expected to handle them; otherwise, `false`.
    pub fn is_screenshots_hooked(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamScreenshots_IsScreenshotsHooked(self.screenshots) }
    }

    /// Either causes the Steam Overlay to take a screenshot, or tells your screenshot manager that a screenshot needs to be taken.
    /// Depending on the value of [`Self::is_screenshots_hooked`].
    ///
    /// - Triggers a [`ScreenshotRequested`] callback.
    /// - Triggers a [`ScreenshotReady`] callback.
    /// - Only causes [`ScreenshotRequested`] if hooking has been enabled with [`Self::hook_screenshots`].
    /// - Otherwise [`ScreenshotReady`] will be called when the screenshot has been saved and added to the library.
    pub fn trigger_screenshot(&self) {
        unsafe {
            sys::SteamAPI_ISteamScreenshots_TriggerScreenshot(self.screenshots);
        }
    }

    /// Adds a screenshot to the user's Steam screenshot library from disk.
    ///
    /// Triggers a [`ScreenshotReady`] callback.
    /// The handle to this screenshot that is valid for the duration of the game process and can be used to apply tags.
    ///
    /// This call is asynchronous, a [`ScreenshotReady`] callback will be sent when the screenshot has finished writing to disk.
    pub fn add_screenshot_to_library(
        &self,
        filename: &Path,
        thumbnail_filename: Option<&Path>,
        width: i32,
        height: i32,
    ) -> Result<ScreenshotHandle, ScreenshotLibraryAddError> {
        let filename =
            path_to_absolute_cstring(filename).ok_or(ScreenshotLibraryAddError::InvalidPath)?;

        let thumbnail_filename = if let Some(thumbnail_filename) = thumbnail_filename {
            Some(
                path_to_absolute_cstring(thumbnail_filename)
                    .ok_or(ScreenshotLibraryAddError::InvalidPath)?,
            )
        } else {
            None
        };

        unsafe {
            let handle = sys::SteamAPI_ISteamScreenshots_AddScreenshotToLibrary(
                self.screenshots,
                filename.as_ptr(),
                thumbnail_filename.map_or(std::ptr::null(), |s| s.as_ptr()),
                width,
                height,
            );

            if handle != sys::INVALID_SCREENSHOT_HANDLE {
                Ok(handle)
            } else {
                Err(ScreenshotLibraryAddError::SavingFailed)
            }
        }
    }

    /// Adds a VR screenshot to the user's library from disk.
    ///
    /// `kind` says what the two files actually hold, because Steam presents
    /// mono, stereo and panorama captures differently; the paths must be
    /// absolute, which is what the existing library calls already require.
    pub fn add_vr_screenshot_to_library(
        &self,
        kind: VRScreenshotKind,
        filename: &Path,
        vr_filename: &Path,
    ) -> Result<ScreenshotHandle, ScreenshotLibraryAddError> {
        let filename =
            path_to_absolute_cstring(filename).ok_or(ScreenshotLibraryAddError::InvalidPath)?;
        let vr_filename =
            path_to_absolute_cstring(vr_filename).ok_or(ScreenshotLibraryAddError::InvalidPath)?;

        unsafe {
            let handle = sys::SteamAPI_ISteamScreenshots_AddVRScreenshotToLibrary(
                self.screenshots,
                kind.into(),
                filename.as_ptr(),
                vr_filename.as_ptr(),
            );
            if handle != sys::INVALID_SCREENSHOT_HANDLE {
                Ok(handle)
            } else {
                Err(ScreenshotLibraryAddError::SavingFailed)
            }
        }
    }

    /// Writes raw RGB data straight into the user's screenshot library.
    ///
    /// This is the call a game makes in response to [`ScreenshotRequested`] when
    /// screenshots are hooked with [`Screenshots::hook_screenshots`]. Steam copies
    /// the buffer before returning, so `rgb` does not have to outlive the call.
    /// The byte count Steam takes is a `u32`, so a buffer it cannot express is
    /// rejected here rather than truncated.
    pub fn write_screenshot(
        &self,
        rgb: &[u8],
        width: i32,
        height: i32,
    ) -> Result<ScreenshotHandle, SteamError> {
        let cub_rgb = screenshot_byte_count(rgb.len())?;
        let handle = unsafe {
            sys::SteamAPI_ISteamScreenshots_WriteScreenshot(
                self.screenshots,
                // Steam only reads `cub_rgb` bytes, but it takes a non-const
                // pointer; an empty buffer gets an aligned address that is
                // never read rather than a null one.
                if rgb.is_empty() {
                    std::ptr::NonNull::<u8>::dangling().as_ptr() as *mut c_void
                } else {
                    rgb.as_ptr() as *mut c_void
                },
                cub_rgb,
                width,
                height,
            )
        };
        if handle == sys::INVALID_SCREENSHOT_HANDLE {
            Err(SteamError::IOFailure)
        } else {
            Ok(handle)
        }
    }

    /// Labels where in the game a screenshot was taken, shown to the user.
    ///
    /// Returns `Ok(false)` when Steam rejected the handle (for example a handle
    /// from an earlier process), and `Err` only when the location could not be
    /// handed to Steam as a string.
    pub fn set_location(
        &self,
        screenshot: ScreenshotHandle,
        location: &str,
    ) -> Result<bool, SteamError> {
        let location = CString::new(location).map_err(|_| SteamError::InvalidParameter)?;
        Ok(unsafe {
            sys::SteamAPI_ISteamScreenshots_SetLocation(
                self.screenshots,
                screenshot,
                location.as_ptr(),
            )
        })
    }

    /// Tags a screenshot with another player, as in "screenshot taken with X".
    pub fn tag_user(&self, screenshot: ScreenshotHandle, user: SteamId) -> bool {
        unsafe { sys::SteamAPI_ISteamScreenshots_TagUser(self.screenshots, screenshot, user.raw()) }
    }

    /// Tags a screenshot with a workshop item so it shows up on that item's page.
    pub fn tag_published_file(&self, screenshot: ScreenshotHandle, file: PublishedFileId) -> bool {
        unsafe {
            sys::SteamAPI_ISteamScreenshots_TagPublishedFile(self.screenshots, screenshot, file.0)
        }
    }
}

/// Steam sizes a raw screenshot with a `u32`; a buffer it cannot express is a
/// bad argument, not something to clip to fit.
fn screenshot_byte_count(len: usize) -> Result<u32, SteamError> {
    u32::try_from(len).map_err(|_| SteamError::InvalidParameter)
}

/// What kind of VR capture a pair of screenshot files holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VRScreenshotKind {
    None,
    Mono,
    Stereo,
    MonoCubemap,
    MonoPanorama,
    StereoPanorama,
}

impl From<VRScreenshotKind> for sys::EVRScreenshotType {
    fn from(kind: VRScreenshotKind) -> Self {
        match kind {
            VRScreenshotKind::None => sys::EVRScreenshotType::k_EVRScreenshotType_None,
            VRScreenshotKind::Mono => sys::EVRScreenshotType::k_EVRScreenshotType_Mono,
            VRScreenshotKind::Stereo => sys::EVRScreenshotType::k_EVRScreenshotType_Stereo,
            VRScreenshotKind::MonoCubemap => {
                sys::EVRScreenshotType::k_EVRScreenshotType_MonoCubemap
            }
            VRScreenshotKind::MonoPanorama => {
                sys::EVRScreenshotType::k_EVRScreenshotType_MonoPanorama
            }
            VRScreenshotKind::StereoPanorama => {
                sys::EVRScreenshotType::k_EVRScreenshotType_StereoPanorama
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vr_kinds_map_onto_distinct_steam_variants() {
        let kinds = [
            VRScreenshotKind::None,
            VRScreenshotKind::Mono,
            VRScreenshotKind::Stereo,
            VRScreenshotKind::MonoCubemap,
            VRScreenshotKind::MonoPanorama,
            VRScreenshotKind::StereoPanorama,
        ];
        let mapped: Vec<sys::EVRScreenshotType> = kinds.iter().copied().map(Into::into).collect();
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(
                    mapped[i], mapped[j],
                    "{:?}/{:?} collide",
                    kinds[i], kinds[j]
                );
            }
        }
    }

    #[test]
    fn screenshot_byte_counts_steam_cannot_express_are_rejected() {
        // Same boundary the wrapper uses: `write_screenshot` calls this before
        // touching the FFI, so changing the guard fails here.
        assert_eq!(screenshot_byte_count(0).unwrap(), 0);
        assert_eq!(
            screenshot_byte_count(usize::try_from(u32::MAX).unwrap()).unwrap(),
            u32::MAX
        );
        for too_big in [
            usize::try_from(u64::from(u32::MAX) + 1).unwrap(),
            usize::MAX,
        ] {
            assert!(
                matches!(
                    screenshot_byte_count(too_big),
                    Err(SteamError::InvalidParameter)
                ),
                "a {too_big} byte buffer must not be sent as a u32 count"
            );
        }
    }
}

#[derive(Debug, Error)]
pub enum ScreenshotLibraryAddError {
    /// Steam failed to save the file for an unspecified reason.
    #[error("The screenshot file could not be saved")]
    SavingFailed,
    /// One of the paths provided was invalid.
    #[error("Invalid path")]
    InvalidPath,
}

/// A screenshot has been requested by the user from the Steam screenshot hotkey.
/// This will only be called if [`Screenshots::hook_screenshots`] has been enabled, in which case Steam will not take the screenshot itself.
#[derive(Clone, Debug)]
pub struct ScreenshotRequested;

impl_callback!(_cb: ScreenshotRequested_t => ScreenshotRequested {
    Self
});

#[derive(Clone, Debug, Error)]
pub enum ScreenshotReadyError {
    /// The screenshot could not be loaded or parsed.
    #[error("The screenshot could not be loaded or parsed")]
    Fail,
    /// The screenshot could not be saved to the disk.
    #[error("The screenshot could not be saved to the disk")]
    IoFailure,
}

/// A screenshot successfully written or otherwise added to the library and can now be tagged.
#[derive(Clone, Debug)]
pub struct ScreenshotReady {
    /// The screenshot handle that has been written.
    pub local_handle: Result<ScreenshotHandle, ScreenshotReadyError>,
}

impl_callback!(cb: ScreenshotReady_t => ScreenshotReady {
    let local_handle = match to_steam_result(cb.m_eResult) {
        Ok(_) => Ok(cb.m_hLocal),
        Err(SteamError::IOFailure) => Err(ScreenshotReadyError::IoFailure),
        Err(_) => Err(ScreenshotReadyError::Fail),
    };

    Self { local_handle }
});

fn path_to_absolute_cstring(filename: &Path) -> Option<CString> {
    let filename = filename.canonicalize().ok()?;
    try_c_string(filename.to_str()?)
}
