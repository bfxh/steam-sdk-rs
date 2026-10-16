//! The two FakeIP calls that are safe to reach today: asking Steam to reserve a block of fake
//! ports, and opening a listen socket on one of them by *index*.
//!
//! Both are deliberately fire-and-collect-later. `isteamnetworkingsockets.h:875-877` says the
//! listen socket does not have to wait for the allocation to finish, and `:836-837` says a
//! `SteamNetworkingFakeIPResult_t` is posted when the request completes -- which is exactly the
//! message this crate cannot yet read. The remaining two FakeIP functions are therefore *not*
//! wrapped here, for one reason that lives in the generated layer rather than in this file:
//! `SteamNetworkingFakeIPResult_t` is declared as an opaque shell (`steamworks-sys` binds it as
//! `_unused: [u8; 0]` at `windows_bindings.rs:17499-17501`), so `GetFakeIP` could not be handed a
//! struct with the header's real fields (`steamnetworkingfakeip.h:93`), and
//! `ISteamNetworkingUtils::SetGlobalCallback_FakeIPResult` would have to accept a callback over a
//! layout we cannot name. Regenerating the bindings with that header in the input list is the
//! prerequisite for both, and it is a decision about the generated layer rather than about this
//! module.

use super::*;

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
}

#[cfg(test)]
mod fake_ip_tests {
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
}
