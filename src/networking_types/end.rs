//! Why a connection ended.
//!
//! Steam's end reasons come in two numbered families -- the debug codes it uses internally and
//! the application codes a game sends -- and both arrive as plain `i32` on the wire. The mapping
//! tables are here rather than in the module root so the table grows without dragging every
//! other networking type through the diff.

use super::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct AppNetConnectionEnd {
    code: i32,
}

impl AppNetConnectionEnd {
    pub fn code(&self) -> i32 {
        self.code
    }

    /// Create a generic normal net connection end.
    pub fn generic_normal() -> Self {
        Self {
            code: ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_App_Generic as i32,
        }
    }

    /// Create a normal net connection end.
    ///
    /// Indicates that the application ended the connection in a "usual" manner.
    /// E.g.: user intentionally disconnected from the server,
    /// gameplay ended normally, etc
    ///
    /// Panics if `code` is not between 1000 and 1999.
    pub fn normal(code: i32) -> Self {
        let min = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_App_Min as i32;
        let max = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_App_Max as i32;
        if code < min || code > max {
            panic!("app net connection end code {} is out of range. App normal codes must be between {} and {}", code, min, max);
        }

        Self { code }
    }

    /// Creates a generic exception net connection end code.
    pub fn generic_exception() -> Self {
        Self {
            code: ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_AppException_Generic as i32,
        }
    }

    /// Create a exception net connection end.
    ///
    /// Indicates that the application ended the connection in some sort of exceptional
    /// or unusual manner that might indicate a bug or configuration
    /// issue.
    ///
    /// Panics if `code` is not between 2000 and 2999.
    pub fn exception(code: i32) -> Self {
        let min = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_AppException_Min as i32;
        let max = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_AppException_Max as i32;
        if code < min || code > max {
            panic!("app net connection end code {} is out of range. App exceptions code must be between {} and {}", code, min, max);
        }

        Self { code }
    }

    /// Returns true if the connection ended in a "normal" way, like a user intentionally disconnecting.
    pub fn is_normal(&self) -> bool {
        let min = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_App_Min as i32;
        let max = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_App_Max as i32;

        self.code >= min && self.code <= max
    }

    /// Returns true if the connection ended because of an error.
    pub fn is_exception(&self) -> bool {
        let min = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_AppException_Min as i32;
        let max = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_AppException_Max as i32;

        self.code >= min && self.code <= max
    }
}

