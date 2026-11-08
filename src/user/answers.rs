//! Small answer types that are neither callbacks nor tickets: the licence check and the
//! market-eligibility verdict.

use super::*;

/// The answer to "may this user play this app", from `EUserHasLicenseForAppResult`.
///
/// Both interfaces that ask the question -- [`User::user_has_license_for_app`] and
/// [`Server::user_has_license`](crate::Server::user_has_license) -- return this one type through
/// the shared table `license_answer`, so the table exists once and is judged once.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum UserHasLicense {
    /// "User has a license for specified app" (`steamclientpublic.h:221`).
    HasLicense,
    /// The user does not have a license for the specified app.
    ///
    /// "User does not have a license for the specified app" (`steamclientpublic.h:222`).
    DoesNotHaveLicense,
    /// The user has not been authenticated.
    ///
    /// "User has not been authenticated" (`steamclientpublic.h:223`). Distinct from
    /// [`UserHasLicense::DoesNotHaveLicense`] on purpose: one says nobody checked this user, the
    /// other says they were checked and the answer was no.
    NoAuth,
    /// A number the generated layer has no name for. Kept as a value rather than folded into
    /// [`UserHasLicense::NoAuth`], because "not authenticated" is a fact about the user while this is
    /// a fact about the SDK build. Not reachable from anything the bindings declare today
    /// (`EUserHasLicenseForAppResult` names exactly the three above, in all four platform files), and
    /// constructing a fourth discriminant would be out of range rather than a test of the table -- so
    /// this arm carries no test, which is the same treatment the other future-only arms get.
    Unrecognized(i32),
}

/// The table, apart from the two calls that need a live interface.
pub(crate) fn license_answer(raw: sys::EUserHasLicenseForAppResult) -> UserHasLicense {
    use sys::EUserHasLicenseForAppResult::*;
    match raw {
        k_EUserHasLicenseResultHasLicense => UserHasLicense::HasLicense,
        k_EUserHasLicenseResultDoesNotHaveLicense => UserHasLicense::DoesNotHaveLicense,
        k_EUserHasLicenseResultNoAuth => UserHasLicense::NoAuth,
        other => UserHasLicense::Unrecognized(other as i32),
    }
}

/// Steam's answer to [`User::market_eligibility`].
///
/// The response carries no result code: `isteamuser.h:385` is "struct MarketEligibilityResponse_t"
/// and the body is the callback id plus the
/// five fields below (`isteamuser.h:385-394`) -- so there is no failure this type could represent:
/// Steam declines through [`MarketEligibility::allowed`] and its reason bitfield, not through a
/// result code.
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
    /// The header gives this field no comment -- `isteamuser.h:390` is all of "RTime32
    /// m_rtAllowedAtTime;" -- so it is forwarded as a raw
    /// number and no meaning -- including that a zero means nothing was said -- is invented for it.
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

#[cfg(test)]
mod license_tests {
    //! The licence table, judged without a client. Both interfaces that ask the question read through
    //! [`license_answer`], so this is the one place its answers can be checked.

    use super::{license_answer, UserHasLicense as License};
    use steamworks_sys as sys;
    use sys::EUserHasLicenseForAppResult as Raw;

    #[test]
    fn every_licence_answer_steam_declares_has_a_name() {
        let pairs = [
            (Raw::k_EUserHasLicenseResultHasLicense, License::HasLicense),
            (
                Raw::k_EUserHasLicenseResultDoesNotHaveLicense,
                License::DoesNotHaveLicense,
            ),
            (Raw::k_EUserHasLicenseResultNoAuth, License::NoAuth),
        ];
        for (raw, named) in pairs {
            assert_eq!(license_answer(raw), named, "{:?}", raw as i32);
        }
        // "not authenticated" and "no license" are different facts about the world, and a caller has
        // to be able to tell them apart.
        assert_ne!(License::NoAuth, License::DoesNotHaveLicense);
    }

    /// A premise pin rather than a lock on this crate's code: these are Steam's numbers, and if a
    /// regeneration ever reorders them the table above has to be re-read. Checked by renumbering one
    /// discriminant in the generated layer, which turns this red -- no change of mine can.
    #[test]
    fn the_licence_numbers_are_the_ones_the_header_gives() {
        assert_eq!(Raw::k_EUserHasLicenseResultHasLicense as i32, 0);
        assert_eq!(Raw::k_EUserHasLicenseResultDoesNotHaveLicense as i32, 1);
        assert_eq!(Raw::k_EUserHasLicenseResultNoAuth as i32, 2);
    }
}
