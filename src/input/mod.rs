use sys::InputHandle_t;

use super::*;

mod actions;
mod callbacks;
mod core;
mod effects;
mod layers;
mod origins;
mod roster;
mod session;

pub use callbacks::*;

/// Access to the steam input interface
pub struct Input {
    pub(crate) input: *mut sys::ISteamInput,
    pub(crate) _inner: Arc<Inner>,
}

pub enum InputType {
    Unknown,
    SteamController,
    XBox360Controller,
    XBoxOneController,
    GenericGamepad,
    PS4Controller,
    AppleMFiController,
    AndroidController,
    SwitchJoyConPair,
    SwitchJoyConSingle,
    SwitchProController,
    MobileTouch,
    PS3Controller,
    PS5Controller,
    SteamDeckController,
}

/// Steam's two-part answer about a device's loaded binding, or nothing at all.
///
/// Extracted so the test below exercises the same function the production path runs, rather than
/// restating `bool::then_some` in the test -- a test of std's behaviour would stay green however
/// the mapping broke.
fn binding_revision(ok: bool, major: i32, minor: i32) -> Option<(i32, i32)> {
    ok.then_some((major, minor))
}

/// How many layer handles this wrapper will read back: Steam's own count, never negative and
/// never more than the `STEAM_INPUT_MAX_ACTIVE_LAYERS` array it was handed.
fn layers_to_take(reported: i32, capacity: usize) -> usize {
    (reported.max(0) as usize).min(capacity)
}
