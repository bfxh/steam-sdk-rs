//! The ad-hoc "UDP port" face of the FakeIP system: `ISteamNetworkingFakeUDPPort`.
//!
//! Obtained from [`NetworkingSockets::create_fake_udp_port`], which is the only way to get one --
//! "To communicate using an ad-hoc sendto/recv from (UDP-style) API, use CreateFakeUDPPort"
//! (`isteamnetworkingsockets.h:860-861`). The interface itself is declared in
//! `steamnetworkingfakeip.h:22-78`, and unlike most of this crate's surfaces it has to be disposed
//! of explicitly: see [`FakeUDPPort::destroy`] for the reason, which is a line in Steam's own docs
//! about the object *not* being reference counted.

use super::*;
use crate::networking_types::{NetworkingMessage, SendFlags, SteamIpAddr};

/// Steam's conservative recommended ceiling for one datagram
/// (`steamnetworkingfakeip.h:10-15`).
///
/// Quoted in full because it is *not* a limit the code enforces: "It is HIGHLY recommended to limit
/// messages sent via Fake UDP port to this value ... Although the real MTU might be higher than
/// this, this particular conservative value is chosen so that fragmentation won't be occurring and
/// hiding performance problems from you." Exposing the number is the whole point; refusing above it
/// would invent a rule Steam does not have.
pub const FAKE_UDP_RECOMMENDED_MTU: usize = 1200;

/// The one size limit that *is* a hard rule: "Messages larger than this size are not allowed and
/// cannot be sent via Fake UDP port" (`steamnetworkingfakeip.h:17-18`; the sentence runs from the
/// "Messages larger than" line, not from the constant's line).
pub const FAKE_UDP_MAX_MESSAGE_BYTES: usize = 4096;

/// The length Steam is told for one datagram, after the hard size rule is applied.
///
/// Kept out of the FFI block so the refusal is something a test can reach without a client: Steam
/// would answer an oversized send with its own `EResult`, but the wrapper knows the documented
/// ceiling and says so rather than spending a call on it.
fn fake_message_len(len: usize) -> Result<u32, SteamError> {
    if len > FAKE_UDP_MAX_MESSAGE_BYTES {
        return Err(SteamError::InvalidParameter);
    }
    u32::try_from(len).map_err(|_| SteamError::InvalidParameter)
}

/// How many received messages to believe, out of the batch Steam was asked to fill.
///
/// `ReceiveMessages` returns a plain `int` and the crate must not turn a hostile or merely
/// confused number into a slice length: a negative answer would wrap to an enormous `usize` at
/// `set_len`, and an answer above `asked` would claim slots the caller never offered. Both are cut
/// here, which is the same discipline as `written_bytes` in `src/user/voice.rs` -- Steam's count is
/// trusted only as far as the memory on this side allows.
fn received_count(raw: ::std::os::raw::c_int, asked: usize) -> usize {
    if raw < 0 {
        0
    } else {
        (raw as usize).min(asked)
    }
}

/// Claims the right to hold the one Rust wrapper around a FakeIP datagram port.
///
/// The rule being enforced is Steam's, quoted because it is the whole reason the type needs a claim:
/// "If you call this function multiple times and pass the same (nonnegative) fake port index, the same
/// object will be returned, and this object is not reference counted"
/// (`isteamnetworkingsockets.h:906-908`). Two live wrappers of one uncounted object means `destroy()`
/// through one leaves the other pointing at freed memory, in safe code, on a type that is `Send + Sync`
/// -- so the second call to wrap a server index must fail rather than alias. Ephemeral ports are the
/// opposite case and must not be deduplicated at all: "a distinct object will be returned for each
/// call" (`:910-911`), so any negative index is allowed every time and is never recorded.
fn claim_port(claimed: &mut std::collections::HashSet<i32>, idx: i32) -> bool {
    if idx < 0 {
        return true;
    }
    claimed.insert(idx)
}

