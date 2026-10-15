use super::*;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic;
use std::process::abort;
use std::sync::RwLock;

/// Access to the steam utils interface
pub struct Utils {
    pub(crate) utils: *mut sys::ISteamUtils,
    pub(crate) _inner: Arc<Inner>,
}

#[derive(Clone, Debug)]
pub struct GamepadTextInputDismissed {
    pub submitted_text_len: Option<u32>,
}

impl_callback!(cb: GamepadTextInputDismissed_t => GamepadTextInputDismissed {
    Self {
        submitted_text_len: cb.m_bSubmitted.then_some(cb.m_unSubmittedText),
    }
});

#[derive(Clone, Debug)]
pub struct FloatingGamepadTextInputDismissed;

impl_callback!(_cb: FloatingGamepadTextInputDismissed_t => FloatingGamepadTextInputDismissed {
    Self
});

pub enum NotificationPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub enum GamepadTextInputMode {
    Normal,
    Password,
}

impl From<GamepadTextInputMode> for sys::EGamepadTextInputMode {
    fn from(mode: GamepadTextInputMode) -> Self {
        match mode {
            GamepadTextInputMode::Normal => {
                sys::EGamepadTextInputMode::k_EGamepadTextInputModeNormal
            }
            GamepadTextInputMode::Password => {
                sys::EGamepadTextInputMode::k_EGamepadTextInputModePassword
            }
        }
    }
}

pub enum GamepadTextInputLineMode {
    SingleLine,
    MultipleLines,
}

impl From<GamepadTextInputLineMode> for sys::EGamepadTextInputLineMode {
    fn from(mode: GamepadTextInputLineMode) -> Self {
        match mode {
            GamepadTextInputLineMode::SingleLine => {
                sys::EGamepadTextInputLineMode::k_EGamepadTextInputLineModeSingleLine
            }
            GamepadTextInputLineMode::MultipleLines => {
                sys::EGamepadTextInputLineMode::k_EGamepadTextInputLineModeMultipleLines
            }
        }
    }
}

pub enum FloatingGamepadTextInputMode {
    SingleLine,
    MultipleLines,
    Email,
    Numeric,
}

impl From<FloatingGamepadTextInputMode> for sys::EFloatingGamepadTextInputMode {
    fn from(mode: FloatingGamepadTextInputMode) -> Self {
        match mode {
            FloatingGamepadTextInputMode::SingleLine => {
                sys::EFloatingGamepadTextInputMode::k_EFloatingGamepadTextInputModeModeSingleLine
            }
            FloatingGamepadTextInputMode::MultipleLines => {
                sys::EFloatingGamepadTextInputMode::k_EFloatingGamepadTextInputModeModeMultipleLines
            }
            FloatingGamepadTextInputMode::Email => {
                sys::EFloatingGamepadTextInputMode::k_EFloatingGamepadTextInputModeModeEmail
            }
            FloatingGamepadTextInputMode::Numeric => {
                sys::EFloatingGamepadTextInputMode::k_EFloatingGamepadTextInputModeModeNumeric
            }
        }
    }
}

/// Global rust warning callback
static WARNING_CALLBACK: RwLock<Option<Box<dyn Fn(i32, &CStr) + Send + Sync>>> = RwLock::new(None);

/// C function to pass as the real callback, which forwards to the `WARNING_CALLBACK` if any
unsafe extern "C" fn c_warning_callback(level: i32, msg: *const c_char) {
    let lock = WARNING_CALLBACK.read().expect("warning func lock poisoned");
    let cb = match lock.as_ref() {
        Some(cb) => cb,
        None => {
            return;
        }
    };

    let s = CStr::from_ptr(msg);

    let res = panic::catch_unwind(panic::AssertUnwindSafe(|| cb(level, s)));
    if let Err(err) = res {
        if let Some(err) = err.downcast_ref::<&str>() {
            println!("Steam warning callback panicked: {}", err);
        } else if let Some(err) = err.downcast_ref::<String>() {
            println!("Steam warning callback panicked: {}", err);
        } else {
            println!("Steam warning callback panicked");
        }
        abort();
    }
}

