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

unsafe impl Send for NetworkingSockets {}

unsafe impl Sync for NetworkingSockets {}

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
            results
                .into_iter()
                .map(|x| {
                    if x >= 0 {
                        Ok(MessageNumber(x as u64))
                    } else {
                        Err((-x).try_into().expect("invalid error code"))
                    }
                })
                .collect()
        }
    }
}
