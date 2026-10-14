use super::*;
use std::time::Duration;

/// An id for a steam app/game
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct AppId(pub u32);
impl From<u32> for AppId {
    fn from(id: u32) -> Self {
        AppId(id)
    }
}

/// Access to the steam apps interface
pub struct Apps {
    pub(crate) apps: *mut sys::ISteamApps,
    pub(crate) _inner: Arc<Inner>,
}

impl Apps {
    /// Returns whether the user currently has the app with the given
    /// ID currently installed.
    ///
    /// This does not mean the user owns the game.
    pub fn is_app_installed(&self, app_id: AppId) -> bool {
        unsafe { sys::SteamAPI_ISteamApps_BIsAppInstalled(self.apps, app_id.0) }
    }

    /// Returns whether the user owns the specific dlc and has it
    /// installed.
    pub fn is_dlc_installed(&self, app_id: AppId) -> bool {
        unsafe { sys::SteamAPI_ISteamApps_BIsDlcInstalled(self.apps, app_id.0) }
    }

    /// Returns whether the user is subscribed to the app with the given
    /// ID.
    ///
    /// This should only be used to check ownership of a game related to
    /// yours (e.g. demo).
    pub fn is_subscribed_app(&self, app_id: AppId) -> bool {
        unsafe { sys::SteamAPI_ISteamApps_BIsSubscribedApp(self.apps, app_id.0) }
    }