impl Utils {
    /// Returns the app ID of the current process
    pub fn app_id(&self) -> AppId {
        unsafe { AppId(sys::SteamAPI_ISteamUtils_GetAppID(self.utils)) }
    }

    /// Returns the country code of the current user based on their IP
    pub fn ip_country(&self) -> String {
        unsafe {
            let ipcountry = sys::SteamAPI_ISteamUtils_GetIPCountry(self.utils);
            let ipcountry = CStr::from_ptr(ipcountry);
            ipcountry.to_string_lossy().into_owned()
        }
    }

    /// Returns whether or not the overlay is enabled in Steam
    pub fn is_overlay_enabled(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_IsOverlayEnabled(self.utils) }
    }

    /// Returns the language the steam client is currently
    /// running in.
    ///
    /// Generally you want `Apps::current_game_language` instead of this
    pub fn ui_language(&self) -> String {
        unsafe {
            let lang = sys::SteamAPI_ISteamUtils_GetSteamUILanguage(self.utils);
            let lang = CStr::from_ptr(lang);
            lang.to_string_lossy().into_owned()
        }
    }

    /// Returns the current real time on the Steam servers
    /// in Unix epoch format (seconds since 1970/1/1 UTC).
    pub fn get_server_real_time(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamUtils_GetServerRealTime(self.utils) }
    }

    /// Sets the position on the screen where popups from the steam overlay
    /// should appear and display themselves in.
    pub fn set_overlay_notification_position(&self, position: NotificationPosition) {
        unsafe {
            let position = match position {
                NotificationPosition::TopLeft => sys::ENotificationPosition::k_EPositionTopLeft,
                NotificationPosition::TopRight => sys::ENotificationPosition::k_EPositionTopRight,
                NotificationPosition::BottomLeft => {
                    sys::ENotificationPosition::k_EPositionBottomLeft
                }
                NotificationPosition::BottomRight => {
                    sys::ENotificationPosition::k_EPositionBottomRight
                }
            };
            sys::SteamAPI_ISteamUtils_SetOverlayNotificationPosition(self.utils, position);
        }
    }

    /// Sets the Steam warning callback, which is called to emit warning messages.
    ///
    /// The passed-in function takes two arguments: a severity level (0 = info, 1 = warning) and
    /// the message itself.
    ///
    /// See [Steamwork's debugging page](https://partner.steamgames.com/doc/sdk/api/debugging) for more info.
    pub fn set_warning_callback<F>(&self, cb: F)
    where
        F: Fn(i32, &CStr) + Send + Sync + 'static,
    {
        let mut lock = WARNING_CALLBACK
            .write()
            .expect("warning func lock poisoned");
        *lock = Some(Box::new(cb));
        unsafe {
            sys::SteamAPI_ISteamUtils_SetWarningMessageHook(self.utils, Some(c_warning_callback));
        }
    }

    /// Gets the gamepad text input from the Big Picture overlay.
    ///
    /// This must be called within the `show_gamepad_text_input` callback. Returns `Some(String)` if user submitted the
    /// text, and `None` otherwise
    pub fn get_entered_gamepad_text_input(
        &self,
        dismissed_data: &GamepadTextInputDismissed,
    ) -> Option<String> {
        unsafe {
            let len = dismissed_data.submitted_text_len?;
            let mut buf = vec![0u8; len as usize];

            sys::SteamAPI_ISteamUtils_GetEnteredGamepadTextInput(
                self.utils,
                buf.as_mut_ptr().cast(),
                len,
            )
            .then(|| String::from_utf8(buf).expect("Steamworks returned invalid UTF-8 string"))
        }
    }

    /// Checks if Steam & the Steam Overlay are running in Big Picture mode.
    ///
    /// Games must be launched through the Steam client to enable the Big Picture overlay.
    /// During development, a game can be added as a non-steam game to the developer's library to test this feature.
    pub fn is_steam_in_big_picture_mode(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_IsSteamInBigPictureMode(self.utils) }
    }

    /// Checks if Steam is running on a Steam Deck device.
    pub fn is_steam_running_on_steam_deck(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_IsSteamRunningOnSteamDeck(self.utils) }
    }

    /// Activates the Big Picture text input dialog which only supports gamepad input.
    pub fn show_gamepad_text_input<F>(
        &self,
        input_mode: GamepadTextInputMode,
        input_line_mode: GamepadTextInputLineMode,
        description: &str,
        max_characters: u32,
        existing_text: Option<&str>,
        dismissed_cb: F,
    ) -> bool
    where
        F: FnMut(GamepadTextInputDismissed) + 'static + Send,
    {
        unsafe {
            let description = CString::new(description).unwrap();
            let existing_text = existing_text.map(|s| CString::new(s).unwrap());
            std::mem::forget(register_callback(&self._inner, dismissed_cb));
            sys::SteamAPI_ISteamUtils_ShowGamepadTextInput(
                self.utils,
                input_mode.into(),
                input_line_mode.into(),
                description.as_ptr(),
                max_characters,
                existing_text
                    .as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(std::ptr::null()),
            )
        }
    }

    /// Opens a floating keyboard over the game content and sends OS keyboard keys directly to the game.
    ///
    /// The text field position is specified in pixels relative the origin of the game window and is used to
    /// position the floating keyboard in a way that doesn't cover the text field.
    ///
    /// Callback is triggered when user dismisses the text input
    pub fn show_floating_gamepad_text_input<F>(
        &self,
        keyboard_mode: FloatingGamepadTextInputMode,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        mut dismissed_cb: F,
    ) -> bool
    where
        F: FnMut() + 'static + Send, // TODO: Support FnOnce callbacks
    {
        unsafe {
            std::mem::forget(register_callback(
                &self._inner,
                move |_: FloatingGamepadTextInputDismissed| {
                    dismissed_cb();
                },
            ));
            sys::SteamAPI_ISteamUtils_ShowFloatingGamepadTextInput(
                self.utils,
                keyboard_mode.into(),
                x,
                y,
                width,
                height,
            )
        }
    }

    /// Seconds the current application has been active, for debugging.
    pub fn seconds_since_app_active(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamUtils_GetSecondsSinceAppActive(self.utils) }
    }

    /// Seconds the computer has been active, for debugging.
    pub fn seconds_since_computer_active(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamUtils_GetSecondsSinceComputerActive(self.utils) }
    }

    /// Steam IPC calls made since the last time this was called.
    ///
    /// Every call costs at least a thread context switch, so this is a way to
    /// see how much overlay traffic a frame generates.
    pub fn ipc_call_count(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamUtils_GetIPCCallCount(self.utils) }
    }

    /// Battery state of the machine Steam is running on.
    ///
    /// Steam reports "on AC power" with the sentinel value 255 rather than a
    /// percentage, so that case maps to [`BatteryPower::OnAcPower`] instead of
    /// leaking a 255% reading.
    pub fn battery_power(&self) -> BatteryPower {
        battery_power_from_raw(unsafe {
            sys::SteamAPI_ISteamUtils_GetCurrentBatteryPower(self.utils)
        })
    }

    /// Whether the overlay still has something to present.
    ///
    /// When this is true the game should keep presenting frames; a game that
    /// blocks without presenting will stall the overlay.
    pub fn overlay_needs_present(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_BOverlayNeedsPresent(self.utils) }
    }

    /// Whether Steam is running inside a VR session.
    pub fn is_steam_running_in_vr(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_IsSteamRunningInVR(self.utils) }
    }

    /// Whether the game is running from a Steam China launcher.
    pub fn is_steam_china_launcher(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_IsSteamChinaLauncher(self.utils) }
    }

    /// Length in bytes of the text the user entered in the last gamepad text
    /// input dialog, which is what
    /// [`Utils::get_entered_gamepad_text_input`] then reads.
    pub fn entered_gamepad_text_length(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamUtils_GetEnteredGamepadTextLength(self.utils) }
    }

    /// Closes an open Big Picture text input dialog.
    ///
    /// Returns whether a dialog was actually dismissed.
    pub fn dismiss_gamepad_text_input(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_DismissGamepadTextInput(self.utils) }
    }

    /// Closes an open floating gamepad text input dialog.
    ///
    /// Returns whether a dialog was actually dismissed.
    pub fn dismiss_floating_gamepad_text_input(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_DismissFloatingGamepadTextInput(self.utils) }
    }

    /// Offsets where overlay notifications appear, from the position set with
    /// [`Utils::set_overlay_notification_position`].
    ///
    /// This is per-game, and does nothing when called outside a game context.
    pub fn set_overlay_notification_inset(&self, horizontal: i32, vertical: i32) {
        unsafe {
            sys::SteamAPI_ISteamUtils_SetOverlayNotificationInset(self.utils, horizontal, vertical)
        }
    }

    /// Whether the host PC streams the game's output to the headset, rather
    /// than running a desktop VR session.
    pub fn is_vr_headset_streaming_enabled(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_IsVRHeadsetStreamingEnabled(self.utils) }
    }

    /// Turns in-headset streaming on or off.
    pub fn set_vr_headset_streaming_enabled(&self, enabled: bool) {
        unsafe {
            sys::SteamAPI_ISteamUtils_SetVRHeadsetStreamingEnabled(self.utils, enabled);
        }
    }

    /// What Steam believes the machine's IPv6 connectivity to the internet is
    /// over `protocol`.
    ///
    /// Note what this does *not* say: it does not tell you whether the Steam
    /// client itself reached Steam over IPv6, and a game that needs a real
    /// answer about a specific host should probe it rather than trust this.
    pub fn ipv6_connectivity_state(&self, protocol: IPv6ConnectivityProtocol) -> IPv6Connectivity {
        ipv6_state_from_raw(unsafe {
            sys::SteamAPI_ISteamUtils_GetIPv6ConnectivityState(self.utils, protocol.into())
        })
    }

    /// Loads the text-filter dictionaries for the language Steam is running in.
    ///
    /// `options` is reserved by Steam and must be 0. Returns whether filtering is
    /// available: when it returns false, Steam says
    /// [`Utils::filter_text`] acts as a passthrough rather than failing, so a
    /// caller must not read "unavailable" as "clean text".
    pub fn init_text_filter(&self, options: u32) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_InitFilterText(self.utils, options) }
    }

    /// Runs Steam's community text filter over `input`.
    ///
    /// `source` is who the text belongs to (the chat speaker, or the owner of a
    /// player name), because the user's own filter settings apply per source.
    /// Steam writes the result -- filtered or not -- into a buffer it expects to
    /// be at least one byte larger than the input, which this allocates for you,
    /// and returns how many characters it changed.
    pub fn filter_text(
        &self,
        context: TextFilteringContext,
        source: SteamId,
        input: &str,
    ) -> Result<FilteredText, SteamError> {
        let input = CString::new(input).map_err(|_| SteamError::InvalidParameter)?;
        let input_len = input.as_bytes().len();
        // Steam asks for strlen(input)+1; the extra slot is the terminator this
        // reads back to, and it is pre-zeroed so a missing terminator cannot
        // make the read run past the allocation.
        let mut out: Vec<std::os::raw::c_char> = vec![0; input_len + 1];
        let filtered = unsafe {
            sys::SteamAPI_ISteamUtils_FilterText(
                self.utils,
                context.into(),
                source.raw(),
                input.as_ptr(),
                out.as_mut_ptr(),
                u32::try_from(out.len()).map_err(|_| SteamError::InvalidParameter)?,
            )
        };
        filtered_text_from_buffer(&out, filtered)
    }
}

