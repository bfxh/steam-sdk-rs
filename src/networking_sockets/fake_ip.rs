//! The FakeIP allocation surface: ask Steam for a block of fake ports, read the assignment back,
//! open a listen socket on one of them *by index*, and take the completion message off the callback
//! pipe.
//!
//! Three of the four FakeIP entry points in `isteamnetworkingsockets.h` are here (h:862, h:867,
//! h:878). The fourth, `GetRemoteFakeIPForConnection` (h:895), is not blocked by the generated layer
//! any more than these three were: `SteamNetworkingIPAddr` binds with a real layout and already
//! converts at `src/networking_types/addresses.rs:183-187`. What blocks it is a public-type decision
//! -- every address-facing API in this crate speaks `std::net::SocketAddr` and keeps `SteamIpAddr`
//! private -- so it is registered as a design question rather than guessed. See the FakeIP paragraph
//! of `docs/SuperSteam-Design.md`.
//!
//! `ISteamNetworkingUtils::SetGlobalCallback_FakeIPResult` is deliberately not wrapped either. The
//! same completion payload (`SteamNetworkingFakeIPResult_t`, `steamnetworkingfakeip.h:93`) reaches
//! callers as [`FakeIpResult`] through this crate's installed-callback stream, and the SDK's global
//! callback takes a bare function pointer with nowhere to put a capture, so wrapping it would offer a
//! second, weaker route to a message that already arrives.

use super::*;
use crate::callback::Callback;
use crate::to_steam_result;
use std::ffi::c_void;
use std::net::Ipv4Addr;

impl NetworkingSockets {
    /// Asks Steam to reserve a block of FakeIP ports for this user or game server.
    ///
    /// Returns `false` if a request is *already in progress*, and `true` if a new one was started
    /// (`isteamnetworkingsockets.h:835-836`). It is a `bool` and not a `Result` because that is the
    /// only answer Steam gives and the two cases are both normal: nothing here has failed, there is
    /// simply already a reservation running.
    ///
    /// The number of ports is not bounded by anything this crate can name. The header states the
    /// limit only in prose -- `isteamnetworkingsockets.h:829-830`: allocations come from a global
    /// namespace and "there is a relatively strict limit on the maximum number of ports you may
    /// request. (At the time of this writing, the limit is 4.)" -- and no constant for it exists in
    /// the vendored headers or the bindings, so the value is passed through untouched rather than
    /// checked against a number this wrapper would have invented. Lines 831-832 warn that the
    /// assignments are *not* guaranteed to have any particular order or to be contiguous, so nothing here pretends port
    /// `i + 1` follows port `i`.
    ///
    /// For a game server this must be called after initialising the SDK but before logging in
    /// (`isteamnetworkingsockets.h:839-840`), and the request is queued until logon completes -- so
    /// calling it and then logging in is the documented order, not a race. Failure is reported only
    /// after several retries and can take minutes (`:845-849`); the header treats that as fatal for
    /// the application, and this wrapper adds no opinion to that.
    pub fn begin_async_request_fake_ip(&self, num_ports: i32) -> bool {
        unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_BeginAsyncRequestFakeIP(self.sockets, num_ports)
        }
    }

    /// Opens a listen socket that accepts P2P connections sent to one of *our* FakeIP addresses.
    ///
    /// `idx_fake_port` is the **index into the reservation**, not a port number: `:873-875` says to
    /// pass 0 for the first port of the block. Call [`begin_async_request_fake_ip`](Self::begin_async_request_fake_ip)
    /// first, but do not wait for it -- `:875-877` allows the socket to be created while the
    /// allocation is still pending, which is why this pair is reachable even though the completion
    /// message is not.
    ///
    /// A peer reaches this socket with `ConnectByIPAddress`, passing the FakeIP address; until Steam
    /// knows the real identity, the connection reports the FakeIP one
    /// (`isteamnetworkingsockets.h:852-857`).
    ///
    /// Like the other listen sockets, Steam's refusal is the invalid handle and it arrives as
    /// [`InvalidHandle`]; the options are handed over at call time, for the reason
    /// `SteamNetworkingConfigValue_t` documents.
    pub fn create_listen_socket_p2p_fake_ip(
        &self,
        idx_fake_port: i32,
        options: impl IntoIterator<Item = NetworkingConfigEntry>,
    ) -> Result<ListenSocket, InvalidHandle> {
        let options: Vec<_> = options.into_iter().map(|x| x.into()).collect();
        let handle = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_CreateListenSocketP2PFakeIP(
                self.sockets,
                idx_fake_port,
                options.len() as _,
                options.as_ptr(),
            )
        };
        if handle == sys::k_HSteamListenSocket_Invalid {
            Err(InvalidHandle)
        } else {
            Ok(ListenSocket::new(handle, self.sockets, self.inner.clone()))
        }
    }

    /// Reads back the FakeIP block Steam assigned, once the request has succeeded.
    ///
    /// No port index is offered as an argument on purpose: `isteamnetworkingsockets.h:865` documents
    /// `idxFirstPort` as "currently reserved and must be zero", so the wrapper passes 0 instead of
    /// exposing a knob with exactly one legal value, and `:866` -- "Make sure and check
    /// SteamNetworkingFakeIPResult_t::m_eResult" -- is the reason this returns a `Result` rather than
    /// the struct. The whole reservation comes back at once; the per-port list is
    /// [`FakeIpAllocation::ports`].
    ///
    /// Steam's four refusals are all reported through [`SteamError`] rather than as an empty answer:
    /// they are named at `steamnetworkingfakeip.h:98-102` -- Busy (the request has not completed, and
    /// `:104-105` singles it out as the one failure a poller may tolerate), InvalidParameter,
    /// LimitExceeded (too many ports asked for, or a second request after one had succeeded) and
    /// NoMatch (never asked). A struct Steam left untouched reads as its zero value, and `EResult`'s
    /// zero is not OK (`windows_bindings.rs:408` numbers `k_EResultOK` as 1), so "never answered" still
    /// comes back `Err` instead of a successful-looking allocation with a zero address.
    pub fn get_fake_ip(&self) -> Result<FakeIpAllocation, SteamError> {
        let mut raw = std::mem::MaybeUninit::<sys::SteamNetworkingFakeIPResult_t>::zeroed();
        unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_GetFakeIP(self.sockets, 0, raw.as_mut_ptr());
            fake_ip_allocation(&raw.assume_init())
        }
    }
}