    /// Returns whether the user is subscribed via a free weekend
    pub fn is_subscribed_from_free_weekend(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamApps_BIsSubscribedFromFreeWeekend(self.apps) }
    }

    /// Returns whether the user has a VAC ban on their account.
    pub fn is_vac_banned(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamApps_BIsVACBanned(self.apps) }
    }

    /// Returns whether the license for the current app ID
    /// is for cyber cafes.
    pub fn is_cybercafe(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamApps_BIsCybercafe(self.apps) }
    }

    /// Returns whether the license for the current app ID
    /// provides low violence depots.
    pub fn is_low_violence(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamApps_BIsLowViolence(self.apps) }
    }

    /// Returns whether the user is subscribed to the current app ID
    pub fn is_subscribed(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamApps_BIsSubscribed(self.apps) }
    }

    /// Returns the build id of this app.
    pub fn app_build_id(&self) -> i32 {
        unsafe { sys::SteamAPI_ISteamApps_GetAppBuildId(self.apps) as i32 }
    }

    /// Returns the installation folder of the app with the given ID.
    ///
    /// This works even if the app isn't installed, returning where it
    /// would be installed in the default location.
    pub fn app_install_dir(&self, app_id: AppId) -> String {
        unsafe {
            let mut buffer = vec![0; 2048];
            sys::SteamAPI_ISteamApps_GetAppInstallDir(
                self.apps,
                app_id.0,
                buffer.as_mut_ptr(),
                buffer.len() as u32,
            );
            let path = CStr::from_ptr(buffer.as_ptr());
            path.to_string_lossy().into_owned()
        }
    }

    /// Returns the steam id of the original owner of the app.
    ///
    /// Differs from the current user if the app is borrowed.
    pub fn app_owner(&self) -> SteamId {
        unsafe { SteamId(sys::SteamAPI_ISteamApps_GetAppOwner(self.apps)) }
    }

    /// Returns a list of languages that the current app supports.
    pub fn available_game_languages(&self) -> Vec<String> {
        unsafe {
            let langs = sys::SteamAPI_ISteamApps_GetAvailableGameLanguages(self.apps);
            let langs = CStr::from_ptr(langs);
            let langs = langs.to_string_lossy();
            langs.split(',').map(|v| v.to_owned()).collect()
        }
    }

    /// Returns the language the user has set for the current game.
    ///
    /// If the language hasn't been set this returns the language
    /// used for the steam UI.
    pub fn current_game_language(&self) -> String {
        unsafe {
            let lang = sys::SteamAPI_ISteamApps_GetCurrentGameLanguage(self.apps);
            let lang = CStr::from_ptr(lang);
            lang.to_string_lossy().into_owned()
        }
    }

    /// Returns the current beta name if any.
    ///
    /// If the user isn't playing on a beta branch then this
    /// returns `None`
    pub fn current_beta_name(&self) -> Option<String> {
        unsafe {
            let mut buffer = vec![0; 256];
            if sys::SteamAPI_ISteamApps_GetCurrentBetaName(
                self.apps,
                buffer.as_mut_ptr(),
                buffer.len() as _,
            ) {
                let path = CStr::from_ptr(buffer.as_ptr());
                Some(path.to_string_lossy().into_owned())
            } else {
                None
            }
        }
    }

    /// Returns the command line if the game was launched via Steam URL
    ///
    /// If the game was not launched through Steam URL, this returns an empty string.
    ///
    /// See [Steam API](https://partner.steamgames.com/doc/api/ISteamApps#GetLaunchCommandLine)
    pub fn launch_command_line(&self) -> String {
        unsafe {
            let mut buffer = vec![0; 256];
            let _bytes = sys::SteamAPI_ISteamApps_GetLaunchCommandLine(
                self.apps,
                buffer.as_mut_ptr(),
                buffer.len() as _,
            );
            let command_line = CStr::from_ptr(buffer.as_ptr());
            command_line.to_string_lossy().into_owned()
        }
    }

    /// Gets the associated launch parameter if the game is run via `steam://run/<appid>/?param1=value1;param2=value2;param3=value3` etc.
    ///
    /// Parameter names starting with the character '@' are reserved for internal use and will always return an empty string.
    /// Parameter names starting with an underscore '_' are reserved for steam features -- they can be queried by the game, but it is advised that you don't use param names beginning with an underscore for your own features.
    ///
    /// See [Steam API](https://partner.steamgames.com/doc/api/ISteamApps#GetLaunchQueryParam)
    pub fn launch_query_param(&self, key: &str) -> String {
        let key = CString::new(key).unwrap();
        unsafe {
            let value = sys::SteamAPI_ISteamApps_GetLaunchQueryParam(self.apps, key.as_ptr());
            let value = CStr::from_ptr(value);
            value.to_string_lossy().into_owned()
        }
    }

    /// Number of DLC entries Steam reports for this app, including ones the
    /// user does not own.
    pub fn dlc_count(&self) -> i32 {
        unsafe { sys::SteamAPI_ISteamApps_GetDLCCount(self.apps) }
    }

    /// Every DLC Steam lists for this app.
    ///
    /// `available` is whether the user owns it; entries are read by index, so
    /// a DLC that appears or disappears mid-listing can shift the rest.
    pub fn dlcs(&self) -> Vec<DlcInfo> {
        let count = self.dlc_count();
        if count <= 0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(count as usize);
        for index in 0..count {
            let mut app_id = 0u32;
            let mut available = false;
            // Steam copies into the caller's buffer; 256 matches the size used
            // for the other fixed strings in this module.
            let mut name = vec![0 as std::os::raw::c_char; 256];
            let ok = unsafe {
                sys::SteamAPI_ISteamApps_BGetDLCDataByIndex(
                    self.apps,
                    index,
                    &mut app_id,
                    &mut available,
                    name.as_mut_ptr(),
                    name.len() as i32,
                )
            };
            if ok {
                out.push(DlcInfo {
                    app_id: AppId(app_id),
                    available,
                    name: fixed_cstring(&name),
                });
            }
        }
        out
    }

    /// Asks Steam to download the given DLC.
    ///
    /// Nothing is returned because Steam reports the outcome through its own
    /// download progress, not through this call.
    pub fn install_dlc(&self, app_id: AppId) {
        unsafe { sys::SteamAPI_ISteamApps_InstallDLC(self.apps, app_id.0) }
    }

    /// Asks Steam to remove the given DLC from the installation.
    pub fn uninstall_dlc(&self, app_id: AppId) {
        unsafe { sys::SteamAPI_ISteamApps_UninstallDLC(self.apps, app_id.0) }
    }

    /// Download progress for optional DLC: `(bytes_downloaded, bytes_total)`.
    ///
    /// `None` when Steam has no such DLC, which is also what it answers for a
    /// DLC that is already fully installed and needs no download.
    pub fn dlc_download_progress(&self, app_id: AppId) -> Option<(u64, u64)> {
        let mut downloaded = 0u64;
        let mut total = 0u64;
        let ok = unsafe {
            sys::SteamAPI_ISteamApps_GetDlcDownloadProgress(
                self.apps,
                app_id.0,
                &mut downloaded,
                &mut total,
            )
        };
        ok.then_some((downloaded, total))
    }

    /// Whether this copy was borrowed through Family Sharing.
    ///
    /// When true, [`Apps::app_owner`] names the lender rather than the player.
    pub fn is_subscribed_from_family_sharing(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamApps_BIsSubscribedFromFamilySharing(self.apps) }
    }

    /// Whether the app is running as a timed trial, and how much of the allowance
    /// has been used.
    ///
    /// `None` means "not a timed trial" -- it is not a reading of zero seconds.
    pub fn timed_trial(&self) -> Option<TimedTrial> {
        let mut allowed = 0u32;
        let mut played = 0u32;
        timed_trial_from(
            unsafe { sys::SteamAPI_ISteamApps_BIsTimedTrial(self.apps, &mut allowed, &mut played) },
            allowed,
            played,
        )
    }

    /// When the user first bought this app, as a Unix timestamp.
    ///
    /// Steam returns 0 when there is no purchase record, which maps to `None`.
    pub fn earliest_purchase_unix_time(&self, app_id: AppId) -> Option<u32> {
        match unsafe { sys::SteamAPI_ISteamApps_GetEarliestPurchaseUnixTime(self.apps, app_id.0) } {
            0 => None,
            seconds => Some(seconds),
        }
    }

    /// Tells Steam the install looks damaged so it should verify files.
    ///
    /// `missing_files_only` narrows the check to absent files rather than
    /// re-hashing everything. The return is Steam's "request accepted", not the
    /// verification result.
    pub fn mark_content_corrupt(&self, missing_files_only: bool) -> bool {
        unsafe { sys::SteamAPI_ISteamApps_MarkContentCorrupt(self.apps, missing_files_only) }
    }

    /// Beta branch counts: `(total, public_available, private)`.
    pub fn beta_counts(&self) -> (i32, i32, i32) {
        let mut available = 0i32;
        let mut private = 0i32;
        let total = unsafe {
            sys::SteamAPI_ISteamApps_GetNumBetas(self.apps, &mut available, &mut private)
        };
        (total, available, private)
    }

    /// Lists the beta branches Steam offers for this app.
    ///
    /// Indexes come from [`Apps::beta_counts`]; `last_updated` is a Unix
    /// timestamp of the branch's most recent build.
    pub fn betas(&self) -> Vec<BetaInfo> {
        let (total, _, _) = self.beta_counts();
        if total <= 0 {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(total as usize);
        for index in 0..total {
            let mut flags = 0u32;
            let mut build_id = 0u32;
            let mut last_updated = 0u32;
            let mut name = vec![0 as std::os::raw::c_char; 256];
            let mut description = vec![0 as std::os::raw::c_char; 1024];
            let ok = unsafe {
                sys::SteamAPI_ISteamApps_GetBetaInfo(
                    self.apps,
                    index,
                    &mut flags,
                    &mut build_id,
                    name.as_mut_ptr(),
                    name.len() as i32,
                    description.as_mut_ptr(),
                    description.len() as i32,
                    &mut last_updated,
                )
            };
            if ok {
                out.push(BetaInfo {
                    name: fixed_cstring(&name),
                    description: fixed_cstring(&description),
                    flags,
                    build_id,
                    last_updated,
                });
            }
        }
        out
    }

    /// Switches the app to a beta branch (or back to `public`).
    ///
    /// `Ok(false)` is Steam declining the branch name; a name containing an
    /// interior nul is refused as `InvalidParameter` before any call is made.
    pub fn set_active_beta(&self, branch: &str) -> Result<bool, SteamError> {
        let branch = c_string_arg(branch)?;
        Ok(unsafe { sys::SteamAPI_ISteamApps_SetActiveBeta(self.apps, branch.as_ptr()) })
    }

    /// Asks Steam for the recorded size and SHA-1 of one of the app's files,
    /// addressed the way Steam addresses it (the name is forwarded verbatim as a
    /// C string; see the `ISteamApps` docs for what it is relative to).
    ///
    /// This is asynchronous: the answer arrives through `callback` as
    /// `Result<FileDetails, SteamError>`, so a failure from Steam surfaces as an
    /// `Err` carrying its `EResult` rather than as a zero-sized file. A path with
    /// an interior nul is refused before any call is made.
    pub fn file_details<F>(&self, path: &str, callback: F) -> Result<(), SteamError>
    where
        F: FnOnce(Result<FileDetails, SteamError>) + 'static + Send,
    {
        let path = c_string_arg(path)?;
        let api_call = unsafe { sys::SteamAPI_ISteamApps_GetFileDetails(self.apps, path.as_ptr()) };
        unsafe {
            register_call_result::<sys::FileDetailsResult_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    callback(if io_error {
                        Err(SteamError::IOFailure)
                    } else {
                        to_steam_result(v.m_eResult).map(|()| FileDetails {
                            size_bytes: v.m_ulFileSize,
                            sha1: v.m_FileSHA,
                            flags: v.m_unFlags,
                        })
                    })
                },
            );
        }
        Ok(())
    }
}

