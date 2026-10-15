//! The two vocabularies an HTML input event names -- mouse buttons and key modifiers -- and
//! the mapping each one carries onto the raw Steam variant.

use super::*;

/// Which mouse button an HTML input event refers to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HtmlMouseButton {
    Left,
    Right,
    Middle,
}

impl HtmlMouseButton {
    // `pub(super)`: `browser.rs` maps an event onto the raw Steam variant, and the split left the
    // two vocabularies in a sibling file.
    pub(super) fn as_raw(self) -> sys::ISteamHTMLSurface_EHTMLMouseButton {
        match self {
            HtmlMouseButton::Left => sys::ISteamHTMLSurface_EHTMLMouseButton::eHTMLMouseButton_Left,
            HtmlMouseButton::Right => {
                sys::ISteamHTMLSurface_EHTMLMouseButton::eHTMLMouseButton_Right
            }
            HtmlMouseButton::Middle => {
                sys::ISteamHTMLSurface_EHTMLMouseButton::eHTMLMouseButton_Middle
            }
        }
    }
}

/// A keyboard modifier for HTML input events.
///
/// Deliberately a single choice, not a set: Steam's generated parameter type is
/// a `#[repr(i32)]` enum whose variants happen to carry bit values (1, 2, 4),
/// so a combination such as Shift+Ctrl has no valid discriminant in that type.
/// Building one by transmuting would create an invalid enum value, which is
/// undefined behaviour rather than merely a wrong number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum HtmlKeyModifier {
    #[default]
    None,
    Alt,
    Ctrl,
    Shift,
}

impl HtmlKeyModifier {
    pub(super) fn as_raw(self) -> sys::ISteamHTMLSurface_EHTMLKeyModifiers {
        match self {
            HtmlKeyModifier::None => {
                sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_None
            }
            HtmlKeyModifier::Alt => {
                sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_AltDown
            }
            HtmlKeyModifier::Ctrl => {
                sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_CtrlDown
            }
            HtmlKeyModifier::Shift => {
                sys::ISteamHTMLSurface_EHTMLKeyModifiers::k_eHTMLKeyModifier_ShiftDown
            }
        }
    }
}
