//! Small answer types that are neither callbacks nor tickets: the licence check and the
//! market-eligibility verdict.

use super::*;

/// Results from [`User::user_has_license_for_app`]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UserHasLicense {
    /// The user has a license for the specified app.
    HasLicense,
    /// Steam answered with a result this binding does not name; see
    /// [`AuthSessionError::Unknown`] for why the arm exists and why it is not tested.
    Unknown,
    /// The user does not have a license for the specified app.
    DoesNotHaveLicense,
    /// The user has not been authenticated.
    NoAuth,
}

/// Steam's answer to [`User::market_eligibility`].
///
/// The response carries no result code -- `MarketEligibilityResponse_t` is the callback id plus the
/// five fields below (`isteamuser.h:385-394`) -- so there is no failure this type could represent:
/// Steam says "not allowed" through [`MarketEligibility::allowed`] and its reason bitfield.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarketEligibility {
    /// `isteamuser.h:388` declares the field as "bool m_bAllowed;" and puts no comment on it,
    /// so this reads it as it names it: whether the account may use the Community Market.
    pub allowed: bool,
    /// Steam's own reason bitfield -- `isteamuser.h:389` declares it as "EMarketNotAllowedReasonFlags
    /// m_eNotAllowedReason;" -- forwarded unchanged.
    ///
    /// `EMarketNotAllowedReasonFlags` arrives from bindgen as a `#[repr(transparent)]` newtype over
    /// `c_int` with associated constants, not a Rust enum, and the header writes the values as
    /// `(1 << 0)`..`(1 << 3)` under a name ending in `Flags` (`steamclientpublic.h:362-377`:
    /// TemporaryFailure, AccountDisabled, AccountLockedDown, AccountLimited; `None = 0`). Steam's
    /// vocabulary therefore lets reasons combine, and a Rust mirror would either drop an unknown
    /// bit or present two reasons as one. The bits and the named constants are on
    /// `sys::EMarketNotAllowedReasonFlags`.
    pub not_allowed_reason: sys::EMarketNotAllowedReasonFlags,
    /// `m_rtAllowedAtTime`, a Steam `RTime32` (the bindings define `pub type RTime32 = uint32`).
    /// The header gives this field no comment (`isteamuser.h:390`), so it is forwarded as a raw
    /// number and no meaning -- including "0 means nothing was said" -- is invented for it.
    pub allowed_at: u32,
    /// "The number of days any user is required to have had Steam Guard before they can use the
    /// market" (`isteamuser.h:392`).
    pub steam_guard_required_days: i32,
    /// "The number of days after initial device authorization a user must wait before using the
    /// market on that device" (`isteamuser.h:393`).
    pub new_device_cooldown_days: i32,
}

impl MarketEligibility {
    pub(crate) fn from_raw(raw: &sys::MarketEligibilityResponse_t) -> Self {
        Self {
            allowed: raw.m_bAllowed,
            not_allowed_reason: raw.m_eNotAllowedReason,
            allowed_at: raw.m_rtAllowedAtTime,
            steam_guard_required_days: raw.m_cdaySteamGuardRequiredDays,
            new_device_cooldown_days: raw.m_cdayNewDeviceCooldown,
        }
    }
}
