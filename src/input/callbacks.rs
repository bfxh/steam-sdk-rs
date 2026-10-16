//! The two device notifications -- a controller appeared, a controller went away -- with the
//! `impl_callback!` each one registers behind, and the tests that pin their payloads and their
//! Steam callback ids.

use super::*;

/// A controller appeared. Needs [`Input::enable_device_callbacks`] before anything arrives.
///
/// The handle is Steam's own `InputHandle_t`, the same currency
/// [`Input::get_connected_controllers`] returns, so a callback handle can be compared with the
/// roster directly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceConnected {
    pub device: sys::InputHandle_t,
}

impl_callback!(cb: SteamInputDeviceConnected_t => DeviceConnected {
    DeviceConnected {
        device: cb.m_ulConnectedDeviceHandle,
    }
});

/// A controller went away. Needs [`Input::enable_device_callbacks`] before anything arrives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceDisconnected {
    pub device: sys::InputHandle_t,
}

impl_callback!(cb: SteamInputDeviceDisconnected_t => DeviceDisconnected {
    DeviceDisconnected {
        device: cb.m_ulDisconnectedDeviceHandle,
    }
});

/// Both device callbacks are one-field reads of a struct Steam writes, and the two field names
/// differ only by `Connected` / `Disconnected`. That is exactly the kind of pair a later edit
/// can cross-wire, so each direction is pinned against the binding struct itself -- no Steam
/// client needed.
#[cfg(test)]
mod device_callback_tests {
    use super::*;

    #[test]
    fn a_connected_event_carries_the_handle_steam_wrote() {
        let mut raw = sys::SteamInputDeviceConnected_t {
            m_ulConnectedDeviceHandle: 0x1234_5678_9abc_def0,
        };
        let got =
            unsafe { <DeviceConnected as Callback>::from_raw(&mut raw as *mut _ as *mut c_void) };
        assert_eq!(got.device, 0x1234_5678_9abc_def0);
    }

    #[test]
    fn a_disconnected_event_reads_its_own_field() {
        let mut raw = sys::SteamInputDeviceDisconnected_t {
            m_ulDisconnectedDeviceHandle: 7,
        };
        let got = unsafe {
            <DeviceDisconnected as Callback>::from_raw(&mut raw as *mut _ as *mut c_void)
        };
        assert_eq!(got.device, 7);
    }

    /// The two payload structs are one `u64` each, so swapping which Steam struct a payload
    /// decodes would still move the same bytes and read back the same number. The callback id is
    /// what actually distinguishes them, so that is what gets pinned: a crossed wire here would
    /// deliver plug events to the unplug handler and nothing else would notice.
    #[test]
    fn the_two_device_callbacks_keep_their_own_steam_callback_ids() {
        assert_eq!(
            <DeviceConnected as Callback>::ID,
            sys::SteamInputDeviceConnected_t_k_iCallback as i32
        );
        assert_eq!(
            <DeviceDisconnected as Callback>::ID,
            sys::SteamInputDeviceDisconnected_t_k_iCallback as i32
        );
        assert_ne!(
            <DeviceConnected as Callback>::ID,
            <DeviceDisconnected as Callback>::ID
        );
    }
}