/// Gives up a claim so the index can be wrapped again. An ephemeral port has nothing to release.
fn release_port(claimed: &mut std::collections::HashSet<i32>, claim: Option<i32>) {
    if let Some(idx) = claim {
        claimed.remove(&idx);
    }
}

/// A datagram port addressed with FakeIP, "acts like a UDP port, sending and receiving datagrams
/// addressed using FakeIP addresses" (`steamnetworkingfakeip.h:24-25`).
///
/// This type has a `Drop` that gives up its claim on the index and nothing more. Steam's own docs for
/// the factory say the object is
/// shared and uncounted: "If you call this function multiple times and pass the same (nonnegative)
/// fake port index, the same object will be returned, and this object is not reference counted"
/// (`isteamnetworkingsockets.h:906-908`). An automatic destroy would then kill the other handle's
/// object too, silently -- so disposal stays explicit ([`FakeUDPPort::destroy`]). What `Drop` does
/// instead is give up the *claim* (see below): releasing the claim without destroying the object is
/// always safe, and holding onto it would leave an index un-wrappable forever after a wrapper was
/// dropped without calling `destroy`.
pub struct FakeUDPPort {
    port: *mut sys::ISteamNetworkingFakeUDPPort,
    inner: Arc<Inner>,
    /// Reused so a polling loop does not allocate per call, exactly as `NetPollGroup` does
    /// (`src/networking_sockets/mod.rs:577-582`).
    message_buffer: Vec<*mut sys::SteamNetworkingMessage_t>,
    /// `Some(index)` while this value holds the claim on a server port; `None` for an ephemeral one.
    claim: Option<i32>,
}

// Steam states one rule and then relaxes it: "this function call is not threadsafe with respect to
// any other method of this interface. (However, in general all other operations are threadsafe with
// respect to each other.)" (`steamnetworkingfakeip.h:33-35`). `destroy` therefore takes the value by
// ownership and `receive_messages` takes `&mut self`, which is how Rust expresses that nothing
// else may
// be in flight"; everything else is shared `&self`.
unsafe impl Send for FakeUDPPort {}
unsafe impl Sync for FakeUDPPort {}

impl NetworkingSockets {
    /// Gets the datagram port for one FakeIP port index, or `None` if Steam hands back null.
    ///
    /// `idx_fake_server_port >= 0` is a *server* port: it refers to the index reserved by
    /// `BeginAsyncRequestFakeIP`, "You may call this before the allocation has completed. However,
    /// any attempts to send packets will fail until the allocation has succeeded"
    /// (`isteamnetworkingsockets.h:901-904`). Passing `-1` creates a client/ephemeral port, "a
    /// distinct object will be returned for each call" (`h:910-911`).
    ///
    /// Whether null happens is not documented anywhere in the interface's own comment, so this
    /// reports it as `None` and infers nothing beyond "no port". The FakeIP *allocation* calls
    /// (`BeginAsyncRequestFakeIP`, `GetFakeIP`) are not wrapped by this crate yet, so a caller
    /// obtaining a global FakeIP still reaches for `sys::` for that step; see
    /// `docs/SuperSteam-Design.md` §6.1c, which now records that boundary instead of pretending the
    /// port appeared out of nowhere.
    ///
    /// A second call for the same nonnegative index returns `None` while a wrapper is alive: Steam
    /// would hand back the same uncounted object, and two owners of it is the use-after-free this
    /// type exists to prevent. Ask for the claim first, so two threads racing on one index cannot both
    /// get past it, and hand it back if Steam produced nothing.
    pub fn create_fake_udp_port(&self, idx_fake_server_port: i32) -> Option<FakeUDPPort> {
        let claimed = self
            .inner
            .networking_sockets_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut claimed = claimed;
        if !claim_port(&mut claimed.fake_udp_ports, idx_fake_server_port) {
            return None;
        }
        drop(claimed);

        let port = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_CreateFakeUDPPort(
                self.sockets,
                idx_fake_server_port,
            )
        };
        if port.is_null() {
            self.release_claim(idx_fake_server_port);
            return None;
        }
        Some(FakeUDPPort {
            port,
            inner: self.inner.clone(),
            message_buffer: Vec::new(),
            claim: (idx_fake_server_port >= 0).then_some(idx_fake_server_port),
        })
    }

    /// Gives up this socket's claim on one port index, tolerating a poisoned lock because the only
    /// state touched is a set of integers.
    fn release_claim(&self, idx: i32) {
        let mut data = self
            .inner
            .networking_sockets_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        release_port(&mut data.fake_udp_ports, (idx >= 0).then_some(idx));
    }
}

