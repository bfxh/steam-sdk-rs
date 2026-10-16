//! What this app and this machine are: app id, IP country, UI language, server time, and the
//! two idle-seconds reads with the IPC counter beside them.

use super::*;

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
}
