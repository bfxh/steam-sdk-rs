//! The overlay's own state and geometry: whether it wants a present, and where a notification
//! is pinned and inset.

use super::*;

impl Utils {
    /// Returns whether or not the overlay is enabled in Steam
    pub fn is_overlay_enabled(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_IsOverlayEnabled(self.utils) }
    }

    /// Sets the position on the screen where popups from the steam overlay
    /// should appear and display themselves in.
    pub fn set_overlay_notification_position(&self, position: NotificationPosition) {
        unsafe {
            let position = match position {
                NotificationPosition::TopLeft => sys::ENotificationPosition::k_EPositionTopLeft,
                NotificationPosition::TopRight => sys::ENotificationPosition::k_EPositionTopRight,
                NotificationPosition::BottomLeft => {
                    sys::ENotificationPosition::k_EPositionBottomLeft
                }
                NotificationPosition::BottomRight => {
                    sys::ENotificationPosition::k_EPositionBottomRight
                }
            };
            sys::SteamAPI_ISteamUtils_SetOverlayNotificationPosition(self.utils, position);
        }
    }

    /// Whether the overlay still has something to present.
    ///
    /// When this is true the game should keep presenting frames; a game that
    /// blocks without presenting will stall the overlay.
    pub fn overlay_needs_present(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_BOverlayNeedsPresent(self.utils) }
    }

    /// Offsets where overlay notifications appear, from the position set with
    /// [`Utils::set_overlay_notification_position`].
    ///
    /// This is per-game, and does nothing when called outside a game context.
    pub fn set_overlay_notification_inset(&self, horizontal: i32, vertical: i32) {
        unsafe {
            sys::SteamAPI_ISteamUtils_SetOverlayNotificationInset(self.utils, horizontal, vertical)
        }
    }
}