/// Enumerate various causes of connection termination.  These are designed to work similar
/// to HTTP error codes: the numeric range gives you a rough classification as to the source
/// of the problem.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum NetConnectionEnd {
    /// Invalid/sentinel value
    ///
    /// Int value: 0
    Invalid,

    // Application codes.  These are the values you will pass to
    // ISteamNetworkingSockets::CloseConnection.  You can use these codes if
    // you want to plumb through application-specific reason codes.  If you don't
    // need this facility, feel free to always pass
    // k_ESteamNetConnectionEnd_App_Generic.
    //
    // The distinction between "normal" and "exceptional" termination is
    // one you may use if you find useful, but it's not necessary for you
    // to do so.  The only place where we distinguish between normal and
    // exceptional is in connection analytics.  If a significant
    // proportion of connections terminates in an exceptional manner,
    // this can trigger an alert.
    //
    // 1xxx: Application ended the connection in a "usual" manner.
    //       E.g.: user intentionally disconnected from the server,
    //             gameplay ended normally, etc
    App(AppNetConnectionEnd),

    //
    // System codes.  These will be returned by the system when
    // the connection state is k_ESteamNetworkingConnectionState_ClosedByPeer
    // or k_ESteamNetworkingConnectionState_ProblemDetectedLocally.  It is
    // illegal to pass a code in this range to ISteamNetworkingSockets::CloseConnection
    //

    // You cannot do what you want to do because you're running in offline mode.
    LocalOfflineMode,

    // We're having trouble contacting many (perhaps all) relays.
    // Since it's unlikely that they all went offline at once, the best
    // explanation is that we have a problem on our end.  Note that we don't
    // bother distinguishing between "many" and "all", because in practice,
    // it takes time to detect a connection problem, and by the time
    // the connection has timed out, we might not have been able to
    // actively probe all of the relay clusters, even if we were able to
    // contact them at one time.  So this code just means that:
    //
    // * We don't have any recent successful communication with any relay.
    // * We have evidence of recent failures to communicate with multiple relays.
    LocalManyRelayConnectivity,

    // A hosted server is having trouble talking to the relay
    // that the client was using, so the problem is most likely
    // on our end
    LocalHostedServerPrimaryRelay,

    // We're not able to get the network config.  This is
    // *almost* always a local issue, since the network config
    // comes from the CDN, which is pretty darn reliable.
    LocalNetworkConfig,

    // Steam rejected our request because we don't have rights
    // to do this.
    LocalRights,

    // ICE P2P rendezvous failed because we were not able to
    // determine our "public" address (e.g. reflexive address via STUN)
    //
    // If relay fallback is available (it always is on Steam), then
    // this is only used internally and will not be returned as a high
    // level failure.
    LocalP2PICENoPublicAddresses,

    // 4xxx: Connection failed or ended, and it appears that the
    //       cause does NOT have to do with the local host or their
    //       connection to the Internet.  It could be caused by the
    //       remote host, or it could be somewhere in between.

    // The connection was lost, and as far as we can tell our connection
    // to relevant services (relays) has not been disrupted.  This doesn't
    // mean that the problem is "their fault", it just means that it doesn't
    // appear that we are having network issues on our end.
    RemoteTimeout,

    // Something was invalid with the cert or crypt handshake
    // info you gave me, I don't understand or like your key types,
    // etc.
    RemoteBadEncrypt,

    // You presented me with a cert that was I was able to parse
    // and *technically* we could use encrypted communication.
    // But there was a problem that prevents me from checking your identity
    // or ensuring that somebody int he middle can't observe our communication.
    // E.g.: - the CA key was missing (and I don't accept unsigned certs)
    // - The CA key isn't one that I trust,
    // - The cert doesn't was appropriately restricted by app, user, time, data center, etc.
    // - The cert wasn't issued to you.
    // - etc
    RemoteBadCert,

    // Something wrong with the protocol version you are using.
    // (Probably the code you are running is too old.)
    RemoteBadProtocolVersion,

    // NAT punch failed failed because we never received any public
    // addresses from the remote host.  (But we did receive some
    // signals form them.)
    //
    // If relay fallback is available (it always is on Steam), then
    // this is only used internally and will not be returned as a high
    // level failure.
    RemoteP2PICENoPublicAddresses,

    // A failure that isn't necessarily the result of a software bug,
    // but that should happen rarely enough that it isn't worth specifically
    // writing UI or making a localized message for.
    // The debug string should contain further details.
    MiscGeneric,

    // Generic failure that is most likely a software bug.
    MiscInternalError,

    // The connection to the remote host timed out, but we
    // don't know if the problem is on our end, in the middle,
    // or on their end.
    MiscTimeout,

    // There's some trouble talking to Steam.
    MiscSteamConnectivity,

    // A server in a dedicated hosting situation has no relay sessions
    // active with which to talk back to a client.  (It's the client's
    // job to open and maintain those sessions.)
    MiscNoRelaySessionsToClient,

    // While trying to initiate a connection, we never received
    // *any* communication from the peer.
    //k_ESteamNetConnectionEnd_Misc_ServerNeverReplied = 5007,

    // P2P rendezvous failed in a way that we don't have more specific
    // information
    MiscP2PRendezvous,

    // NAT punch failed, probably due to NAT/firewall configuration.
    //
    // If relay fallback is available (it always is on Steam), then
    // this is only used internally and will not be returned as a high
    // level failure.
    MiscP2PNATFirewall,

    // Our peer replied that it has no record of the connection.
    // This should not happen ordinarily, but can happen in a few
    // exception cases:
    //
    // - This is an old connection, and the peer has already cleaned
    //   up and forgotten about it.  (Perhaps it timed out and they
    //   closed it and were not able to communicate this to us.)
    // - A bug or internal protocol error has caused us to try to
    //   talk to the peer about the connection before we received
    //   confirmation that the peer has accepted the connection.
    // - The peer thinks that we have closed the connection for some
    //   reason (perhaps a bug), and believes that is it is
    //   acknowledging our closure.
    MiscPeerSentNoConnection,

    /// A code that could not be handled.
    Other(i32),
}

