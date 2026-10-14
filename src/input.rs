use sys::InputHandle_t;

use super::*;

/// Access to the steam input interface
pub struct Input {
    pub(crate) input: *mut sys::ISteamInput,
    pub(crate) _inner: Arc<Inner>,
}

pub enum InputType {
    Unknown,
    SteamController,
    XBox360Controller,
    XBoxOneController,
    GenericGamepad,
    PS4Controller,
    AppleMFiController,
    AndroidController,
    SwitchJoyConPair,
    SwitchJoyConSingle,
    SwitchProController,
    MobileTouch,
    PS3Controller,
    PS5Controller,
    SteamDeckController,
}

impl Input {
    /// Init must be called when starting use of this interface.
    /// if explicitly_call_run_frame is called then you will need to manually call RunFrame
    /// each frame, otherwise Steam Input will updated when SteamAPI_RunCallbacks() is called
    pub fn init(&self, explicitly_call_run_frame: bool) -> bool {
        unsafe { sys::SteamAPI_ISteamInput_Init(self.input, explicitly_call_run_frame) }
    }

    /// Synchronize API state with the latest Steam Input action data available. This
    /// is performed automatically by SteamAPI_RunCallbacks, but for the absolute lowest
    /// possible latency, you call this directly before reading controller state.
    /// Note: This must be called from somewhere before GetConnectedControllers will
    /// return any handles
    pub fn run_frame(&self) {
        unsafe { sys::SteamAPI_ISteamInput_RunFrame(self.input, false) }
    }

    /// Returns a list of the currently connected controllers
    pub fn get_connected_controllers(&self) -> Vec<sys::InputHandle_t> {
        let mut handles = vec![0_u64; sys::STEAM_INPUT_MAX_COUNT as usize];
        let quantity = self.get_connected_controllers_slice(&mut handles);
        handles.shrink_to(quantity);
        handles
    }

    /// Returns a list of the currently connected controllers without allocating, and the count
    pub fn get_connected_controllers_slice(
        &self,
        mut controllers: impl AsMut<[InputHandle_t]>,
    ) -> usize {
        let handles = controllers.as_mut();
        assert!(handles.len() >= sys::STEAM_INPUT_MAX_COUNT as usize);
        unsafe {
            return sys::SteamAPI_ISteamInput_GetConnectedControllers(
                self.input,
                handles.as_mut_ptr(),
            ) as usize;
        }
    }

    /// Allows to load a specific Action Manifest File localy
    pub fn set_input_action_manifest_file_path(&self, path: &str) -> bool {
        let path = CString::new(path).unwrap();
        unsafe {
            sys::SteamAPI_ISteamInput_SetInputActionManifestFilePath(self.input, path.as_ptr())
        }
    }

    /// Returns the associated ControllerActionSet handle for the specified controller,
    pub fn get_action_set_handle(&self, action_set_name: &str) -> sys::InputActionSetHandle_t {
        let name = CString::new(action_set_name).unwrap();
        unsafe { sys::SteamAPI_ISteamInput_GetActionSetHandle(self.input, name.as_ptr()) }
    }

    /// Returns the input type for a controler
    pub fn get_input_type_for_handle(&self, input_handle: sys::InputHandle_t) -> InputType {
        let input_type: sys::ESteamInputType =
            unsafe { sys::SteamAPI_ISteamInput_GetInputTypeForHandle(self.input, input_handle) };

        match input_type {
            sys::ESteamInputType::k_ESteamInputType_SteamController => InputType::SteamController,
            sys::ESteamInputType::k_ESteamInputType_GenericGamepad => InputType::GenericGamepad,
            sys::ESteamInputType::k_ESteamInputType_PS4Controller => InputType::PS4Controller,
            sys::ESteamInputType::k_ESteamInputType_SwitchJoyConPair => InputType::SwitchJoyConPair,
            sys::ESteamInputType::k_ESteamInputType_MobileTouch => InputType::MobileTouch,
            sys::ESteamInputType::k_ESteamInputType_PS3Controller => InputType::PS3Controller,
            sys::ESteamInputType::k_ESteamInputType_PS5Controller => InputType::PS5Controller,
            sys::ESteamInputType::k_ESteamInputType_XBox360Controller => {
                InputType::XBox360Controller
            }
            sys::ESteamInputType::k_ESteamInputType_XBoxOneController => {
                InputType::XBoxOneController
            }
            sys::ESteamInputType::k_ESteamInputType_AppleMFiController => {
                InputType::AppleMFiController
            }
            sys::ESteamInputType::k_ESteamInputType_AndroidController => {
                InputType::AndroidController
            }
            sys::ESteamInputType::k_ESteamInputType_SwitchJoyConSingle => {
                InputType::SwitchJoyConSingle
            }
            sys::ESteamInputType::k_ESteamInputType_SwitchProController => {
                InputType::SwitchProController
            }
            sys::ESteamInputType::k_ESteamInputType_SteamDeckController => {
                InputType::SteamDeckController
            }
            _ => InputType::Unknown,
        }
    }

