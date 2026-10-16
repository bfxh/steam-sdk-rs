//! IPv6 reachability: the protocol a probe ran over, the state it came back with, and the
//! raw-to-Rust mapping that has to answer for values Steam may add later.

use super::*;

/// Which transport to ask about with [`Utils::ipv6_connectivity_state`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IPv6ConnectivityProtocol {
    Http,
    Udp,
}

impl From<IPv6ConnectivityProtocol> for sys::ESteamIPv6ConnectivityProtocol {
    fn from(protocol: IPv6ConnectivityProtocol) -> Self {
        match protocol {
            IPv6ConnectivityProtocol::Http => {
                sys::ESteamIPv6ConnectivityProtocol::k_ESteamIPv6ConnectivityProtocol_HTTP
            }
            IPv6ConnectivityProtocol::Udp => {
                sys::ESteamIPv6ConnectivityProtocol::k_ESteamIPv6ConnectivityProtocol_UDP
            }
        }
    }
}

/// Steam's belief about IPv6 reachability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IPv6Connectivity {
    Unknown,
    Good,
    Bad,
}

pub(super) fn ipv6_state_from_raw(raw: sys::ESteamIPv6ConnectivityState) -> IPv6Connectivity {
    match raw {
        sys::ESteamIPv6ConnectivityState::k_ESteamIPv6ConnectivityState_Good => {
            IPv6Connectivity::Good
        }
        sys::ESteamIPv6ConnectivityState::k_ESteamIPv6ConnectivityState_Bad => {
            IPv6Connectivity::Bad
        }
        // The binding is `#[non_exhaustive]`, and Steam's own Unknown answer
        // has to survive as Unknown rather than being read as Good or Bad.
        _ => IPv6Connectivity::Unknown,
    }
}

impl Utils {
    /// What Steam believes the machine's IPv6 connectivity to the internet is
    /// over `protocol`.
    ///
    /// Note what this does *not* say: it does not tell you whether the Steam
    /// client itself reached Steam over IPv6, and a game that needs a real
    /// answer about a specific host should probe it rather than trust this.
    pub fn ipv6_connectivity_state(&self, protocol: IPv6ConnectivityProtocol) -> IPv6Connectivity {
        ipv6_state_from_raw(unsafe {
            sys::SteamAPI_ISteamUtils_GetIPv6ConnectivityState(self.utils, protocol.into())
        })
    }
}