impl From<NetConnectionEnd> for i32 {
    fn from(end: NetConnectionEnd) -> Self {
        use ESteamNetConnectionEnd::*;
        match end {
            NetConnectionEnd::Invalid => k_ESteamNetConnectionEnd_Invalid as i32,
            NetConnectionEnd::App(app_net_connection_end) => app_net_connection_end.code(),
            NetConnectionEnd::LocalOfflineMode => k_ESteamNetConnectionEnd_Local_OfflineMode as i32,
            NetConnectionEnd::LocalManyRelayConnectivity => {
                k_ESteamNetConnectionEnd_Local_ManyRelayConnectivity as i32
            }
            NetConnectionEnd::LocalHostedServerPrimaryRelay => {
                k_ESteamNetConnectionEnd_Local_HostedServerPrimaryRelay as i32
            }
            NetConnectionEnd::LocalNetworkConfig => {
                k_ESteamNetConnectionEnd_Local_NetworkConfig as i32
            }
            NetConnectionEnd::LocalRights => k_ESteamNetConnectionEnd_Local_Rights as i32,
            NetConnectionEnd::LocalP2PICENoPublicAddresses => {
                k_ESteamNetConnectionEnd_Local_P2P_ICE_NoPublicAddresses as i32
            }
            NetConnectionEnd::RemoteTimeout => k_ESteamNetConnectionEnd_Remote_Timeout as i32,
            NetConnectionEnd::RemoteBadEncrypt => k_ESteamNetConnectionEnd_Remote_BadCrypt as i32,
            NetConnectionEnd::RemoteBadCert => k_ESteamNetConnectionEnd_Remote_BadCert as i32,
            NetConnectionEnd::RemoteBadProtocolVersion => {
                k_ESteamNetConnectionEnd_Remote_BadProtocolVersion as i32
            }
            NetConnectionEnd::RemoteP2PICENoPublicAddresses => {
                k_ESteamNetConnectionEnd_Remote_P2P_ICE_NoPublicAddresses as i32
            }
            NetConnectionEnd::MiscGeneric => k_ESteamNetConnectionEnd_Misc_Generic as i32,
            NetConnectionEnd::MiscInternalError => {
                k_ESteamNetConnectionEnd_Misc_InternalError as i32
            }
            NetConnectionEnd::MiscTimeout => k_ESteamNetConnectionEnd_Misc_Timeout as i32,
            NetConnectionEnd::MiscSteamConnectivity => {
                k_ESteamNetConnectionEnd_Misc_SteamConnectivity as i32
            }
            NetConnectionEnd::MiscNoRelaySessionsToClient => {
                k_ESteamNetConnectionEnd_Misc_NoRelaySessionsToClient as i32
            }
            NetConnectionEnd::MiscP2PRendezvous => {
                k_ESteamNetConnectionEnd_Misc_P2P_Rendezvous as i32
            }
            NetConnectionEnd::MiscP2PNATFirewall => {
                k_ESteamNetConnectionEnd_Misc_P2P_NAT_Firewall as i32
            }
            NetConnectionEnd::MiscPeerSentNoConnection => {
                k_ESteamNetConnectionEnd_Misc_PeerSentNoConnection as i32
            }
            NetConnectionEnd::Other(code) => code,
        }
    }
}

impl From<i32> for NetConnectionEnd {
    fn from(end: i32) -> Self {
        use ESteamNetConnectionEnd::*;
        match end {
            end if end == k_ESteamNetConnectionEnd_Invalid as i32 => NetConnectionEnd::Invalid,
            end if end >= k_ESteamNetConnectionEnd_App_Min as i32
                && end <= k_ESteamNetConnectionEnd_AppException_Max as i32 =>
            {
                NetConnectionEnd::App(AppNetConnectionEnd { code: end })
            }
            end if end == k_ESteamNetConnectionEnd_Local_OfflineMode as i32 => {
                NetConnectionEnd::LocalOfflineMode
            }
            end if end == k_ESteamNetConnectionEnd_Local_ManyRelayConnectivity as i32 => {
                NetConnectionEnd::LocalManyRelayConnectivity
            }
            end if end == k_ESteamNetConnectionEnd_Local_HostedServerPrimaryRelay as i32 => {
                NetConnectionEnd::LocalHostedServerPrimaryRelay
            }
            end if end == k_ESteamNetConnectionEnd_Local_NetworkConfig as i32 => {
                NetConnectionEnd::LocalNetworkConfig
            }
            end if end == k_ESteamNetConnectionEnd_Local_Rights as i32 => {
                NetConnectionEnd::LocalRights
            }
            end if end == k_ESteamNetConnectionEnd_Local_P2P_ICE_NoPublicAddresses as i32 => {
                NetConnectionEnd::LocalP2PICENoPublicAddresses
            }
            end if end == k_ESteamNetConnectionEnd_Remote_Timeout as i32 => {
                NetConnectionEnd::RemoteTimeout
            }
            end if end == k_ESteamNetConnectionEnd_Remote_BadCrypt as i32 => {
                NetConnectionEnd::RemoteBadEncrypt
            }
            end if end == k_ESteamNetConnectionEnd_Remote_BadCert as i32 => {
                NetConnectionEnd::RemoteBadCert
            }
            end if end == k_ESteamNetConnectionEnd_Remote_BadProtocolVersion as i32 => {
                NetConnectionEnd::RemoteBadProtocolVersion
            }
            end if end == k_ESteamNetConnectionEnd_Remote_P2P_ICE_NoPublicAddresses as i32 => {
                NetConnectionEnd::RemoteP2PICENoPublicAddresses
            }
            end if end == k_ESteamNetConnectionEnd_Misc_Generic as i32 => {
                NetConnectionEnd::MiscGeneric
            }
            end if end == k_ESteamNetConnectionEnd_Misc_InternalError as i32 => {
                NetConnectionEnd::MiscInternalError
            }
            end if end == k_ESteamNetConnectionEnd_Misc_Timeout as i32 => {
                NetConnectionEnd::MiscTimeout
            }
            end if end == k_ESteamNetConnectionEnd_Misc_SteamConnectivity as i32 => {
                NetConnectionEnd::MiscSteamConnectivity
            }
            end if end == k_ESteamNetConnectionEnd_Misc_NoRelaySessionsToClient as i32 => {
                NetConnectionEnd::MiscNoRelaySessionsToClient
            }
            end if end == k_ESteamNetConnectionEnd_Misc_P2P_Rendezvous as i32 => {
                NetConnectionEnd::MiscP2PRendezvous
            }
            end if end == k_ESteamNetConnectionEnd_Misc_P2P_NAT_Firewall as i32 => {
                NetConnectionEnd::MiscP2PNATFirewall
            }
            end if end == k_ESteamNetConnectionEnd_Misc_PeerSentNoConnection as i32 => {
                NetConnectionEnd::MiscPeerSentNoConnection
            }
            end => Self::Other(end),
        }
    }
}

