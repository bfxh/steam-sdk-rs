use super::*;

use std::ffi::CStr;
use std::os::raw::c_char;
use std::panic;
use std::process::abort;
use std::sync::RwLock;

mod battery;
mod callbacks;
mod client_state;
mod counts;
mod filtering;
mod identity;
mod ipv6;
mod locks;
mod overlay;
mod param_strings;
mod strings;
mod text_input;
mod warning;

pub use battery::*;
pub use callbacks::*;
pub(crate) use counts::*;
pub use filtering::*;
pub use ipv6::*;
pub(crate) use locks::*;
pub(crate) use param_strings::*;
pub(crate) use strings::*;
pub use text_input::*;

/// Access to the steam utils interface
pub struct Utils {
    pub(crate) utils: *mut sys::ISteamUtils,
    pub(crate) _inner: Arc<Inner>,
}

#[cfg(test)]
mod tests {
    use super::*;
    // Two converters live in the topic files that own the values they decode; the root's tests
    // name them by path now that the split moved them out of this module's scope.
    use super::filtering::filtered_text_from_buffer;
    use super::ipv6::ipv6_state_from_raw;

    #[test]
    fn ac_power_sentinel_is_not_reported_as_255_percent() {
        assert_eq!(battery_power_from_raw(255), BatteryPower::OnAcPower);
        assert_eq!(battery_power_from_raw(0), BatteryPower::Percent(0));
        assert_eq!(battery_power_from_raw(100), BatteryPower::Percent(100));
        // Anything under the sentinel stays a percentage, untouched.
        for raw in [1u8, 42, 100, 254] {
            assert_eq!(battery_power_from_raw(raw), BatteryPower::Percent(raw));
        }
    }

    #[test]
    fn ipv6_protocols_map_onto_distinct_steam_variants() {
        use sys::ESteamIPv6ConnectivityProtocol as S;
        let mapped = [
            S::from(IPv6ConnectivityProtocol::Http),
            S::from(IPv6ConnectivityProtocol::Udp),
        ];
        assert_ne!(mapped[0], mapped[1]);
    }

    #[test]
    fn ipv6_unknown_stays_unknown() {
        use sys::ESteamIPv6ConnectivityState as S;
        assert_eq!(
            ipv6_state_from_raw(S::k_ESteamIPv6ConnectivityState_Unknown),
            IPv6Connectivity::Unknown
        );
        assert_eq!(
            ipv6_state_from_raw(S::k_ESteamIPv6ConnectivityState_Good),
            IPv6Connectivity::Good
        );
        assert_eq!(
            ipv6_state_from_raw(S::k_ESteamIPv6ConnectivityState_Bad),
            IPv6Connectivity::Bad
        );
    }

    fn cbuf(text: &str, capacity: usize) -> Vec<std::os::raw::c_char> {
        let mut buf = vec![0 as std::os::raw::c_char; capacity];
        for (slot, byte) in buf.iter_mut().zip(text.as_bytes()) {
            *slot = *byte as std::os::raw::c_char;
        }
        buf
    }

    #[test]
    fn filter_output_is_cut_at_the_first_nul_not_read_past_it() {
        let ok = filtered_text_from_buffer(&cbuf("hi***", 8), 3).unwrap();
        assert_eq!(ok.text, "hi***");
        assert_eq!(ok.filtered_characters, 3);

        // Steam writes into a buffer it was given as strlen(input)+1; anything
        // after the terminator must not end up in the string.
        let padded = filtered_text_from_buffer(&cbuf("clean", 32), 0).unwrap();
        assert_eq!(padded.text, "clean");
        assert_eq!(padded.filtered_characters, 0);

        // A missing terminator must not run the read past the allocation.
        let unterminated: Vec<std::os::raw::c_char> =
            "abc".bytes().map(|b| b as std::os::raw::c_char).collect();
        assert_eq!(
            filtered_text_from_buffer(&unterminated, 1).unwrap().text,
            "abc"
        );
    }

    #[test]
    fn a_negative_filter_count_is_a_failure_not_zero_changes() {
        assert!(matches!(
            filtered_text_from_buffer(&cbuf("x", 2), -1),
            Err(SteamError::IOFailure)
        ));
        assert_eq!(
            filtered_text_from_buffer(&cbuf("x", 2), 0)
                .unwrap()
                .filtered_characters,
            0
        );
    }

    #[test]
    fn filter_contexts_map_onto_distinct_steam_variants() {
        use sys::ETextFilteringContext as S;
        let contexts = [
            TextFilteringContext::Unknown,
            TextFilteringContext::GameContent,
            TextFilteringContext::Chat,
            TextFilteringContext::Name,
        ];
        let mapped: Vec<S> = contexts.iter().copied().map(S::from).collect();
        for i in 0..mapped.len() {
            for j in (i + 1)..mapped.len() {
                assert_ne!(
                    mapped[i], mapped[j],
                    "{:?}/{:?} collide",
                    contexts[i], contexts[j]
                );
            }
        }
    }
}
