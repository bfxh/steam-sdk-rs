//! The four notifications a game server posts about players -- approved, denied, kicked and a
//! group status answer -- each with its `impl_callback!`, plus the `DenyReason` vocabulary the
//! denial carries. Four `pub struct`, four `impl_callback!` (measured with `grep -c`).

use super::*;

/// Called when a client has been approved to connect to this game server
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GSClientApprove {
    /// The steam ID of the user requesting a p2p session
    pub user: SteamId,
    /// Owner of the game, may be different from user when Family Sharing is being used
    pub owner: SteamId,
}

impl_callback!(cb: GSClientApprove_t => GSClientApprove {
    Self {
        user: SteamId(cb.m_SteamID.m_steamid.m_unAll64Bits),
        owner: SteamId(cb.m_OwnerSteamID.m_steamid.m_unAll64Bits),
    }
});

/// Reason for when a client fails to join or is kicked from a game server.
#[repr(i32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub enum DenyReason {
    Invalid,
    InvalidVersion,
    Generic,
    NotLoggedOn,
    NoLicense,
    Cheater,
    LoggedInElseWhere,
    UnknownText,
    IncompatibleAnticheat,
    MemoryCorruption,
    IncompatibleSoftware,
    SteamConnectionLost,
    SteamConnectionError,
    SteamResponseTimedOut,
    SteamValidationStalled,
    SteamOwnerLeftGuestUser,
}

impl From<sys::EDenyReason> for DenyReason {
    fn from(r: sys::EDenyReason) -> Self {
        match r {
            sys::EDenyReason::k_EDenyInvalid => DenyReason::Invalid,
            sys::EDenyReason::k_EDenyInvalidVersion => DenyReason::InvalidVersion,
            sys::EDenyReason::k_EDenyGeneric => DenyReason::Generic,
            sys::EDenyReason::k_EDenyNotLoggedOn => DenyReason::NotLoggedOn,
            sys::EDenyReason::k_EDenyNoLicense => DenyReason::NoLicense,
            sys::EDenyReason::k_EDenyCheater => DenyReason::Cheater,
            sys::EDenyReason::k_EDenyLoggedInElseWhere => DenyReason::LoggedInElseWhere,
            sys::EDenyReason::k_EDenyUnknownText => DenyReason::UnknownText,
            sys::EDenyReason::k_EDenyIncompatibleAnticheat => DenyReason::IncompatibleAnticheat,
            sys::EDenyReason::k_EDenyMemoryCorruption => DenyReason::MemoryCorruption,
            sys::EDenyReason::k_EDenyIncompatibleSoftware => DenyReason::IncompatibleSoftware,
            sys::EDenyReason::k_EDenySteamConnectionLost => DenyReason::SteamConnectionLost,
            sys::EDenyReason::k_EDenySteamConnectionError => DenyReason::SteamConnectionError,
            sys::EDenyReason::k_EDenySteamResponseTimedOut => DenyReason::SteamResponseTimedOut,
            sys::EDenyReason::k_EDenySteamValidationStalled => DenyReason::SteamValidationStalled,
            sys::EDenyReason::k_EDenySteamOwnerLeftGuestUser => DenyReason::SteamOwnerLeftGuestUser,
            _ => DenyReason::Invalid,
        }
    }
}

/// Called when a user has been denied to connection to this game server
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GSClientDeny {
    /// The steam ID of the user requesting a p2p session
    pub user: SteamId,
    pub deny_reason: DenyReason,
    pub optional_text: String,
}

impl_callback!(cb: GSClientDeny_t => GSClientDeny {
    let deny_text = unsafe {
        let cstr = CStr::from_ptr(cb.m_rgchOptionalText.as_ptr() as *const c_char);
        cstr.to_string_lossy().into_owned()
    };

    GSClientDeny {
        user: SteamId(cb.m_SteamID.m_steamid.m_unAll64Bits),
        deny_reason: DenyReason::from(cb.m_eDenyReason),
        optional_text: deny_text,
    }
});

/// Called when a user has been denied to connection to this game server
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GSClientKick {
    /// The steam ID of the user requesting a p2p session
    pub user: SteamId,
    pub deny_reason: DenyReason,
}

impl_callback!(cb: GSClientKick_t => GSClientKick {
    Self {
        user: SteamId(cb.m_SteamID.m_steamid.m_unAll64Bits),
        deny_reason: DenyReason::from(cb.m_eDenyReason),
    }
});

/// Called when we have received the group status of a user
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct GSClientGroupStatus {
    /// The steam ID of the user we queried
    pub user: SteamId,
    pub group: SteamId,
    pub member: bool,
    pub officer: bool,
}

impl_callback!(cb: GSClientGroupStatus_t => GSClientGroupStatus {
    Self {
        user: SteamId(cb.m_SteamIDUser.m_steamid.m_unAll64Bits),
        group: SteamId(cb.m_SteamIDGroup.m_steamid.m_unAll64Bits),
        member: cb.m_bMember,
        officer: cb.m_bOfficer,
    }
});