    /// Returns the glyph for an input action
    pub fn get_glyph_for_action_origin(&self, action_origin: sys::EInputActionOrigin) -> String {
        unsafe {
            let glyph_path =
                sys::SteamAPI_ISteamInput_GetGlyphForActionOrigin_Legacy(self.input, action_origin);
            let glyph_path = CStr::from_ptr(glyph_path);
            glyph_path.to_string_lossy().into_owned()
        }
    }

    /// Returns the name of an input action
    pub fn get_string_for_action_origin(&self, action_origin: sys::EInputActionOrigin) -> String {
        unsafe {
            let name_path =
                sys::SteamAPI_ISteamInput_GetStringForActionOrigin(self.input, action_origin);
            let name_path = CStr::from_ptr(name_path);
            name_path.to_string_lossy().into_owned()
        }
    }

    /// Reconfigure the controller to use the specified action set
    /// This is cheap, and can be safely called repeatedly.
    pub fn activate_action_set_handle(
        &self,
        input_handle: sys::InputHandle_t,
        action_set_handle: sys::InputActionSetHandle_t,
    ) {
        unsafe {
            sys::SteamAPI_ISteamInput_ActivateActionSet(self.input, input_handle, action_set_handle)
        }
    }

    /// Get the handle of the specified Digital action.
    pub fn get_digital_action_handle(&self, action_name: &str) -> sys::InputDigitalActionHandle_t {
        let name = CString::new(action_name).unwrap();
        unsafe { sys::SteamAPI_ISteamInput_GetDigitalActionHandle(self.input, name.as_ptr()) }
    }

    /// Get the handle of the specified Analog action.
    pub fn get_analog_action_handle(&self, action_name: &str) -> sys::InputAnalogActionHandle_t {
        let name = CString::new(action_name).unwrap();
        unsafe { sys::SteamAPI_ISteamInput_GetAnalogActionHandle(self.input, name.as_ptr()) }
    }

    /// Returns the current state of the supplied digital game action.
    pub fn get_digital_action_data(
        &self,
        input_handle: sys::InputHandle_t,
        action_handle: sys::InputDigitalActionHandle_t,
    ) -> sys::InputDigitalActionData_t {
        unsafe {
            sys::SteamAPI_ISteamInput_GetDigitalActionData(self.input, input_handle, action_handle)
        }
    }

    /// Returns the current state of the supplied analog game action.
    pub fn get_analog_action_data(
        &self,
        input_handle: sys::InputHandle_t,
        action_handle: sys::InputAnalogActionHandle_t,
    ) -> sys::InputAnalogActionData_t {
        unsafe {
            sys::SteamAPI_ISteamInput_GetAnalogActionData(self.input, input_handle, action_handle)
        }
    }

    /// Get the origin(s) for a digital action within an action set.
    pub fn get_digital_action_origins(
        &self,
        input_handle: sys::InputHandle_t,
        action_set_handle: sys::InputActionSetHandle_t,
        digital_action_handle: sys::InputDigitalActionHandle_t,
    ) -> Vec<sys::EInputActionOrigin> {
        unsafe {
            let mut origins = Vec::with_capacity(sys::STEAM_INPUT_MAX_ORIGINS as usize);
            let len = sys::SteamAPI_ISteamInput_GetDigitalActionOrigins(
                self.input,
                input_handle,
                action_set_handle,
                digital_action_handle,
                origins.as_mut_ptr(),
            );
            origins.set_len(len as usize);
            origins
        }
    }