impl FakeUDPPort {
    /// Sends one datagram to a FakeIP address.
    ///
    /// `flags` is the same `SendFlags` vocabulary as connection sends. Steam's notes for this call
    /// are worth reading before using it (`steamnetworkingfakeip.h:38-58`): reliable messages are
    /// **not** supported here, `k_EResultBusy` means "this is a server port and the global
    /// allocation has not yet completed", and `k_EResultIPNotFound` means an ephemeral peer's
    /// connection has timed out. They come back as the matching `SteamError` variants.
    ///
    /// A payload above [`FAKE_UDP_MAX_MESSAGE_BYTES`] is refused here with
    /// `InvalidParameter` rather than sent, because Steam documents that size as "not allowed and
    /// cannot be sent" (`steamnetworkingfakeip.h:17-18`). Staying under [`FAKE_UDP_RECOMMENDED_MTU`]
    /// is recommended but never enforced.
    pub fn send_to_fake_ip(
        &self,
        address: SocketAddr,
        data: &[u8],
        flags: SendFlags,
    ) -> Result<(), SteamError> {
        let len = fake_message_len(data.len())?;
        let address = SteamIpAddr::from(address);
        let result = unsafe {
            sys::SteamAPI_ISteamNetworkingFakeUDPPort_SendMessageToFakeIP(
                self.port,
                address.as_ptr(),
                data.as_ptr() as _,
                len,
                flags.bits(),
            )
        };
        crate::to_steam_result(result)
    }

    /// Drains up to `batch_size` received datagrams; the returned messages own their payload, as
    /// everywhere else in this module.
    ///
    /// `steamnetworkingfakeip.h:61-66` words it exactly: "SteamNetworkingMessage_t::m_identity in the
    /// returned message(s) will always contain a FakeIP.  See
    /// ISteamNetworkingUtils::GetRealIdentityForFakeIP."
    ///
    /// Steam's return value is clamped by `received_count` before it is used as a length, so a
    /// negative or over-large answer cannot make this read slots that were never offered.
    pub fn receive_messages(&mut self, batch_size: usize) -> Vec<NetworkingMessage> {
        if batch_size == 0 {
            return Vec::new();
        }
        if self.message_buffer.capacity() < batch_size {
            // Same reasoning as `NetPollGroup`: the buffer is drained below, so `len` is 0 and
            // `reserve` guarantees capacity for the whole batch.
            self.message_buffer.reserve(batch_size);
        }
        self.message_buffer.clear();
        unsafe {
            let raw = sys::SteamAPI_ISteamNetworkingFakeUDPPort_ReceiveMessages(
                self.port,
                self.message_buffer.as_mut_ptr(),
                batch_size as ::std::os::raw::c_int,
            );
            // Only the slots Steam actually filled may become owned messages; a clamped count
            // keeps a bad answer from turning into a length this side never allocated.
            self.message_buffer.set_len(received_count(raw, batch_size));
        }
        self.message_buffer
            .drain(..)
            .map(|message| NetworkingMessage {
                message,
                _inner: self.inner.clone(),
            })
            .collect()
    }

