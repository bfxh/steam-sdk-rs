//! Questions about the Steam client that is running right now: Big Picture, Steam Deck, VR
//! (running, and the headset-streaming switch in both directions), and the China launcher.

use super::*;

impl Utils {
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

    /// Whether Steam is running inside a VR session.
    pub fn is_steam_running_in_vr(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_IsSteamRunningInVR(self.utils) }
    }

    /// Whether the game is running from a Steam China launcher.
    pub fn is_steam_china_launcher(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_IsSteamChinaLauncher(self.utils) }
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
}