/// The FakeIP block Steam assigned: one address and the ports actually granted.
///
/// `identity` is the local `ISteamNetworkingSockets` object the reservation belongs to, which
/// `steamnetworkingfakeip.h:110-114` explains is needed because a process can hold more than one (a
/// user's, and a local game server's).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeIpAllocation {
    /// Our assigned fake IPv4 address, and not exclusively ours: `:116-122` says Steam tries to
    /// avoid sharing an IP but cannot promise it, and may hand back an address another host is using.
    pub ip: Ipv4Addr,
    /// The granted ports in reservation order. Steam fills only the leading slots of its eight-entry
    /// array (`:124-130`), so the trailing zeros are dropped instead of reported as ports; and
    /// `isteamnetworkingsockets.h:831-832` warns they are neither ordered nor contiguous, so no
    /// caller should assume `ports[i + 1]` follows `ports[i]`.
    pub ports: Vec<u16>,
    /// Which local sockets object holds this reservation.
    pub identity: NetworkingIdentity,
}

/// Steam's answer to a FakeIP allocation request, as it arrives on the callback pipe.
///
/// Posted when [`begin_async_request_fake_ip`](NetworkingSockets::begin_async_request_fake_ip)
/// completes (`isteamnetworkingsockets.h:836-837`), carrying the same status the polling read
/// reports, so the outcome here is a `Result` and not a status field to remember to check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeIpResult {
    pub outcome: Result<FakeIpAllocation, SteamError>,
}

impl_callback!(cb: SteamNetworkingFakeIPResult_t => FakeIpResult {
    FakeIpResult {
        outcome: fake_ip_allocation(&cb),
    }
});

fn fake_ip_allocation(
    raw: &sys::SteamNetworkingFakeIPResult_t,
) -> Result<FakeIpAllocation, SteamError> {
    to_steam_result(raw.m_eResult).map(|()| FakeIpAllocation {
        ip: fake_ipv4(raw.m_unIP),
        ports: allocated_ports(raw.m_unPorts),
        identity: NetworkingIdentity::from(raw.m_identity),
    })
}

/// Steam keeps a fake IPv4 in one host-order integer whose documented shape is "aa.bb.cc.dd as
/// `0xaabbccdd`" (`steamnetworkingtypes.h:205`, the comment on the accessor that returns the same
/// representation). `Ipv4Addr::from(u32)` reads its argument in exactly that order -- most significant
/// octet first -- so no byte swapping belongs here, and the test that pins this is what stops a future
/// edit from "fixing" it by hand.
fn fake_ipv4(bits: u32) -> Ipv4Addr {
    Ipv4Addr::from(bits)
}

