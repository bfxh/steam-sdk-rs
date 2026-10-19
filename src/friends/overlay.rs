//! The Steam overlay: asking the client to open a dialog, a web page, the store (with the
//! `OverlayToStoreFlag` variant this crate models) or another user's profile, and the two
//! invite dialogs.

use super::*;
use crate::utils::{c_string_prefix, try_c_string};
use std::ffi::CString;

pub enum OverlayToStoreFlag {
    None = 0,
    AddToCart = 1,
    AddToCartAndShow = 2,
}

impl Friends {
    pub fn activate_game_overlay(&self, dialog: &str) {
        let dialog = c_string_prefix(dialog);
        unsafe {
            sys::SteamAPI_ISteamFriends_ActivateGameOverlay(self.friends, dialog.as_ptr());
        }
    }

    // I don't know why these are part of friends either
    pub fn activate_game_overlay_to_web_page(&self, url: &str) {
        unsafe {
            let url = c_string_prefix(url);
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayToWebPage(
                self.friends,
                url.as_ptr(),
                sys::EActivateGameOverlayToWebPageMode::k_EActivateGameOverlayToWebPageMode_Default,
            );
        }
    }

    pub fn activate_game_overlay_to_store(
        &self,
        app_id: AppId,
        overlay_to_store_flag: OverlayToStoreFlag,
    ) {
        unsafe {
            let overlay_to_store_flag = match overlay_to_store_flag {
                OverlayToStoreFlag::None => sys::EOverlayToStoreFlag::k_EOverlayToStoreFlag_None,
                OverlayToStoreFlag::AddToCart => {
                    sys::EOverlayToStoreFlag::k_EOverlayToStoreFlag_AddToCart
                }
                OverlayToStoreFlag::AddToCartAndShow => {
                    sys::EOverlayToStoreFlag::k_EOverlayToStoreFlag_AddToCartAndShow
                }
            };
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayToStore(
                self.friends,
                app_id.0,
                overlay_to_store_flag,
            );
        }
    }

    pub fn activate_game_overlay_to_user(&self, dialog: &str, user: SteamId) {
        let dialog = c_string_prefix(dialog);
        unsafe {
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayToUser(
                self.friends,
                dialog.as_ptr(),
                user.0,
            );
        }
    }

    /// Opens up an invite dialog for the given lobby
    pub fn activate_invite_dialog(&self, lobby: LobbyId) {
        unsafe {
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayInviteDialog(self.friends, lobby.0);
        }
    }

    /// Opens up an invite dialog that will send Rich Presence connect string to friends
    ///
    /// A connect string containing a nul byte is cut at that byte rather than crashing: Steam's
    /// parameter is a `const char *`, so nothing past the nul could have reached it.
    pub fn activate_invite_dialog_connect_string(&self, connect: &str) {
        let connect = c_string_prefix(connect);
        unsafe {
            sys::SteamAPI_ISteamFriends_ActivateGameOverlayInviteDialogConnectString(
                self.friends,
                connect.as_ptr(),
            );
        }
    }

    /// Registers a protocol (scheme) for the overlay browser to intercept instead of navigating to.
    ///
    /// Steam's own instruction is "Call this before calling ActivateGameOverlayToWebPage() to have
    /// the Steam Overlay Browser block navigations" (`isteamfriends.h:419`), and the next line names
    /// what arrives instead: "to your specified protocol (scheme) uris and instead dispatch a
    /// OverlayBrowserProtocolNavigation_t callback to your game" (`:420`). The call is `bool`
    /// (`:422`), so it has a channel and the two refusals are kept apart: `Err(InvalidParameter)` is
    /// this wrapper saying the string could not be expressed at all, `Ok(false)` is Steam declining
    /// a protocol it could have taken. Truncating at the nul instead -- the shape used by the void
    /// setters above -- would register a different scheme than the one the caller named and still
    /// report having done it.
    pub fn register_protocol_in_overlay_browser(&self, protocol: &str) -> SteamResult<bool> {
        let protocol = protocol_arg(protocol)?;
        Ok(unsafe {
            sys::SteamAPI_ISteamFriends_RegisterProtocolInOverlayBrowser(
                self.friends,
                protocol.as_ptr(),
            )
        })
    }
}

/// The scheme as a C string, or a refusal when it cannot be one.
fn protocol_arg(value: &str) -> SteamResult<CString> {
    try_c_string(value).ok_or(SteamError::InvalidParameter)
}

#[cfg(test)]
mod overlay_tests {
    //! Which strings reach Steam at all. Whether a running client accepts a given scheme needs a
    //! live client and is registered as unverified, along with the ordering the header insists on.

    use super::protocol_arg;
    use crate::SteamError;

    /// A scheme with an interior nul is not sent: the wrapper answers, so it is never mistaken for
    /// Steam's own refusal, and never half-registered.
    #[test]
    fn a_scheme_that_cannot_be_a_c_string_is_refused_before_the_call() {
        assert!(protocol_arg("mygame").is_ok());
        assert_eq!(
            protocol_arg("my\0game").err(),
            Some(SteamError::InvalidParameter),
            "an interior nul is this crate's refusal, not a truncation and not a panic"
        );
        assert!(
            protocol_arg("").is_ok(),
            "the empty scheme is expressible; Steam decides it"
        );
    }
}