/// Which kind of content a filtered string is, as Steam classifies it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFilteringContext {
    Unknown,
    GameContent,
    Chat,
    Name,
}

impl From<TextFilteringContext> for sys::ETextFilteringContext {
    fn from(context: TextFilteringContext) -> Self {
        match context {
            TextFilteringContext::Unknown => {
                sys::ETextFilteringContext::k_ETextFilteringContextUnknown
            }
            TextFilteringContext::GameContent => {
                sys::ETextFilteringContext::k_ETextFilteringContextGameContent
            }
            TextFilteringContext::Chat => sys::ETextFilteringContext::k_ETextFilteringContextChat,
            TextFilteringContext::Name => sys::ETextFilteringContext::k_ETextFilteringContextName,
        }
    }
}

/// What [`Utils::filter_text`] produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FilteredText {
    /// The text Steam wrote back, which equals the input when nothing was filtered.
    pub text: String,
    /// Characters Steam changed -- a count of characters, not bytes.
    pub filtered_characters: usize,
}

/// Reads the filter output without trusting it to be terminated: the buffer is
/// cut at the first nul, and a negative return from Steam is a failure rather
/// than zero changes.
fn filtered_text_from_buffer(
    buffer: &[std::os::raw::c_char],
    filtered: i32,
) -> Result<FilteredText, SteamError> {
    if filtered < 0 {
        return Err(SteamError::IOFailure);
    }
    let bytes: Vec<u8> = buffer
        .iter()
        .map(|byte| *byte as u8)
        .take_while(|byte| *byte != 0)
        .collect();
    Ok(FilteredText {
        text: String::from_utf8_lossy(&bytes).into_owned(),
        filtered_characters: filtered as usize,
    })
}