    /// Get the origin(s) for an analog action within an action set.
    pub fn get_analog_action_origins(
        &self,
        input_handle: sys::InputHandle_t,
        action_set_handle: sys::InputActionSetHandle_t,
        analog_action_handle: sys::InputAnalogActionHandle_t,
    ) -> Vec<sys::EInputActionOrigin> {
        unsafe {
            let mut origins = Vec::with_capacity(sys::STEAM_INPUT_MAX_ORIGINS as usize);
            let len = sys::SteamAPI_ISteamInput_GetAnalogActionOrigins(
                self.input,
                input_handle,
                action_set_handle,
                analog_action_handle,
                origins.as_mut_ptr(),
            );
            origins.set_len(len as usize);
            origins
        }
    }

    pub fn get_motion_data(&self, input_handle: sys::InputHandle_t) -> sys::InputMotionData_t {
        unsafe { sys::SteamAPI_ISteamInput_GetMotionData(self.input, input_handle) }
    }

    /// Invokes the Steam overlay and brings up the binding screen.
    /// Returns true for success, false if overlay is disabled/unavailable.
    /// If the player is using Big Picture Mode the configuration will open in
    /// the overlay. In desktop mode a popup window version of Big Picture will
    /// be created and open the configuration.
    pub fn show_binding_panel(&self, input_handle: sys::InputHandle_t) -> bool {
        unsafe { sys::SteamAPI_ISteamInput_ShowBindingPanel(self.input, input_handle) }
    }

    /// Shutdown must be called when ending use of this interface.
    pub fn shutdown(&self) {
        unsafe {
            sys::SteamAPI_ISteamInput_Shutdown(self.input);
        }
    }
}

/// Steam's two-part answer about a device's loaded binding, or nothing at all.
///
/// Extracted so the test below exercises the same function the production path runs, rather than
/// restating `bool::then_some` in the test -- a test of std's behaviour would stay green however
/// the mapping broke.
fn binding_revision(ok: bool, major: i32, minor: i32) -> Option<(i32, i32)> {
    ok.then_some((major, minor))
}

/// How many layer handles this wrapper will read back: Steam's own count, never negative and
/// never more than the `STEAM_INPUT_MAX_ACTIVE_LAYERS` array it was handed.
fn layers_to_take(reported: i32, capacity: usize) -> usize {
    (reported.max(0) as usize).min(capacity)
}

impl Input {
    /// Trigger a vibration event on a supported controller. Steam translates this into haptic
    /// pulses for Steam Controllers (`isteaminput.h:953`).
    pub fn trigger_vibration(&self, controller: sys::InputHandle_t, left: u16, right: u16) {
        unsafe { sys::SteamAPI_ISteamInput_TriggerVibration(self.input, controller, left, right) }
    }

    /// The same, including Xbox trigger impulse rumble (`isteaminput.h:956`).
    pub fn trigger_vibration_extended(
        &self,
        controller: sys::InputHandle_t,
        left: u16,
        right: u16,
        left_trigger: u16,
        right_trigger: u16,
    ) {
        unsafe {
            sys::SteamAPI_ISteamInput_TriggerVibrationExtended(
                self.input,
                controller,
                left,
                right,
                left_trigger,
                right_trigger,
            )
        }
    }

    /// Send a haptic pulse. Works on Steam Deck and Steam Controller devices
    /// (`isteaminput.h:959`). Intensities are `uint8` and the gains are signed dB, both as Steam
    /// declares them; the `other_*` pair applies when the location is `Both`.
    pub fn trigger_simple_haptic_event(
        &self,
        controller: sys::InputHandle_t,
        location: sys::EControllerHapticLocation,
        intensity: u8,
        gain: i8,
        other_intensity: u8,
        other_gain: i8,
    ) {
        unsafe {
            sys::SteamAPI_ISteamInput_TriggerSimpleHapticEvent(
                self.input,
                controller,
                location,
                intensity,
                gain,
                other_intensity,
                other_gain,
            )
        }
    }

