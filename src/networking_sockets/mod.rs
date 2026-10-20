use crate::{networking_sockets_callback, networking_types::NetConnectionRealTimeLaneStatus};
use crate::{
    networking_types::{
        ListenSocketEvent, MessageNumber, NetConnectionEnd, NetConnectionInfo,
        NetConnectionRealTimeInfo, NetworkingAvailability, NetworkingAvailabilityError,
        NetworkingConfigEntry, NetworkingIdentity, NetworkingMessage, SendFlags, SteamIpAddr,
    },
    SteamError,
};
use crate::{CallbackHandle, Inner, SteamResult};
#[cfg(test)]
use serial_test::serial;
use std::convert::TryInto;
use std::ffi::CString;
use std::net::SocketAddr;
use std::sync::mpsc::Receiver;
use std::sync::Arc;
use sys::SteamNetworkingMessage_t;

use crate::networking_types::{AppNetConnectionEnd, NetConnectionEvent};
use steamworks_sys as sys;

mod connection;
mod fake_ip;
mod fake_udp;

pub use connection::*;
pub use fake_ip::*;
pub use fake_udp::*;

mod auth;
mod connect;
mod listen;
mod listen_socket;
mod poll_group;
mod status;

pub use listen_socket::*;
pub use poll_group::*;

/// Access to the steam networking sockets interface
pub struct NetworkingSockets {
    pub(crate) sockets: *mut sys::ISteamNetworkingSockets,
    pub(crate) inner: Arc<Inner>,
}

// SAFETY: nothing in this crate's tree needs `Send` for this type today (measured: this
// line deleted, `cargo check --all-targets` stays green). The promise is kept because
// the type is public and a caller may move it to the thread that pumps callbacks.
// The type holds a raw Steam interface pointer, plain-copy handles, and the owning `Arc<Inner>`;
// it carries no non-atomic reference count, which is the one hazard a `Send` claim here cannot
// see -- `scripts/ci/callback_threads.py` counts `Rc` tokens beside every promise and ships at
// zero for exactly that reason. Moving the value therefore moves sole ownership of the pointer;
// nothing reads the same state through two threads because of the move itself.
unsafe impl Send for NetworkingSockets {}

unsafe impl Sync for NetworkingSockets {}

/// How many messages to believe Steam wrote, out of the slots the caller offered.
///
/// Four batched receives answer with a plain `int` -- `ReceiveMessagesOnConnection`,
/// `ReceiveMessagesOnPollGroup`, the Fake UDP port's `ReceiveMessages`, and
/// `ISteamNetworkingMessages::ReceiveMessagesOnChannel` -- and the crate must not turn a hostile or
/// merely confused number into a slice length: cast through, the invalid-handle answer is ~1.8e19 and
/// `set_len` would then describe a longer `Vec` than any allocation, which is UB the moment the
/// messages are drained; a count above the slots offered claims memory nobody handed over. Both are
/// cut here, in one place, because the callers used to keep their own copies of the same rule -- two
/// of them clamped, two of them did not.
///
/// The negative case is documented rather than invented: "Returns the number of messages returned
/// into your array, up to nMaxMessages." and "If the connection handle is invalid, -1 is returned."
/// (`isteamnetworkingsockets.h:320-321`), and `ReceiveMessagesOnPollGroup` claims "the same
/// guarantees are present as mentioned in ReceiveMessagesOnConnection."
/// (`isteamnetworkingsockets.h:552-553`). Nor is it defensive-only: `create_poll_group` returns
/// whatever Steam hands back without rejecting `k_HSteamNetPollGroup_Invalid`, so a failed create
/// reaches those calls with an invalid handle -- exactly the case the header says answers -1.
pub(crate) fn messages_received(raw: std::os::raw::c_int, offered: usize) -> usize {
    usize::try_from(raw).unwrap_or(0).min(offered)
}

