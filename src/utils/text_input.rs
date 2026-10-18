//! Typing on a controller: the three mode vocabularies (entry mode, line mode, floating
//! mode) with their write-side `From` directions, and the calls that show a box, read what
//! came back, report its length, and dismiss it again.

use super::*;
use crate::utils::try_c_string;

pub enum GamepadTextInputMode {
    Normal,
    Password,
}

impl From<GamepadTextInputMode> for sys::EGamepadTextInputMode {
    fn from(mode: GamepadTextInputMode) -> Self {
        match mode {
            GamepadTextInputMode::Normal => {
                sys::EGamepadTextInputMode::k_EGamepadTextInputModeNormal
            }
            GamepadTextInputMode::Password => {
                sys::EGamepadTextInputMode::k_EGamepadTextInputModePassword
            }
        }
    }
}

pub enum GamepadTextInputLineMode {
    SingleLine,
    MultipleLines,
}

impl From<GamepadTextInputLineMode> for sys::EGamepadTextInputLineMode {
    fn from(mode: GamepadTextInputLineMode) -> Self {
        match mode {
            GamepadTextInputLineMode::SingleLine => {
                sys::EGamepadTextInputLineMode::k_EGamepadTextInputLineModeSingleLine
            }
            GamepadTextInputLineMode::MultipleLines => {
                sys::EGamepadTextInputLineMode::k_EGamepadTextInputLineModeMultipleLines
            }
        }
    }
}

pub enum FloatingGamepadTextInputMode {
    SingleLine,
    MultipleLines,
    Email,
    Numeric,
}

impl From<FloatingGamepadTextInputMode> for sys::EFloatingGamepadTextInputMode {
    fn from(mode: FloatingGamepadTextInputMode) -> Self {
        match mode {
            FloatingGamepadTextInputMode::SingleLine => {
                sys::EFloatingGamepadTextInputMode::k_EFloatingGamepadTextInputModeModeSingleLine
            }
            FloatingGamepadTextInputMode::MultipleLines => {
                sys::EFloatingGamepadTextInputMode::k_EFloatingGamepadTextInputModeModeMultipleLines
            }
            FloatingGamepadTextInputMode::Email => {
                sys::EFloatingGamepadTextInputMode::k_EFloatingGamepadTextInputModeModeEmail
            }
            FloatingGamepadTextInputMode::Numeric => {
                sys::EFloatingGamepadTextInputMode::k_EFloatingGamepadTextInputModeModeNumeric
            }
        }
    }
}

impl Utils {
    /// Gets the gamepad text input from the Big Picture overlay.
    ///
    /// This must be called within the `show_gamepad_text_input` callback. Returns `Some(String)` if user submitted the
    /// text, and `None` otherwise
    pub fn get_entered_gamepad_text_input(
        &self,
        dismissed_data: &GamepadTextInputDismissed,
    ) -> Option<String> {
        unsafe {
            let len = dismissed_data.submitted_text_len?;
            let mut buf = vec![0u8; len as usize];

            sys::SteamAPI_ISteamUtils_GetEnteredGamepadTextInput(
                self.utils,
                buf.as_mut_ptr().cast(),
                len,
            )
            .then(|| String::from_utf8(buf).expect("Steamworks returned invalid UTF-8 string"))
        }
    }

    /// Activates the Big Picture text input dialog which only supports gamepad input.
    pub fn show_gamepad_text_input<F>(
        &self,
        input_mode: GamepadTextInputMode,
        input_line_mode: GamepadTextInputLineMode,
        description: &str,
        max_characters: u32,
        existing_text: Option<&str>,
        dismissed_cb: F,
    ) -> bool
    where
        F: FnMut(GamepadTextInputDismissed) + 'static + Send,
    {
        // Refused before the callback is registered, so a rejected call leaves no handler behind.
        let description = match try_c_string(description) {
            Some(description) => description,
            None => return false,
        };
        let existing_text = match existing_text.map(try_c_string) {
            None => None,
            Some(None) => return false,
            Some(Some(text)) => Some(text),
        };
        unsafe {
            std::mem::forget(register_callback(&self._inner, dismissed_cb));
            sys::SteamAPI_ISteamUtils_ShowGamepadTextInput(
                self.utils,
                input_mode.into(),
                input_line_mode.into(),
                description.as_ptr(),
                max_characters,
                existing_text
                    .as_ref()
                    .map(|s| s.as_ptr())
                    .unwrap_or(std::ptr::null()),
            )
        }
    }

    /// Opens a floating keyboard over the game content and sends OS keyboard keys directly to the game.
    ///
    /// The text field position is specified in pixels relative the origin of the game window and is used to
    /// position the floating keyboard in a way that doesn't cover the text field.
    ///
    /// Callback is triggered when user dismisses the text input
    pub fn show_floating_gamepad_text_input<F>(
        &self,
        keyboard_mode: FloatingGamepadTextInputMode,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        mut dismissed_cb: F,
    ) -> bool
    where
        F: FnMut() + 'static + Send, // TODO: Support FnOnce callbacks
    {
        unsafe {
            std::mem::forget(register_callback(
                &self._inner,
                move |_: FloatingGamepadTextInputDismissed| {
                    dismissed_cb();
                },
            ));
            sys::SteamAPI_ISteamUtils_ShowFloatingGamepadTextInput(
                self.utils,
                keyboard_mode.into(),
                x,
                y,
                width,
                height,
            )
        }
    }

    /// Length in bytes of the text the user entered in the last gamepad text
    /// input dialog, which is what
    /// [`Utils::get_entered_gamepad_text_input`] then reads.
    pub fn entered_gamepad_text_length(&self) -> u32 {
        unsafe { sys::SteamAPI_ISteamUtils_GetEnteredGamepadTextLength(self.utils) }
    }

    /// Closes an open Big Picture text input dialog.
    ///
    /// Returns whether a dialog was actually dismissed.
    pub fn dismiss_gamepad_text_input(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_DismissGamepadTextInput(self.utils) }
    }

    /// Closes an open floating gamepad text input dialog.
    ///
    /// Returns whether a dialog was actually dismissed.
    pub fn dismiss_floating_gamepad_text_input(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamUtils_DismissFloatingGamepadTextInput(self.utils) }
    }
}