    /// Stop the momentum of an analog action, so a released stick does not keep drifting.
    pub fn stop_analog_action_momentum(
        &self,
        controller: sys::InputHandle_t,
        action: sys::InputAnalogActionHandle_t,
    ) {
        unsafe {
            sys::SteamAPI_ISteamInput_StopAnalogActionMomentum(self.input, controller, action)
        }
    }

    /// Set the controller LED colour on supported controllers.
    ///
    /// `flags` is a bitmask of `ESteamInputLEDFlag`; 0 means "set a colour". Steam restores the
    /// default on exit, so a caller does not need to put it back while shutting down
    /// (`isteaminput.h:962-963`).
    pub fn set_led_color(
        &self,
        controller: sys::InputHandle_t,
        red: u8,
        green: u8,
        blue: u8,
        flags: u32,
    ) {
        unsafe {
            sys::SteamAPI_ISteamInput_SetLEDColor(self.input, controller, red, green, blue, flags)
        }
    }

    /// Add an action set layer to a controller, on top of its active action set.
    pub fn activate_action_set_layer(
        &self,
        controller: sys::InputHandle_t,
        layer: sys::InputActionSetHandle_t,
    ) {
        unsafe { sys::SteamAPI_ISteamInput_ActivateActionSetLayer(self.input, controller, layer) }
    }

    /// Remove one action set layer.
    pub fn deactivate_action_set_layer(
        &self,
        controller: sys::InputHandle_t,
        layer: sys::InputActionSetHandle_t,
    ) {
        unsafe { sys::SteamAPI_ISteamInput_DeactivateActionSetLayer(self.input, controller, layer) }
    }

    /// Remove every action set layer at once.
    pub fn deactivate_all_action_set_layers(&self, controller: sys::InputHandle_t) {
        unsafe { sys::SteamAPI_ISteamInput_DeactivateAllActionSetLayers(self.input, controller) }
    }

    /// Enumerate the currently active layers.
    ///
    /// Steam documents the out array as `STEAM_INPUT_MAX_ACTIVE_LAYERS` sized and returns "the
    /// number of handles written"; this wrapper allocates exactly that and clamps Steam's count to
    /// it, so an over-reported number cannot read past the array.
    pub fn active_action_set_layers(
        &self,
        controller: sys::InputHandle_t,
    ) -> Vec<sys::InputActionSetHandle_t> {
        let capacity = sys::STEAM_INPUT_MAX_ACTIVE_LAYERS as usize;
        let mut handles = vec![0u64; capacity];
        let written = unsafe {
            sys::SteamAPI_ISteamInput_GetActiveActionSetLayers(
                self.input,
                controller,
                handles.as_mut_ptr(),
            )
        };
        handles.truncate(layers_to_take(written, capacity));
        handles
    }

    /// The action set currently active on a controller, as Steam reports it.
    pub fn current_action_set(
        &self,
        controller: sys::InputHandle_t,
    ) -> sys::InputActionSetHandle_t {
        unsafe { sys::SteamAPI_ISteamInput_GetCurrentActionSet(self.input, controller) }
    }

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

#[cfg(test)]
mod input_tests {
    use super::*;

    #[test]
    fn the_layer_list_never_reads_past_the_array_steam_fills() {
        let capacity = sys::STEAM_INPUT_MAX_ACTIVE_LAYERS as usize;
        assert_eq!(
            capacity, 16,
            "the bound this wrapper allocates is Steam's own constant"
        );
        assert_eq!(layers_to_take(3, capacity), 3);
        assert_eq!(layers_to_take(16, capacity), 16);
        assert_eq!(
            layers_to_take(40, capacity),
            16,
            "Steam's written count is clamped to the array it was given"
        );
        assert_eq!(
            layers_to_take(-1, capacity),
            0,
            "a negative count must not wrap"
        );
    }

    #[test]
    fn an_absent_binding_revision_is_none_not_a_zero_pair() {
        // A false answer drops the pair, because Steam may not have written either out
        // parameter at all. Even the all-zero pair is refused, so a caller cannot mistake an
        // unanswered query for "revision 0.0".
        assert_eq!(binding_revision(false, 0, 0), None);
        assert_eq!(binding_revision(false, 5, 2), None);
        assert_eq!(binding_revision(true, 5, 2), Some((5, 2)));
        assert_eq!(binding_revision(true, 0, 0), Some((0, 0)));
    }
}