/// One slot of `SendMessages`' out-array: a message number, or the failure Steam put there.
///
/// The array is `int64`, and the header says which half of that range means what: "pOutMessageNumberOrResult
/// is an optional array that will receive, for each message, the message number that was assigned to the
/// message if sending was successful.  If sending failed, then a negative EResult value is placed into the
/// array.  For example, the array will hold -k_EResultInvalidState if the connection was in an invalid
/// state." (`isteamnetworkingsockets.h:295-299`).
///
/// So the negative branch is a value Steam chose, and the code is not guaranteed to be one this crate's
/// table knows: `SteamError::try_from` answers `InvalidSteamError::Unknown(raw)` for exactly that case.
/// An unmapped code becomes `SteamError::Generic` here rather than a panic, which is the convention this
/// crate already set for the same shape of answer (`src/lib.rs`, the FakeIP result status). `Generic` is
/// the coarsest true thing that can be said about such a code, and the exact number is still reachable by
/// calling `SteamError::try_from` on the slot directly.
///
/// The negation is checked because `-i64::MIN` is not representable: the old `-x` would have panicked in
/// a debug build and wrapped to the same negative number in a release one, which then fails the
/// conversion anyway -- so the honest reading of that slot is an unnamed failure, not a number.
pub(crate) fn send_outcome(code: i64) -> SteamResult<MessageNumber> {
    if code >= 0 {
        return Ok(MessageNumber(code as u64));
    }
    match code
        .checked_neg()
        .and_then(|raw| SteamError::try_from(raw).ok())
    {
        Some(error) => Err(error),
        None => Err(SteamError::Generic),
    }
}

#[cfg(test)]
mod send_slot_tests {
    //! The pure half of `send_messages`: what one `int64` slot of the out-array means. The FFI call
    //! needs a running client, the reading of its answer does not, so the decision lives in
    //! `send_outcome` and is judged here instead of being registered as uncovered.
    use super::send_outcome;
    use crate::networking_types::MessageNumber;
    use crate::SteamError;
    use steamworks_sys as sys;

    #[test]
    fn a_non_negative_slot_is_a_message_number() {
        assert_eq!(send_outcome(0), Ok(MessageNumber(0)));
        assert_eq!(send_outcome(4242), Ok(MessageNumber(4242)));
        assert_eq!(send_outcome(i64::MAX), Ok(MessageNumber(i64::MAX as u64)));
    }

    /// "If sending failed, then a negative EResult value is placed into the array"
    /// (`isteamnetworkingsockets.h:297-298`), so `-k_EResultInvalidState` has to arrive as the error
    /// it names -- a variant of its own, which is what tells this apart from the fallback below.
    #[test]
    fn a_known_negative_slot_decodes_to_the_error_it_names() {
        let code = -(sys::EResult::k_EResultInvalidState as i64);
        assert!(matches!(send_outcome(code), Err(SteamError::InvalidState)),);
    }

    /// A code outside this crate's table used to abort the caller's frame on `.expect(..)`.
    #[test]
    fn an_unknown_negative_slot_is_an_unnamed_failure_not_a_panic() {
        assert!(matches!(send_outcome(-4242), Err(SteamError::Generic)));
        // `k_EResultFail` is *mapped*, to `Generic` (`src/error.rs:25`); this asserts the two routes
        // do not get confused with each other by the fallback being the same variant.
        assert!(matches!(
            send_outcome(-(sys::EResult::k_EResultFail as i64)),
            Err(SteamError::Generic)
        ));
    }

    /// `-i64::MIN` overflows, and the slot is an `i64` Steam wrote. Debug builds panicked on the old
    /// bare `-x`; release wrapped it back to a negative number that then failed conversion.
    #[test]
    fn the_extreme_negative_slot_is_a_failure_too() {
        assert!(matches!(send_outcome(i64::MIN), Err(SteamError::Generic)));
    }
}

