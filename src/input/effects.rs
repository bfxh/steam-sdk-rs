//! Things the wrapper asks the controller to do: vibration (plain and extended), simple haptic
//! events, killing analog momentum, and the LED colour.

use super::*;

impl Input {
    /// "Trigger a vibration event on supported controllers - Steam will translate these
    /// commands into haptic pulses for Steam" controllers (`isteaminput.h:953`).
    pub fn trigger_vibration(&self, controller: sys::InputHandle_t, left: u16, right: u16) {
        unsafe { sys::SteamAPI_ISteamInput_TriggerVibration(self.input, controller, left, right) }
    }

    /// The same, "including Xbox trigger impulse rumble" (`isteaminput.h:956`).
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

    /// "Send a haptic pulse, works on Steam Deck and Steam Controller devices"
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
}
