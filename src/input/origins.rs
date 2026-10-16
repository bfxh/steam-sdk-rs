//! Action origins as *text and art*: the glyph and string Steam offers for an origin or an
//! `EXboxOrigin`, the two glyph formats, and the translation helpers that move between origin
//! spaces. Every read here goes through `steam_static_str`, because Steam hands back a pointer
//! that may be null.

use super::*;

/// Reads one of Steam's own static strings.
///
/// Several `ISteamInput` queries hand back a `const char *` that Steam owns -- the caller must
/// not free it -- and this wrapper cannot tell "an empty name" from "no pointer at all", so the
/// null case is reported as `None` instead of being walked into. `CStr::from_ptr(null)` is not a
/// delayed error: it is immediate undefined behaviour, which is why every one of these goes
/// through here rather than through a bare `from_ptr`.
///
/// Text that is not valid UTF-8 survives as lossy, because Steam's glyph paths and action names
/// are its own identifiers and dropping the whole answer over one bad byte would be worse.
fn steam_static_str(pointer: *const std::os::raw::c_char) -> Option<String> {
    if pointer.is_null() {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(pointer) }
            .to_string_lossy()
            .into_owned(),
    )
}

impl Input {
    /// The glyph path Steam suggests for an Xbox controller button (`isteaminput.h`).
    /// `None` means Steam handed back no string.
    pub fn glyph_for_xbox_origin(&self, origin: sys::EXboxOrigin) -> Option<String> {
        steam_static_str(unsafe {
            sys::SteamAPI_ISteamInput_GetGlyphForXboxOrigin(self.input, origin)
        })
    }

    /// The PNG glyph path for an action origin at the requested size.
    ///
    /// Steam's own note is that the returned path is a location on disk managed by Steam, and
    /// the returned pointer belongs to Steam -- copy the text, do not free the pointer.
    pub fn glyph_png_for_action_origin(
        &self,
        origin: sys::EInputActionOrigin,
        size: sys::ESteamInputGlyphSize,
        flags: u32,
    ) -> Option<String> {
        steam_static_str(unsafe {
            sys::SteamAPI_ISteamInput_GetGlyphPNGForActionOrigin(self.input, origin, size, flags)
        })
    }

    /// The SVG glyph path for an action origin.
    pub fn glyph_svg_for_action_origin(
        &self,
        origin: sys::EInputActionOrigin,
        flags: u32,
    ) -> Option<String> {
        steam_static_str(unsafe {
            sys::SteamAPI_ISteamInput_GetGlyphSVGForActionOrigin(self.input, origin, flags)
        })
    }

    /// The action name behind an analog handle, as the manifest spells it.
    pub fn string_for_analog_action_name(
        &self,
        action: sys::InputAnalogActionHandle_t,
    ) -> Option<String> {
        steam_static_str(unsafe {
            sys::SteamAPI_ISteamInput_GetStringForAnalogActionName(self.input, action)
        })
    }

    /// The action name behind a digital handle, as the manifest spells it.
    pub fn string_for_digital_action_name(
        &self,
        action: sys::InputDigitalActionHandle_t,
    ) -> Option<String> {
        steam_static_str(unsafe {
            sys::SteamAPI_ISteamInput_GetStringForDigitalActionName(self.input, action)
        })
    }

    /// The name Steam gives an Xbox button.
    pub fn string_for_xbox_origin(&self, origin: sys::EXboxOrigin) -> Option<String> {
        steam_static_str(unsafe {
            sys::SteamAPI_ISteamInput_GetStringForXboxOrigin(self.input, origin)
        })
    }

    /// Translate an origin to another controller type.
    ///
    /// `isteaminput.h:1003`: "for inputs not present on the other controller type this will return
    /// k_EInputActionOrigin_None", and passing `k_ESteamInputType_Unknown` asks Steam for the
    /// closest mapping it knows -- "if a Playstation 5 controller was released this function would
    /// return Playstation 4 origins". The answer is the raw binding enum, deliberately not
    /// collapsed into a Rust enum of our own: `EInputActionOrigin` is `#[non_exhaustive]`, so a
    /// newer Steam can name an origin this crate has never heard of, and forwarding it unchanged
    /// is the only answer that cannot lose or mislabel it.
    pub fn translate_action_origin(
        &self,
        destination: sys::ESteamInputType,
        origin: sys::EInputActionOrigin,
    ) -> sys::EInputActionOrigin {
        unsafe { sys::SteamAPI_ISteamInput_TranslateActionOrigin(self.input, destination, origin) }
    }

    /// The Steam Input origin for an Xbox button on a specific controller.
    ///
    /// Same reasoning as [`Input::translate_action_origin`]: the value is Steam's, forwarded.
    pub fn action_origin_from_xbox_origin(
        &self,
        controller: sys::InputHandle_t,
        origin: sys::EXboxOrigin,
    ) -> sys::EInputActionOrigin {
        unsafe {
            sys::SteamAPI_ISteamInput_GetActionOriginFromXboxOrigin(self.input, controller, origin)
        }
    }
}

impl Input {
    /// Returns the glyph for an input action
    pub fn get_glyph_for_action_origin(
        &self,
        action_origin: sys::EInputActionOrigin,
    ) -> Option<String> {
        steam_static_str(unsafe {
            sys::SteamAPI_ISteamInput_GetGlyphForActionOrigin_Legacy(self.input, action_origin)
        })
    }

    /// Returns the name of an input action
    pub fn get_string_for_action_origin(
        &self,
        action_origin: sys::EInputActionOrigin,
    ) -> Option<String> {
        steam_static_str(unsafe {
            sys::SteamAPI_ISteamInput_GetStringForActionOrigin(self.input, action_origin)
        })
    }
}

#[cfg(test)]
mod static_string_tests {
    use super::*;
    use std::ffi::CString;
    use std::os::raw::c_char;

    #[test]
    fn a_null_string_from_steam_is_none_rather_than_a_walk_into_nothing() {
        assert_eq!(steam_static_str(std::ptr::null::<c_char>()), None);
    }

    #[test]
    fn a_real_steam_string_comes_back_whole() {
        let text = CString::new("glyph_full.png").unwrap();
        assert_eq!(
            steam_static_str(text.as_ptr()),
            Some("glyph_full.png".to_owned())
        );
        let empty = CString::new("").unwrap();
        assert_eq!(steam_static_str(empty.as_ptr()), Some(String::new()));
    }

    #[test]
    fn invalid_utf8_survives_as_lossy_instead_of_vanishing() {
        // One bad byte must not cost the whole answer: these are Steam's own identifiers.
        let text = CString::new(vec![b'a', 0xff, b'b']).unwrap();
        let got = steam_static_str(text.as_ptr()).expect("a non-null pointer always answers");
        assert_eq!(got.as_bytes().first(), Some(&b'a'));
        assert_eq!(got.as_bytes().last(), Some(&b'b'));
        assert!(
            got.chars().count() == 3 && got != "ab",
            "the invalid byte becomes one replacement character, not silence: {got:?}"
        );
    }
}