#[cfg(test)]
mod receive_count_tests {
    //! Pure half of the batched receive: the count clamp. The FFI call itself needs a running
    //! client, but every decision made from its return value does not, so the decisions live in
    //! `messages_received` and are judged here rather than registered as uncovered.
    use super::messages_received;

    #[test]
    fn a_valid_count_becomes_that_length() {
        assert_eq!(messages_received(3, 8), 3);
        assert_eq!(messages_received(0, 8), 0);
        assert_eq!(messages_received(8, 8), 8);
    }

    /// "If the connection handle is invalid, -1 is returned." (`isteamnetworkingsockets.h:321`).
    /// Cast through, that used to become ~1.8e19 and `set_len` would describe a `Vec` longer than
    /// any allocation.
    #[test]
    fn the_invalid_handle_minus_one_reads_as_no_messages() {
        assert_eq!(messages_received(-1, 8), 0);
        assert_eq!(messages_received(i32::MIN, 8), 0);
    }

    /// A count above the slots offered is truncated to them: Steam may not write more pointers than
    /// the array holds, and if the number says otherwise the array is the authority.
    #[test]
    fn an_over_large_count_is_clamped_to_the_offered_slots() {
        assert_eq!(messages_received(99, 8), 8);
        assert_eq!(messages_received(i32::MAX, 0), 0);
    }
}

#[derive(Debug, Error)]
#[error("operation was unsuccessful an invalid handle was returned")]
pub struct InvalidHandle;

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use crate::{networking_types::NetworkingConnectionState, Client};

    use super::*;
    use crate::networking_types::{
        ListenSocketEvent, NetworkingConfigEntry, NetworkingConfigValue, SendFlags,
    };

    #[test]
    #[serial]
    #[ignore = "requires a running Steam client"]
    fn test_create_listen_socket_ip() {
        let client = Client::init().unwrap();
        let sockets = client.networking_sockets();
        let socket_result = sockets.create_listen_socket_ip(
            SocketAddr::new(Ipv4Addr::new(0, 0, 0, 0).into(), 12345),
            vec![],
        );
        assert!(socket_result.is_ok());
    }

    #[test]
    #[serial]
    #[ignore = "requires a running Steam client"]
    fn test_socket_connection() {
        let client = Client::init().unwrap();
        let sockets = client.networking_sockets();

        sockets.init_authentication().unwrap();

        let debug_config = vec![NetworkingConfigEntry::new_int32(
            NetworkingConfigValue::IPAllowWithoutAuth,
            1,
        )];

        println!("Create ListenSocket");
        let bound_ip = SocketAddr::new(Ipv4Addr::new(127, 0, 0, 1).into(), 12345);
        let socket = sockets
            .create_listen_socket_ip(bound_ip, debug_config.clone())
            .unwrap();

        println!("Create connection");
        let mut to_server = sockets
            .connect_by_ip_address(bound_ip, debug_config)
            .unwrap();

        println!("Run callbacks");
        for _ in 0..5 {
            client.run_callbacks();
            std::thread::sleep(::std::time::Duration::from_millis(50));
        }

        let event = socket.try_receive_event().unwrap();
        match event {
            ListenSocketEvent::Connecting(request) => {
                println!("Accept connection");
                request.accept().unwrap();
            }
            _ => panic!("unexpected event"),
        }

        println!("Run callbacks");
        for _ in 0..5 {
            client.run_callbacks();
            std::thread::sleep(::std::time::Duration::from_millis(50));
        }

        let event = socket.try_receive_event().unwrap();
        let mut to_client = match event {
            ListenSocketEvent::Connected(connected) => connected.take_connection(),
            _ => panic!("unexpected event"),
        };

        println!("Configure connection lanes");
        let mut lane_priorities = vec![0; 2];
        let mut lane_weights = vec![0; 2];
        lane_priorities[0] = 1;
        lane_weights[0] = 1;
        lane_priorities[1] = 1;
        lane_weights[1] = 3;

        let result =
            sockets.configure_connection_lanes(&to_server, 2, &lane_priorities, &lane_weights);
        assert!(result.is_ok());

        println!("Get connection info remote client");
        let info = sockets.get_connection_info(&to_client).unwrap();
        match info.state() {
            Ok(state) => assert_eq!(state, NetworkingConnectionState::Connected),
            _ => panic!("unexpected state"),
        }

        println!("Get connection info server");
        let info = sockets.get_connection_info(&to_server).unwrap();
        match info.state() {
            Ok(state) => assert_eq!(state, NetworkingConnectionState::Connected),
            _ => panic!("unexpected state"),
        }

        println!("Get quick connection info remote client");
        let (info, lanes) = sockets
            .get_realtime_connection_status(&to_client, 0)
            .unwrap();
        if let Ok(net_connection) = info.connection_state() {
            assert_eq!(net_connection, NetworkingConnectionState::Connected);
            assert_eq!(lanes.len(), 0);
        } else {
            panic!("unexpected state");
        }

        println!("Get quick connection info server");
        let (info, lanes) = sockets
            .get_realtime_connection_status(&to_server, 2)
            .unwrap();
        if let Ok(net_connection) = info.connection_state() {
            assert_eq!(net_connection, NetworkingConnectionState::Connected);
            assert_eq!(lanes.len(), 2);
        } else {
            panic!("unexpected state");
        }

        println!("Send message to server");
        to_server
            .send_message(&[1, 1, 2, 5], SendFlags::RELIABLE_NO_NAGLE)
            .unwrap();

        std::thread::sleep(::std::time::Duration::from_millis(100));

        println!("Receive message");
        let messages = to_client.receive_messages(10).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].data(), &[1, 1, 2, 5]);

        println!("Send message to client");
        to_client
            .send_message(&[3, 3, 3, 1], SendFlags::RELIABLE_NO_NAGLE)
            .unwrap();

        std::thread::sleep(::std::time::Duration::from_millis(100));

        println!("Receive message");
        let messages = to_server.receive_messages(10).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].data(), &[3, 3, 3, 1]);

        println!("Send message to client with send_messages");
        let utils = client.networking_utils();
        let mut message = utils.allocate_message(0);
        message.set_connection(&to_client);
        message.set_send_flags(SendFlags::RELIABLE_NO_NAGLE);
        message.set_data(vec![1, 2, 34, 5]).unwrap();
        socket.send_messages(vec![message]);

        std::thread::sleep(::std::time::Duration::from_millis(1000));

        println!("Receive message");
        let messages = to_server.receive_messages(10).unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].data(), &[1, 2, 34, 5]);
    }
}