/// A string argument reaching Steam as a C string: an interior nul is a bad
/// argument, not a panic.
pub(crate) fn c_string_arg(value: &str) -> Result<CString, SteamError> {
    CString::new(value).map_err(|_| SteamError::InvalidParameter)
}

/// Formats Steam's 20-byte SHA-1 field as lowercase hex.
fn sha1_hex(sha: &[u8; 20]) -> String {
    let mut out = String::with_capacity(40);
    for byte in sha {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// One file's Steam-side record, from [`Apps::file_details`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FileDetails {
    /// Original file size in bytes, as Steam stored it.
    pub size_bytes: u64,
    /// Original file SHA-1 (20 bytes).
    pub sha1: [u8; 20],
    /// Steam's file-flags bitfield. The SDK header carries no description of
    /// its bits, so it is passed through rather than interpreted.
    pub flags: u32,
}

impl FileDetails {
    /// The SHA-1 as 40 lowercase hex characters.
    pub fn sha1_hex(&self) -> String {
        sha1_hex(&self.sha1)
    }
}

/// Reads a Steam fixed-size char buffer, stopping at the first nul so a buffer
/// that was never terminated cannot run past the allocation.
fn fixed_cstring(buffer: &[std::os::raw::c_char]) -> String {
    let bytes: Vec<u8> = buffer
        .iter()
        .map(|byte| *byte as u8)
        .take_while(|byte| *byte != 0)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Turns Steam's "is this a timed trial" answer plus its two out-parameters into
/// an `Option`: only a true answer is a trial, and a trial with zero seconds
/// allowed stays a trial.
fn timed_trial_from(
    is_trial: bool,
    allowed_seconds: u32,
    played_seconds: u32,
) -> Option<TimedTrial> {
    is_trial.then_some(TimedTrial {
        allowed: Duration::from_secs(allowed_seconds as u64),
        played: Duration::from_secs(played_seconds as u64),
    })
}

/// One DLC entry from [`Apps::dlcs`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DlcInfo {
    pub app_id: AppId,
    /// Whether the user owns this DLC.
    pub available: bool,
    pub name: String,
}

/// Timed-trial allowance from [`Apps::timed_trial`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimedTrial {
    pub allowed: Duration,
    pub played: Duration,
}

