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
/// (`steamnetworkingfakeip.h:11-15`).
///
/// Quoted in full because it is *not* a limit the code enforces: "It is HIGHLY recommended to limit
/// messages sent via Fake UDP port to this value ... Although the real MTU might be higher than
/// this, this particular conservative value is chosen so that fragmentation won't be occurring and
/// hiding performance problems from you." Exposing the number is the whole point; refusing above it
/// would invent a rule Steam does not have.
pub const FAKE_UDP_RECOMMENDED_MTU: usize = 1200;

/// The one size limit that *is* a hard rule: "Messages larger than this size are not allowed and
/// cannot be sent via Fake UDP port" (`steamnetworkingfakeip.h:18-19`).
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

/// A datagram port addressed with FakeIP, "acts like a UDP port, sending and receiving datagrams
/// addressed using FakeIP addresses" (`steamnetworkingfakeip.h:24-25`).
///
/// This type deliberately has **no `Drop`**. Steam's own docs for the factory say the object is
/// shared and uncounted: "If you call this function multiple times and pass the same (nonnegative)
/// fake port index, the same object will be returned, and this object is not reference counted"
/// (`isteamnetworkingsockets.h:906-908`). An automatic destroy would then kill the other handle's
/// object too, silently -- so disposal is explicit ([`FakeUDPPort::destroy`]) and the hazard is
/// stated where it can be read.
pub struct FakeUDPPort {
    port: *mut sys::ISteamNetworkingFakeUDPPort,
    inner: Arc<Inner>,
    /// Reused so a polling loop does not allocate per call, exactly as `NetPollGroup` does
    /// (`src/networking_sockets/mod.rs:577-582`).
    message_buffer: Vec<*mut sys::SteamNetworkingMessage_t>,
}

// Steam states one rule and then relaxes it: "this function call is not threadsafe with respect to
// any other method of this interface. (However, in general all other operations are threadsafe with
// respect to each other.)" (`steamnetworkingfakeip.h:33-35`). `destroy` therefore takes the value by
// ownership and `receive_messages` takes `&mut self`, which is how Rust expresses "nothing else may
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
    pub fn create_fake_udp_port(&self, idx_fake_server_port: i32) -> Option<FakeUDPPort> {
        let port = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_CreateFakeUDPPort(
                self.sockets,
                idx_fake_server_port,
            )
        };
        if port.is_null() {
            return None;
        }
        Some(FakeUDPPort {
            port,
            inner: self.inner.clone(),
            message_buffer: Vec::new(),
        })
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
    /// cannot be sent". Staying under [`FAKE_UDP_RECOMMENDED_MTU`] is recommended but never enforced.
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
    /// `SteamNetworkingMessage_t::m_identity in the returned message(s) will always contain a
    /// FakeIP. See ISteamNetworkingUtils::GetRealIdentityForFakeIP` (`steamnetworkingfakeip.h:61-66`).
    ///
    /// Steam's return value is clamped by `received_count` before it is used as a length, so a
    /// negative or over-large answer cannot make this read slots that were never offered.
    pub fn receive_messages(&mut self, batch_size: usize) -> Vec<NetworkingMessage> {
        if batch_size == 0 {
            return Vec::new();
        }
        if self.message_buffer.capacity() < batch_size {
            self.message_buffer.reserve(batch_size);
        }
        let messages: Vec<*mut sys::SteamNetworkingMessage_t> = unsafe {
            let mut out = vec![std::ptr::null_mut(); batch_size];
            let raw = sys::SteamAPI_ISteamNetworkingFakeUDPPort_ReceiveMessages(
                self.port,
                out.as_mut_ptr(),
                batch_size as ::std::os::raw::c_int,
            );
            let count = received_count(raw, batch_size);
            // The slots Steam did not fill still hold null; only the filled prefix may become
            // owned messages.
            out.truncate(count);
            out
        };
        messages
            .into_iter()
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
    /// Consuming the wrapper is what keeps Rust from handing out a second destroy, and it is the
    /// reason there is no `Drop`: for a server port (index `>= 0`) Steam returns *the same object*
    /// for every call with that index and does not reference count it
    /// (`isteamnetworkingsockets.h:906-908`), so two live wrappers for one index are already a
    /// caller-side alias -- dropping one behind the other's back would be the bug, not the fix.
    /// After this call any other wrapper for the same index refers to a destroyed object; treat it
    /// as a single-owner handle.
    pub fn destroy(self) {
        unsafe { sys::SteamAPI_ISteamNetworkingFakeUDPPort_DestroyFakeUDPPort(self.port) }
    }
}

#[cfg(test)]
mod fake_udp_tests {
    use super::*;

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
        assert!(FAKE_UDP_RECOMMENDED_MTU < FAKE_UDP_MAX_MESSAGE_BYTES);
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
}