/// Which transport to ask about with [`Utils::ipv6_connectivity_state`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IPv6ConnectivityProtocol {
    Http,
    Udp,
}

impl From<IPv6ConnectivityProtocol> for sys::ESteamIPv6ConnectivityProtocol {
    fn from(protocol: IPv6ConnectivityProtocol) -> Self {
        match protocol {
            IPv6ConnectivityProtocol::Http => {
                sys::ESteamIPv6ConnectivityProtocol::k_ESteamIPv6ConnectivityProtocol_HTTP
            }
            IPv6ConnectivityProtocol::Udp => {
                sys::ESteamIPv6ConnectivityProtocol::k_ESteamIPv6ConnectivityProtocol_UDP
            }
        }
    }
}

/// Steam's belief about IPv6 reachability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IPv6Connectivity {
    Unknown,
    Good,
    Bad,
}

fn ipv6_state_from_raw(raw: sys::ESteamIPv6ConnectivityState) -> IPv6Connectivity {
    match raw {
        sys::ESteamIPv6ConnectivityState::k_ESteamIPv6ConnectivityState_Good => {
            IPv6Connectivity::Good
        }
        sys::ESteamIPv6ConnectivityState::k_ESteamIPv6ConnectivityState_Bad => {
            IPv6Connectivity::Bad
        }
        // The binding is `#[non_exhaustive]`, and Steam's own Unknown answer
        // has to survive as Unknown rather than being read as Good or Bad.
        _ => IPv6Connectivity::Unknown,
    }
}