/// One beta branch from [`Apps::betas`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BetaInfo {
    pub name: String,
    pub description: String,
    /// Steam's beta flags bitfield, passed through unchanged.
    pub flags: u32,
    pub build_id: u32,
    /// Unix timestamp of this branch's latest build.
    pub last_updated: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_buffers_are_read_up_to_the_nul_and_no_further() {
        let mut buffer = [0 as std::os::raw::c_char; 16];
        for (slot, byte) in buffer.iter_mut().zip(b"beta-nightly".iter().copied()) {
            *slot = byte as std::os::raw::c_char;
        }
        // Bytes past the terminator must not leak into the value.
        buffer[13] = b'X' as std::os::raw::c_char;
        assert_eq!(fixed_cstring(&buffer), "beta-nightly");

        let empty = [0 as std::os::raw::c_char; 8];
        assert_eq!(fixed_cstring(&empty), "");

        // A buffer with no terminator at all still yields a bounded string.
        let unterminated: Vec<std::os::raw::c_char> =
            "abc".bytes().map(|b| b as std::os::raw::c_char).collect();
        assert_eq!(fixed_cstring(&unterminated), "abc");
    }

    #[test]
    fn timed_trial_zero_is_not_reported_as_no_trial() {
        // The mapping `Apps::timed_trial` delegates to: Steam's bool decides
        // whether there is a trial at all, never the second counts.
        let trial = timed_trial_from(true, 0, 0);
        assert!(
            trial.is_some(),
            "a trial with a zero allowance is still a trial"
        );
        assert_eq!(trial.unwrap().allowed, Duration::ZERO);

        assert!(
            timed_trial_from(false, 7200, 0).is_none(),
            "Steam's false answer must not look like a trial"
        );
        let full = timed_trial_from(true, 7200, 1234).unwrap();
        assert_eq!(full.allowed, Duration::from_secs(7200));
        assert_eq!(full.played, Duration::from_secs(1234));
    }

    #[test]
    fn beta_branch_names_reject_interior_nul() {
        // This is the guard `set_active_beta` and `file_details` run through.
        assert_eq!(c_string_arg("public").unwrap().as_bytes(), b"public");
        assert!(matches!(
            c_string_arg("beta\0branch"),
            Err(SteamError::InvalidParameter)
        ));
    }

    #[test]
    fn sha1_is_printed_as_forty_lowercase_hex_bytes() {
        let mut sha = [0u8; 20];
        for (index, slot) in sha.iter_mut().enumerate() {
            *slot = index as u8;
        }
        let hex = sha1_hex(&sha);
        assert_eq!(hex.len(), 40);
        assert_eq!(hex, "000102030405060708090a0b0c0d0e0f10111213");

        // An all-zero digest still occupies 40 characters, not "0" or nothing.
        assert_eq!(sha1_hex(&[0u8; 20]), "00".repeat(20));
        // Upper case would not compare against Steam's own digests.
        assert!(!hex.chars().any(|c| c.is_ascii_uppercase()));
    }
}

/// Called after the user executes a steam url with command line or query parameters such as `steam://run/<appid>/?param1=value1;param2=value2;param3=value3` while the game is already running.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct NewUrlLaunchParameters;

impl_callback!(_cb: NewUrlLaunchParameters_t => NewUrlLaunchParameters {
    Self
});