    /// Asks Steam to tear down its internal connection to `address` in a few seconds.
    ///
    /// Optional by Steam's own account: "Idle connections automatically time out, and so this is
    /// not strictly *necessary*, but if you have reason to believe that you are done talking to a
    /// given peer for a while, you can call this to speed up the timeout. If any remaining packets
    /// are sent or received from the peer, the cleanup is canceled"
    /// (`steamnetworkingfakeip.h:71-76`). The usual call site is right after an application-layer
    /// close packet.
    pub fn schedule_cleanup(&self, address: SocketAddr) {
        let address = SteamIpAddr::from(address);
        unsafe {
            sys::SteamAPI_ISteamNetworkingFakeUDPPort_ScheduleCleanup(self.port, address.as_ptr())
        }
    }

    /// Destroys this port and its internal connections.
    ///
    /// Consuming the wrapper is what keeps Rust from handing out a second destroy. `Drop` exists but
    /// only releases the claim on the index, never destroys the object, because
    /// `isteamnetworkingsockets.h:906-908` says that for a server port (index `>= 0`) "If you call
    /// this function multiple times and pass the same (nonnegative) fake port index, the same object
    /// will be returned, and this object is not reference counted" -- an automatic destroy would then
    /// kill the object another owner is still using.
    /// After this call any other wrapper for the same index refers to a destroyed object; treat it
    /// as a single-owner handle.
    pub fn destroy(mut self) {
        // The claim is given up before the object is destroyed, and `take()` means the `Drop` that
        // runs at the end of this method has nothing left to release. Doing it in this order closes
        // the window where a fresh wrapper could claim an index whose object had already been
        // destroyed -- and the header says "this object is not reference counted"
        // (`isteamnetworkingsockets.h:906-908`), so such a wrapper would point at freed memory.
        let claim = self.claim.take();
        if let Some(idx) = claim {
            let mut data = self
                .inner
                .networking_sockets_data
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            release_port(&mut data.fake_udp_ports, Some(idx));
            drop(data);
        }
        unsafe { sys::SteamAPI_ISteamNetworkingFakeUDPPort_DestroyFakeUDPPort(self.port) };
    }
}

impl Drop for FakeUDPPort {
    /// Releases the claim on the index, and nothing else.
    ///
    /// This deliberately does *not* destroy the port: the headers say the object is shared and
    /// uncounted, so an automatic destroy would silently kill another owner's port. Releasing the
    /// claim is safe on its own -- the worst case is that the caller wraps the same index again and
    /// gets the same object back, which is exactly what Steam documents. Without this, a wrapper
    /// dropped without calling `destroy` would leave the index claimed forever.
    fn drop(&mut self) {
        let Some(idx) = self.claim.take() else {
            return;
        };
        let mut data = self
            .inner
            .networking_sockets_data
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        release_port(&mut data.fake_udp_ports, Some(idx));
    }
}

#[cfg(test)]
mod fake_udp_tests {
    use super::*;

    /// Steam's rule is "If you call this function multiple times and pass the same (nonnegative)
    /// fake port index, the same object will be returned, and this object is not reference counted"
    /// (`isteamnetworkingsockets.h:906-908`), so a second wrapper for that index must not exist: this
    /// is the one line of the claim that keeps `destroy()` from leaving a dangling sibling.
    #[test]
    fn one_server_index_supports_one_live_wrapper() {
        let mut claimed = std::collections::HashSet::new();
        assert!(
            claim_port(&mut claimed, 2000),
            "the first wrapper claims the index"
        );
        assert!(
            !claim_port(&mut claimed, 2000),
            "a second wrapper for the same index must be refused"
        );
        assert!(
            claim_port(&mut claimed, 2001),
            "a different index is unaffected"
        );
    }

    /// Ephemeral ports are the documented opposite case -- "a distinct object will be returned for
    /// each call" (`:910-911`) -- so deduplicating them would refuse handles Steam says are unique.
    #[test]
    fn ephemeral_ports_are_never_claimed_or_deduplicated() {
        let mut claimed = std::collections::HashSet::new();
        assert!(claim_port(&mut claimed, -1));
        assert!(claim_port(&mut claimed, -1));
        assert!(claim_port(&mut claimed, i32::MIN));
        assert!(
            claimed.is_empty(),
            "nothing about an ephemeral port is recorded"
        );
    }

