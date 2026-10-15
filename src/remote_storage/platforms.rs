//! The platform bit flags Steam accepts for sync targeting, and the conversion onto its raw
//! enum.

use super::*;

bitflags! {
    /// Platform flags used with [`SteamFile::set_sync_platforms`] to restrict
    /// syncing to specific operating systems.
    #[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
    #[repr(C)]
    #[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
    pub struct RemoteStoragePlatforms: u32 {
        const WINDOWS = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformWindows.0 as _;
        const MACOS = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformOSX.0 as _;
        const PS3 = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformPS3.0 as _;
        const LINUX = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformLinux.0 as _;
        const SWITCH = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformSwitch.0 as _;
        const ANDROID = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformAndroid.0 as _;
        const IOS = sys::ERemoteStoragePlatform::k_ERemoteStoragePlatformIOS.0 as _;
    }
}

impl From<RemoteStoragePlatforms> for sys::ERemoteStoragePlatform {
    fn from(platforms: RemoteStoragePlatforms) -> Self {
        sys::ERemoteStoragePlatform(platforms.bits() as _)
    }
}
