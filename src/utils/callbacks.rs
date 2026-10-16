//! The two notifications `ISteamUtils` posts -- a gamepad text input was dismissed, a
//! floating one was -- and the corner vocabulary an overlay notification can be pinned to.

use super::*;

#[derive(Clone, Debug)]
pub struct GamepadTextInputDismissed {
    pub submitted_text_len: Option<u32>,
}

impl_callback!(cb: GamepadTextInputDismissed_t => GamepadTextInputDismissed {
    Self {
        submitted_text_len: cb.m_bSubmitted.then_some(cb.m_unSubmittedText),
    }
});

#[derive(Clone, Debug)]
pub struct FloatingGamepadTextInputDismissed;

impl_callback!(_cb: FloatingGamepadTextInputDismissed_t => FloatingGamepadTextInputDismissed {
    Self
});

pub enum NotificationPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}