    /// Giving a claim back is what lets an index be wrapped again -- and only for a claimed index.
    #[test]
    fn releasing_a_claim_makes_the_index_wrappable_again() {
        let mut claimed = std::collections::HashSet::new();
        assert!(claim_port(&mut claimed, 2000));
        release_port(&mut claimed, Some(2000));
        assert!(
            claim_port(&mut claimed, 2000),
            "the index is free once released"
        );
        release_port(&mut claimed, Some(2000));
        release_port(&mut claimed, Some(4242));
        release_port(&mut claimed, None);
        assert!(
            claim_port(&mut claimed, -1),
            "and an ephemeral port stays claimable"
        );
    }

    /// The vendored header is the source of both numbers; a copy of a copy proves nothing.
    ///
    /// Read at run time instead of `include_str!` for the same reason `lib_tests.rs` reads the
    /// `-sys` sources that way (`src/lib_tests.rs:288-301`): a packaged crate does not carry this
    /// file, so the read skips out loud there. Every CI checkout is a repository, where the path
    /// exists and the assertions below really run.
    fn vendored_fake_ip_header() -> Option<String> {
        let path = format!(
            "{}/steamworks-sys/lib/steam/public/steam/steamnetworkingfakeip.h",
            env!("CARGO_MANIFEST_DIR")
        );
        match std::fs::read_to_string(&path) {
            Ok(text) => Some(text),
            Err(_) => {
                eprintln!("skipping fake-UDP limit parity: {path} is not present");
                None
            }
        }
    }

    fn header_constant(header: &str, name: &str) -> usize {
        for line in header.lines() {
            // Declared as `constexpr int <name> = <value>;`, so the name is never at the start of
            // the line -- matching only a prefix reported "not found" for a header that does define
            // it, which is how this parser failed its first run.
            let position = match line.find(name) {
                Some(at) => at + name.len(),
                None => continue,
            };
            let value = match line[position..].split('=').nth(1) {
                Some(after_equals) => after_equals.trim().trim_end_matches(';').trim(),
                None => panic!("{name} is not written as `name = value`: {line}"),
            };
            return value
                .parse::<usize>()
                .unwrap_or_else(|error| panic!("{name}: {value:?} is not a number ({error})"));
        }
        panic!("{name} not found in the vendored header");
    }

    #[test]
    fn the_two_size_limits_still_match_the_vendored_header() {
        let Some(header) = vendored_fake_ip_header() else {
            return;
        };
        assert_eq!(
            header_constant(
                &header,
                "k_cbSteamNetworkingSocketsFakeUDPPortRecommendedMTU"
            ),
            FAKE_UDP_RECOMMENDED_MTU
        );
        assert_eq!(
            header_constant(
                &header,
                "k_cbSteamNetworkingSocketsFakeUDPPortMaxMessageSize"
            ),
            FAKE_UDP_MAX_MESSAGE_BYTES
        );
        // Compared on the numbers read out of the header, not on the Rust constants: an assert
        // between two constants cannot fail for any reason except someone editing it.
        let mtu = header_constant(
            &header,
            "k_cbSteamNetworkingSocketsFakeUDPPortRecommendedMTU",
        );
        let max = header_constant(
            &header,
            "k_cbSteamNetworkingSocketsFakeUDPPortMaxMessageSize",
        );
        assert!(
            mtu < max,
            "the recommended MTU must sit under the hard maximum"
        );
    }

    #[test]
    fn an_oversized_datagram_is_refused_before_it_reaches_steam() {
        assert_eq!(fake_message_len(0), Ok(0));
        assert_eq!(fake_message_len(1), Ok(1));
        // The recommendation is not a wall: up to the documented maximum still goes out.
        assert_eq!(fake_message_len(FAKE_UDP_RECOMMENDED_MTU), Ok(1200));
        assert_eq!(fake_message_len(FAKE_UDP_MAX_MESSAGE_BYTES), Ok(4096));
        assert_eq!(
            fake_message_len(FAKE_UDP_MAX_MESSAGE_BYTES + 1),
            Err(SteamError::InvalidParameter)
        );
        assert_eq!(
            fake_message_len(usize::MAX),
            Err(SteamError::InvalidParameter)
        );
    }