/// The leading run of granted ports, per Steam's own rule that only the first entries are non-zero
/// (`steamnetworkingfakeip.h:126-129`). This takes a prefix rather than filtering, so a zero inside
/// the array ends the reservation instead of being quietly skipped.
fn allocated_ports(ports: [u16; 8]) -> Vec<u16> {
    ports
        .iter()
        .copied()
        .take_while(|port| *port != 0)
        .collect()
}

#[cfg(test)]
mod fake_ip_tests {
    use super::*;
    use std::path::Path;

    /// Every line citation this file's docs make against `isteamnetworkingsockets.h`, as
    /// `(first, last, a phrase that must sit inside that span)`. The spans are inclusive and
    /// 1-based, exactly as cited in the prose above.
    const CITATIONS: &[(&str, u32, u32, &str)] = &[
        (
            "already in progress",
            835,
            836,
            "Returns false if a request was already in progress",
        ),
        ("the limit is 4", 829, 830, "the limit is 4"),
        (
            "not ordered or contiguous",
            831,
            832,
            "The port assignments are *not* guaranteed to have any particular order",
        ),
        (
            "result posted on completion",
            836,
            837,
            "A SteamNetworkingFakeIPResult_t will be posted when the request",
        ),
        (
            "gameservers before login",
            839,
            840,
            "you *must* call this after initializing the SDK but before",
        ),
        (
            "failure needs several retries",
            845,
            849,
            "will not be reported until *several* retries have been attempted",
        ),
        (
            "the TCP-style flow starts with this socket",
            852,
            857,
            "Server creates a listen socket using CreateListenSocketP2PFakeIP",
        ),
        (
            "index, not port",
            873,
            875,
            "idxFakePort refers to the *index* of the fake port requested",
        ),
        (
            "no need to wait",
            875,
            877,
            "However, you do not need to wait for the",
        ),
        (
            "idxFirstPort has one legal value",
            865,
            865,
            "idxFirstPort is currently reserved and must be zero",
        ),
        (
            "the result field must be checked",
            866,
            866,
            "Make sure and check SteamNetworkingFakeIPResult_t::m_eResult",
        ),
    ];

