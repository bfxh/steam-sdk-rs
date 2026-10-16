//! Turning the interface on and off: `init` and `run_frame` (with the `explicitly_call_run_frame`
//! promise from `isteaminput.h`), the action manifest path, the binding panel, `shutdown`, and
//! the one call that makes Steam post device events at all.

use super::*;

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

    /// Turns on [`DeviceConnected`] and [`DeviceDisconnected`] callbacks.
    ///
    /// Nothing arrives until this is called, and the header promises more than the name
    /// (`isteaminput.h:852-854`): "Each controller that is already connected will generate a
    /// device connected callback when you enable them", so enabling gives you the current roster
    /// as events, not only the plug/unplug that follow. Receive them through
    /// [`Client::register_callback`].
    pub fn enable_device_callbacks(&self) {
        unsafe { sys::SteamAPI_ISteamInput_EnableDeviceCallbacks(self.input) }
    }

    /// Allows to load a specific Action Manifest File localy
    pub fn set_input_action_manifest_file_path(&self, path: &str) -> bool {
        let path = CString::new(path).unwrap();
        unsafe {
            sys::SteamAPI_ISteamInput_SetInputActionManifestFilePath(self.input, path.as_ptr())
        }
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