impl NetworkingSockets {
    /// Send one or more messages on any connection, with per-message lane/channel support.
    ///
    /// Each `NetworkingMessage` must have its connection set via `set_connection()`.
    /// Use `set_channel()` to specify the lane for each message.
    ///
    /// This is the same as `ListenSocket::send_messages()` but callable without
    /// a listen socket — works for both listener and dialer sides.
    ///
    /// pOutMessageNumberOrResult is an optional array that will receive,
    /// for each message, the message number that was assigned to the message
    /// if sending was successful.  If sending failed, then a negative EResult
    /// value is placed into the array.  For example, the array will hold
    /// -k_EResultInvalidState if the connection was in an invalid state.
    /// See ISteamNetworkingSockets::SendMessageToConnection for possible
    /// failure codes.
    pub fn send_messages(
        &self,
        messages: impl IntoIterator<Item = NetworkingMessage>,
    ) -> Vec<SteamResult<MessageNumber>> {
        let messages: Vec<_> = messages.into_iter().map(|x| x.take_message()).collect();
        let mut results = vec![0; messages.len()];
        unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_SendMessages(
                self.sockets,
                messages.len() as _,
                messages.as_ptr(),
                results.as_mut_ptr(),
            );
            results.into_iter().map(send_outcome).collect()
        }
    }
}