/// Battery state as Steam reports it, with the "on AC power" sentinel resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryPower {
    /// Plugged in; Steam reports this as the sentinel 255 rather than a percentage.
    OnAcPower,
    /// Percentage remaining, 0-100 as Steam reports it.
    Percent(u8),
}

/// The FFI reports battery percent as a `u8` and uses 255 for "plugged in";
/// without this split a caller reading `Percent` would see 255%.
pub(crate) fn battery_power_from_raw(raw: u8) -> BatteryPower {
    match raw {
        255 => BatteryPower::OnAcPower,
        percent => BatteryPower::Percent(percent),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ac_power_sentinel_is_not_reported_as_255_percent() {
        assert_eq!(battery_power_from_raw(255), BatteryPower::OnAcPower);
        assert_eq!(battery_power_from_raw(0), BatteryPower::Percent(0));
        assert_eq!(battery_power_from_raw(100), BatteryPower::Percent(100));
        // Anything under the sentinel stays a percentage, untouched.
        for raw in [1u8, 42, 100, 254] {
            assert_eq!(battery_power_from_raw(raw), BatteryPower::Percent(raw));
        }
    }

    #[test]
    fn ipv6_protocols_map_onto_distinct_steam_variants() {
        use sys::ESteamIPv6ConnectivityProtocol as S;
        let mapped = [
            S::from(IPv6ConnectivityProtocol::Http),
            S::from(IPv6ConnectivityProtocol::Udp),
        ];
        assert_ne!(mapped[0], mapped[1]);
    }

    #[test]
    fn ipv6_unknown_stays_unknown() {
        use sys::ESteamIPv6ConnectivityState as S;
        assert_eq!(
            ipv6_state_from_raw(S::k_ESteamIPv6ConnectivityState_Unknown),
            IPv6Connectivity::Unknown
        );
        assert_eq!(
            ipv6_state_from_raw(S::k_ESteamIPv6ConnectivityState_Good),
            IPv6Connectivity::Good
        );
        assert_eq!(
            ipv6_state_from_raw(S::k_ESteamIPv6ConnectivityState_Bad),
            IPv6Connectivity::Bad
        );
    }

    fn cbuf(text: &str, capacity: usize) -> Vec<std::os::raw::c_char> {
        let mut buf = vec![0 as std::os::raw::c_char; capacity];
        for (slot, byte) in buf.iter_mut().zip(text.as_bytes()) {
            *slot = *byte as std::os::raw::c_char;
        }
        buf
    }

    #[test]
    fn filter_output_is_cut_at_the_first_nul_not_read_past_it() {
        let ok = filtered_text_from_buffer(&cbuf("hi***", 8), 3).unwrap();
        assert_eq!(ok.text, "hi***");
        assert_eq!(ok.filtered_characters, 3);

        // Steam writes into a buffer it was given as strlen(input)+1; anything
        // after the terminator must not end up in the string.
        let padded = filtered_text_from_buffer(&cbuf("clean", 32), 0).unwrap();
        assert_eq!(padded.text, "clean");
        assert_eq!(padded.filtered_characters, 0);

        // A missing terminator must not run the read past the allocation.
        let unterminated: Vec<std::os::raw::c_char> =
            "abc".bytes().map(|b| b as std::os::raw::c_char).collect();
        assert_eq!(
            filtered_text_from_buffer(&unterminated, 1).unwrap().text,
            "abc"
        );
    }

    #[test]
    fn a_negative_filter_count_is_a_failure_not_zero_changes() {
        assert!(matches!(
            filtered_text_from_buffer(&cbuf("x", 2), -1),
            Err(SteamError::IOFailure)
        ));
        assert_eq!(
            filtered_text_from_buffer(&cbuf("x", 2), 0)
                .unwrap()
                .filtered_characters,
            0
        );
    }

    #[test]
    fn filter_contexts_map_onto_distinct_steam_variants() {
        use sys::ETextFilteringContext as S;
        let contexts = [
            TextFilteringContext::Unknown,
            TextFilteringContext::GameContent,
            TextFilteringContext::Chat,
            TextFilteringContext::Name,
        ];
        let mapped: Vec<S> = contexts.iter().copied().map(S::from).collect();
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(
                    mapped[i], mapped[j],
                    "{:?}/{:?} collide",
                    contexts[i], contexts[j]
                );
            }
        }
    }
}