impl From<ESteamNetConnectionEnd> for NetConnectionEnd {
    fn from(end: ESteamNetConnectionEnd) -> Self {
        (end as i32).into()
    }
}

#[cfg(test)]
mod end_code_tests {
    //! The table that turns Steam's plain `int` end code into a variant. It matters that the table is
    //! `From` and not `TryFrom`: `m_eEndReason` arrives as a number Steam chose, so every input has to
    //! produce an answer, and the last arm has to be a real variant rather than an abort. Nothing in
    //! the crate judged that table before, and a reader of
    //! `NetConnectionInfo::end_reason` could easily believe the conversion can fail.
    use super::{AppNetConnectionEnd, ESteamNetConnectionEnd, NetConnectionEnd};

    #[test]
    fn zero_is_the_sentinel_steam_names_it() {
        assert_eq!(
            NetConnectionEnd::from(ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_Invalid as i32),
            NetConnectionEnd::Invalid
        );
    }

    /// 1xxx and 2xxx are the application families, and the number is the payload -- so a code in either
    /// range must come back as the same number, not as a neighbouring variant at the boundary.
    #[test]
    fn both_application_ranges_keep_their_own_number() {
        let app_min = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_App_Min as i32;
        let app_max = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_App_Max as i32;
        let exc_min = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_AppException_Min as i32;
        let exc_max = ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_AppException_Max as i32;
        for code in [app_min, app_max, exc_min, exc_max] {
            let expected = if code <= app_max {
                AppNetConnectionEnd::normal(code)
            } else {
                AppNetConnectionEnd::exception(code)
            };
            assert_eq!(
                NetConnectionEnd::from(code),
                NetConnectionEnd::App(expected)
            );
        }
    }

    #[test]
    fn a_named_system_code_becomes_the_variant_it_names() {
        assert_eq!(
            NetConnectionEnd::from(
                ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_Remote_Timeout as i32
            ),
            NetConnectionEnd::RemoteTimeout
        );
    }

    /// A code nobody has defined is not a crisis: it is kept as `Other`, which is why the conversion can
    /// be total and why no caller of `end_reason` should be handed a panic for it.
    #[test]
    fn an_undefined_code_survives_as_other() {
        assert_eq!(NetConnectionEnd::from(4242), NetConnectionEnd::Other(4242));
        assert_eq!(NetConnectionEnd::from(-7), NetConnectionEnd::Other(-7));
        assert_eq!(
            NetConnectionEnd::from(i32::MAX),
            NetConnectionEnd::Other(i32::MAX)
        );
    }

    /// Round trip over the whole classification: sentinel, both app boundaries, a system code, and a
    /// code from each side of every family. `from` then `into` must return the number Steam sent.
    #[test]
    fn every_family_round_trips_through_its_number() {
        let codes = [
            0,
            ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_App_Min as i32,
            ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_App_Max as i32,
            ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_AppException_Min as i32,
            ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_AppException_Max as i32,
            ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_Remote_Timeout as i32,
            ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_Misc_Generic as i32,
            999,
            3000,
            4242,
            -7,
        ];
        for code in codes {
            assert_eq!(
                i32::from(NetConnectionEnd::from(code)),
                code,
                "code {}",
                code
            );
        }
    }

    /// Anti-vacuity for the total-conversion claim above: `i32::MAX` is not a code in any family, and if
    /// the fallback arm were ever dropped the compiler -- not a test -- would be what refuses it.
    #[test]
    fn the_conversion_is_a_from_not_a_try_from() {
        let total: NetConnectionEnd = 4242.into();
        assert_eq!(total, NetConnectionEnd::Other(4242));
    }
}