    #[test]
    fn a_receive_count_is_clamped_to_the_slots_that_were_offered() {
        assert_eq!(received_count(-1, 8), 0);
        assert_eq!(received_count(i32::MIN, 8), 0);
        assert_eq!(received_count(0, 8), 0);
        assert_eq!(received_count(3, 8), 3);
        assert_eq!(received_count(8, 8), 8);
        // Claiming more slots than were offered must not widen the slice.
        assert_eq!(received_count(1000, 8), 8);
        assert_eq!(received_count(i32::MAX, 0), 0);
    }

    /// Every `steamnetworkingfakeip.h` line citation this file's docs make, checked against the
    /// vendored header itself. The two constants above already read their numbers out of this file;
    /// this is the same treatment for the *prose*, which had gone unmeasured -- and measuring it found
    /// two spans one line off (the MTU note begins at 10, not 11; the hard-rule sentence is 17-18,
    /// not 18-19, which is the constant's own line).
    ///
    /// One phrase per span, and each phrase sits inside a single header line: the ruler joins lines
    /// with a space but keeps the `///` prefixes, so a phrase that runs across a line break can never
    /// match, and a check written that way would fail for the wrong reason.
    const CITATIONS: &[(&str, u32, u32, &str)] = &[
        (
            "the interface this type wraps",
            22,
            78,
            "ISteamNetworkingFakeUDPPort",
        ),
        (
            "the MTU figure is a recommendation",
            10,
            15,
            "It is HIGHLY recommended to limit",
        ),
        (
            "the message ceiling is a hard rule",
            17,
            18,
            "Messages larger than this size are not allowed",
        ),
        (
            "what a fake UDP port acts like",
            24,
            25,
            "Acts like a UDP port, sending and receiving datagrams",
        ),
        (
            "which call is not threadsafe",
            33,
            35,
            "not threadsafe with respect",
        ),
        (
            "the send notes this module repeats",
            38,
            58,
            "reliable messages (k_nSteamNetworkingSend_Reliable) are not supported",
        ),
        (
            "identities come back as FakeIP",
            61,
            66,
            "m_identity in the returned message(s) will always contain",
        ),
        (
            "cleanup is optional, not required",
            71,
            76,
            "not strictly *necessary*,",
        ),
    ];

    fn fakeip_header_lines() -> Option<Vec<String>> {
        let text = vendored_fake_ip_header()?;
        Some(
            text.lines()
                .map(|line| line.trim_start().to_string())
                .collect(),
        )
    }

    #[test]
    fn every_fake_udp_header_citation_lands_on_the_sentence_it_quotes() {
        let Some(lines) = fakeip_header_lines() else {
            return;
        };
        for (what, first, last, phrase) in CITATIONS {
            assert!(
                *first >= 1 && *last as usize <= lines.len(),
                "{what}: cited {}-{} but the header has only {} lines",
                first,
                last,
                lines.len()
            );
            let span = lines[(*first as usize - 1)..*last as usize].join(" ");
            assert!(
                span.contains(phrase),
                "{what}: expected {phrase:?} within lines {first}-{last}, got {span:?}"
            );
        }
    }

    /// Guards the guard: the hard-rule sentence must not be findable in the recommendation's span, so
    /// a citation drifting by a few lines here is something the test above actually sees.
    #[test]
    fn a_fake_udp_citation_point_at_the_wrong_line_is_red() {
        let Some(lines) = fakeip_header_lines() else {
            return;
        };
        let (_, _, _, phrase) = CITATIONS[2];
        let shifted = lines[9..15].join(" ");
        assert!(
            !shifted.contains(phrase),
            "the fixture span already contains {phrase:?}, so the check above would not have \
             noticed a citation that drifted by a few lines"
        );
    }
}