pub(crate) struct SteamParamStringArray(Vec<*mut c_char>);
impl Drop for SteamParamStringArray {
    fn drop(&mut self) {
        for c_string in &self.0 {
            unsafe { drop(CString::from_raw(*c_string)) };
        }
    }
}
impl SteamParamStringArray {
    /// Builds the array, or `None` when an entry cannot become a C string.
    ///
    /// The entries are `into_raw` allocations that Steam reads by pointer, so a failure halfway
    /// through must hand back what was already taken. The old version `expect`ed instead, which
    /// did two bad things at once: it panicked inside the caller's process, and the strings
    /// converted before the bad one had no owner left to free them.
    pub(crate) fn new<S: AsRef<str>>(vec: &[S]) -> Option<SteamParamStringArray> {
        let mut raw = Vec::with_capacity(vec.len());
        for s in vec {
            match CString::new(s.as_ref()) {
                Ok(value) => raw.push(value.into_raw()),
                Err(_) => {
                    for taken in raw {
                        drop(unsafe { CString::from_raw(taken) });
                    }
                    return None;
                }
            }
        }
        Some(SteamParamStringArray(raw))
    }

    pub(crate) fn as_raw(&mut self) -> sys::SteamParamStringArray_t {
        sys::SteamParamStringArray_t {
            m_nNumStrings: self.0.len() as i32,
            m_ppStrings: self.0.as_mut_ptr().cast(),
        }
    }
}