    fn sockets_header() -> Option<Vec<String>> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("steamworks-sys/lib/steam/public/steam/isteamnetworkingsockets.h");
        match std::fs::read_to_string(&path) {
            Ok(text) => Some(text.lines().map(|l| l.trim_start().to_string()).collect()),
            Err(_) => {
                // A `.crate` package ships no SDK sources, so the read skips out loud; every CI
                // checkout is a repository, where the assertions below really run.
                eprintln!(
                    "skipping FakeIP citation checks: {} is not present",
                    path.display()
                );
                None
            }
        }
    }

    #[test]
    fn every_header_citation_in_this_file_lands_on_the_sentence_it_quotes() {
        let Some(lines) = sockets_header() else {
            return;
        };
        for (what, first, last, phrase) in CITATIONS {
            assert!(
                *first >= 1 && *last as usize <= lines.len(),
                "{what}: cited {}-{} but the header has only {} lines",
                first,
                *last,
                lines.len()
            );
            let span = lines[(*first as usize - 1)..*last as usize].join(" ");
            assert!(
                span.contains(phrase),
                "{what}: expected {phrase:?} within lines {first}-{last}, got {span:?}"
            );
        }
    }

    /// Guards the guard: the ruler has to be able to fail, or a drifted header would read as a pass.
    #[test]
    fn a_citation_point_at_the_wrong_line_is_red() {
        let Some(lines) = sockets_header() else {
            return;
        };
        let (_, _, _, phrase) = CITATIONS[0];
        let shifted = lines[2..4].join(" ");
        assert!(
            !shifted.contains(phrase),
            "the fixture span already contains {phrase:?}, so the check above would not have \
             noticed a citation that drifted by a few lines"
        );
    }

    #[test]
    fn a_fake_ipv4_integer_reads_as_the_octets_steam_documents() {
        // `steamnetworkingtypes.h:205`: aa.bb.cc.dd is 0xaabbccdd, so 1.2.3.4 must not come back as
        // 4.3.2.1 -- byte-swapping this is exactly the mistake the line below would catch.
        assert_eq!(fake_ipv4(0x0102_0304), Ipv4Addr::new(1, 2, 3, 4));
        assert_eq!(fake_ipv4(0x7f00_0001), Ipv4Addr::new(127, 0, 0, 1));
        assert_eq!(fake_ipv4(0), Ipv4Addr::UNSPECIFIED);
    }

    #[test]
    fn only_the_leading_ports_of_the_reservation_are_reported() {
        assert_eq!(allocated_ports([1, 2, 0, 0, 0, 0, 0, 0]), vec![1, 2]);
        assert_eq!(allocated_ports([0; 8]), Vec::<u16>::new());
        assert_eq!(allocated_ports([9, 0, 0, 0, 0, 0, 0, 0]), vec![9]);
        // A zero in the middle ends the reservation; it is not silently skipped over.
        assert_eq!(allocated_ports([5, 0, 6, 0, 0, 0, 0, 0]), vec![5]);
    }

    #[test]
    fn the_two_fake_udp_size_limits_also_arrive_from_the_bindings() {
        // `fake_udp.rs` already pins these two numbers to the SDK *source*. This pins them to what the
        // generated layer compiled, so a bindgen input that stops carrying the header is a compile
        // error here rather than a test that quietly has nothing left to compare.
        assert_eq!(
            sys::k_cbSteamNetworkingSocketsFakeUDPPortRecommendedMTU as usize,
            crate::networking_sockets::FAKE_UDP_RECOMMENDED_MTU
        );
        assert_eq!(
            sys::k_cbSteamNetworkingSocketsFakeUDPPortMaxMessageSize as usize,
            crate::networking_sockets::FAKE_UDP_MAX_MESSAGE_BYTES
        );
    }

    /// `steamnetworkingfakeip.h:98-102` names the four refusals and `:104-105` singles Busy out as the
    /// one a poller is allowed to tolerate. That distinction survives only while each code lands on its
    /// own variant: folding them into `Generic` would make "not yet" read exactly like "fatal", which is
    /// the one thing the header tells the caller not to do.
    #[test]
    fn the_four_fake_ip_refusals_keep_their_names() {
        assert_eq!(
            to_steam_result(sys::EResult::k_EResultOK),
            Ok(()),
            "OK is not an error and must not reach the refusal path at all"
        );
        assert_eq!(
            to_steam_result(sys::EResult::k_EResultBusy),
            Err(SteamError::Busy),
            "a poller has to be able to tell 'the request has not completed' from a fatal answer"
        );
        assert_eq!(
            to_steam_result(sys::EResult::k_EResultInvalidParam),
            Err(SteamError::InvalidParameter)
        );
        assert_eq!(
            to_steam_result(sys::EResult::k_EResultLimitExceeded),
            Err(SteamError::LimitExceeded)
        );
        assert_eq!(
            to_steam_result(sys::EResult::k_EResultNoMatch),
            Err(SteamError::NoMatch)
        );
    }

    /// The other half of the same contract. Measured against the generated layer, only two of the 130
    /// `k_EResult*` codes have no `SteamError` variant: `k_EResultOK` (1), which is not an error, and
    /// `k_EResultNone` (0) -- which is exactly what a status field holds when Steam never wrote the
    /// struct. `k_EResultNone` therefore has to fall to the catch-all error and never to the `Ok` arm:
    /// reading 0 as "fine, here is your allocation" is the failure the `unwrap()` in `to_steam_result`
    /// used to turn into an abort on `Unknown(0)`.
    #[test]
    fn an_unmapped_result_code_is_an_error_and_not_a_success() {
        assert_eq!(
            sys::EResult::k_EResultNone as i32,
            0,
            "the zero value of an untouched struct must be the code the fallback sees"
        );
        let none = to_steam_result(sys::EResult::k_EResultNone);
        assert!(
            matches!(none, Err(SteamError::Generic)),
            "k_EResultNone decoded as {none:?}; an unnamed code must be the catch-all error"
        );
        // And the catch-all really is only for nameless codes: this one has a variant, and keeping it
        // distinguishable from `Generic` is what lets a caller act on the difference. Written after two
        // regex rulers -- one that required a trailing comma, one that read past the end of the macro
        // call -- both reported it as unnamed; the compiler's answer is the one recorded here.
        let later = to_steam_result(sys::EResult::k_EResultTryLater);
        assert!(
            matches!(later, Err(SteamError::TryLater)),
            "k_EResultTryLater decoded as {later:?}"
        );
    }
}
