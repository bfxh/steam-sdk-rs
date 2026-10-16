//! The power state Steam reports for a device, and the raw byte that decodes into it.

use super::*;

/// Battery state as Steam reports it, with the "on AC power" sentinel resolved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryPower {
    /// Plugged in; Steam reports this as the sentinel 255 rather than a percentage.
    OnAcPower,
    /// Percentage remaining, 0-100 as Steam reports it.
    Percent(u8),
}

/// The FFI reports battery percent as a `u8` and uses 255 for "plugged in";
/// without this split a caller reading `Percent` would see 255%.
pub(crate) fn battery_power_from_raw(raw: u8) -> BatteryPower {
    match raw {
        255 => BatteryPower::OnAcPower,
        percent => BatteryPower::Percent(percent),
    }
}

impl Utils {
    /// Battery state of the machine Steam is running on.
    ///
    /// Steam reports "on AC power" with the sentinel value 255 rather than a
    /// percentage, so that case maps to [`BatteryPower::OnAcPower`] instead of
    /// leaking a 255% reading.
    pub fn battery_power(&self) -> BatteryPower {
        battery_power_from_raw(unsafe {
            sys::SteamAPI_ISteamUtils_GetCurrentBatteryPower(self.utils)
        })
    }
}
