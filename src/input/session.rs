//! Session and identity odds and ends: the Remote Play session id, the session input
//! configuration revision, a device's binding revision, the new-data signal and its wait, and
//! the gamepad-index to handle mapping in both directions.

use super::*;

impl Input {
    /// The Remote Play session driving this controller, as Steam reports it.
    pub fn remote_play_session_id(&self, controller: sys::InputHandle_t) -> u32 {
        unsafe { sys::SteamAPI_ISteamInput_GetRemotePlaySessionID(self.input, controller) }
    }

    /// Bitfield of the session's input configuration settings.
    pub fn session_input_configuration_settings(&self) -> u16 {
        unsafe { sys::SteamAPI_ISteamInput_GetSessionInputConfigurationSettings(self.input) }
    }

    /// The binding revision Steam has loaded for a device.
    ///
    /// `None` covers both of Steam's negative answers -- an invalid handle, or "a mapping is not
    /// yet loaded for the device" (`isteaminput.h:1008`) -- and the pair is then dropped rather
    /// than reported, because Steam may not have written either out parameter.
    pub fn device_binding_revision(&self, controller: sys::InputHandle_t) -> Option<(i32, i32)> {
        let mut major = 0;
        let mut minor = 0;
        let ok = unsafe {
            sys::SteamAPI_ISteamInput_GetDeviceBindingRevision(
                self.input, controller, &mut major, &mut minor,
            )
        };
        binding_revision(ok, major, minor)
    }

    /// Whether new input data has arrived since the last `run_frame`.
    pub fn new_data_available(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamInput_BNewDataAvailable(self.input) }
    }

    /// Block until Steam sends new data, for a game with a dedicated input thread.
    ///
    /// This parks the calling thread: it waits forever when `wait_forever` is set, otherwise up to
    /// `timeout_ms`. Returns `true` only if data arrived before the deadline
    /// (`isteaminput.h:828-830`).
    pub fn wait_for_data(&self, wait_forever: bool, timeout_ms: u32) -> bool {
        unsafe { sys::SteamAPI_ISteamInput_BWaitForData(self.input, wait_forever, timeout_ms) }
    }

    /// The controller driving a legacy gamepad index, as Steam maps it.
    pub fn controller_for_gamepad_index(&self, index: i32) -> sys::InputHandle_t {
        unsafe { sys::SteamAPI_ISteamInput_GetControllerForGamepadIndex(self.input, index) }
    }

    /// The legacy gamepad index Steam assigns to a controller.
    pub fn gamepad_index_for_controller(&self, controller: sys::InputHandle_t) -> i32 {
        unsafe { sys::SteamAPI_ISteamInput_GetGamepadIndexForController(self.input, controller) }
    }
}
