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

pub use connection::*;

/// Access to the steam networking sockets interface
pub struct NetworkingSockets {
    pub(crate) sockets: *mut sys::ISteamNetworkingSockets,
    pub(crate) inner: Arc<Inner>,
}

unsafe impl Send for NetworkingSockets {}
unsafe impl Sync for NetworkingSockets {}

impl NetworkingSockets {
    /// Creates a "server" socket that listens for clients to connect to by calling ConnectByIPAddress, over ordinary UDP (IPv4 or IPv6)
    ///
    /// You must select a specific local port to listen on and set it as the port field of the local address.
    ///
    /// Usually you will set the IP portion of the address to zero, (SteamNetworkingIPAddr::Clear()).
    /// This means that you will not bind to any particular local interface (i.e. the same as INADDR_ANY in plain socket code).
    /// Furthermore, if possible the socket will be bound in "dual stack" mode, which means that it can accept both IPv4 and IPv6 client connections.
    /// If you really do wish to bind a particular interface, then set the local address to the appropriate IPv4 or IPv6 IP.
    ///
    /// If you need to set any initial config options, pass them here.
    /// See SteamNetworkingConfigValue_t for more about why this is preferable to setting the options "immediately" after creation.
    ///
    /// When a client attempts to connect, a SteamNetConnectionStatusChangedCallback_t will be posted.
    /// The connection will be in the k_ESteamNetworkingConnectionState_Connecting state.
    pub fn create_listen_socket_ip(
        &self,
        local_address: SocketAddr,
        options: impl IntoIterator<Item = NetworkingConfigEntry>,
    ) -> Result<ListenSocket, InvalidHandle> {
        let local_address = SteamIpAddr::from(local_address);
        let options: Vec<_> = options.into_iter().map(|x| x.into()).collect();
        let handle = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_CreateListenSocketIP(
                self.sockets,
                local_address.as_ptr(),
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
    /// Creates a connection and begins talking to a "server" over UDP at the
    /// given IPv4 or IPv6 address.  The remote host must be listening with a
    /// matching call to CreateListenSocketIP on the specified port.
    ///
    /// A SteamNetConnectionStatusChangedCallback_t callback will be triggered when we start
    /// connecting, and then another one on either timeout or successful connection.
    ///
    /// If the server does not have any identity configured, then their network address
    /// will be the only identity in use.  Or, the network host may provide a platform-specific
    /// identity with or without a valid certificate to authenticate that identity.  (These
    /// details will be contained in the SteamNetConnectionStatusChangedCallback_t.)  It's
    /// up to your application to decide whether to allow the connection.
    ///
    /// By default, all connections will get basic encryption sufficient to prevent
    /// casual eavesdropping.  But note that without certificates (or a shared secret
    /// distributed through some other out-of-band mechanism), you don't have any
    /// way of knowing who is actually on the other end, and thus are vulnerable to
    /// man-in-the-middle attacks.
    ///
    /// If you need to set any initial config options, pass them here.  See
    /// SteamNetworkingConfigValue_t for more about why this is preferable to
    /// setting the options "immediately" after creation.
    pub fn connect_by_ip_address(
        &self,
        address: SocketAddr,
        options: impl IntoIterator<Item = NetworkingConfigEntry>,
    ) -> Result<NetConnection, InvalidHandle> {
        let handle = unsafe {
            let address = SteamIpAddr::from(address);
            let options: Vec<_> = options.into_iter().map(|x| x.into()).collect();
            sys::SteamAPI_ISteamNetworkingSockets_ConnectByIPAddress(
                self.sockets,
                address.as_ptr(),
                options.len() as _,
                options.as_ptr(),
            )
        };
        if handle == sys::k_HSteamNetConnection_Invalid {
            Err(InvalidHandle)
        } else {
            Ok(NetConnection::new_independent(
                handle,
                self.sockets,
                self.inner.clone(),
            ))
        }
    }

    /// Like CreateListenSocketIP, but clients will connect using ConnectP2P.
    ///
    /// nLocalVirtualPort specifies how clients can connect to this socket using
    /// ConnectP2P.  It's very common for applications to only have one listening socket;
    /// in that case, use zero.  If you need to open multiple listen sockets and have clients
    /// be able to connect to one or the other, then nLocalVirtualPort should be a small
    /// integer (<1000) unique to each listen socket you create.
    ///
    /// If you use this, you probably want to call ISteamNetworkingUtils::InitRelayNetworkAccess()
    /// when your app initializes.
    ///
    /// If you are listening on a dedicated servers in known data center,
    /// then you can listen using this function instead of CreateHostedDedicatedServerListenSocket,
    /// to allow clients to connect without a ticket.  Any user that owns
    /// the app and is signed into Steam will be able to attempt to connect to
    /// your server.  Also, a connection attempt may require the client to
    /// be connected to Steam, which is one more moving part that may fail.  When
    /// tickets are used, then once a ticket is obtained, a client can connect to
    /// your server even if they got disconnected from Steam or Steam is offline.
    ///
    /// If you need to set any initial config options, pass them here.  See
    /// SteamNetworkingConfigValue_t for more about why this is preferable to
    /// setting the options "immediately" after creation.
    pub fn create_listen_socket_p2p(
        &self,
        local_virtual_port: i32,
        options: impl IntoIterator<Item = NetworkingConfigEntry>,
    ) -> Result<ListenSocket, InvalidHandle> {
        let options: Vec<_> = options.into_iter().map(|x| x.into()).collect();
        let handle = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_CreateListenSocketP2P(
                self.sockets,
                local_virtual_port as _,
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

    /// Begin connecting to a peer that is identified using a platform-specific identifier.
    /// This uses the default rendezvous service, which depends on the platform and library
    /// configuration.  (E.g. on Steam, it goes through the steam backend.)
    ///
    /// If you need to set any initial config options, pass them here.  See
    /// SteamNetworkingConfigValue_t for more about why this is preferable to
    /// setting the options "immediately" after creation.
    ///
    /// To use your own signaling service, see:
    /// - ConnectP2PCustomSignaling
    /// - k_ESteamNetworkingConfig_Callback_CreateConnectionSignaling
    pub fn connect_p2p(
        &self,
        identity_remote: NetworkingIdentity,
        remote_virtual_port: i32,
        options: impl IntoIterator<Item = NetworkingConfigEntry>,
    ) -> Result<NetConnection, InvalidHandle> {
        let handle = unsafe {
            let options: Vec<_> = options.into_iter().map(|x| x.into()).collect();
            sys::SteamAPI_ISteamNetworkingSockets_ConnectP2P(
                self.sockets,
                identity_remote.as_ptr(),
                remote_virtual_port as _,
                options.len() as _,
                options.as_ptr(),
            )
        };
        if handle == sys::k_HSteamNetConnection_Invalid {
            Err(InvalidHandle)
        } else {
            Ok(NetConnection::new_independent(
                handle,
                self.sockets,
                self.inner.clone(),
            ))
        }
    }

    /// Create a listen socket on the specified virtual port.  The physical UDP port to use
    /// will be determined by the SDR_LISTEN_PORT environment variable.  If a UDP port is not
    /// configured, this call will fail.
    ///
    /// This call MUST be made through the SteamGameServerNetworkingSockets() interface.
    ///
    /// This function should be used when you are using the ticket generator library
    /// to issue your own tickets.  Clients connecting to the server on this virtual
    /// port will need a ticket, and they must connect using ConnectToHostedDedicatedServer.
    ///
    /// If you need to set any initial config options, pass them here.  See
    /// SteamNetworkingConfigValue_t for more about why this is preferable to
    /// setting the options "immediately" after creation.
    pub fn create_hosted_dedicated_server_listen_socket(
        &self,
        local_virtual_port: u32,
        options: impl IntoIterator<Item = NetworkingConfigEntry>,
    ) -> Result<ListenSocket, InvalidHandle> {
        let options: Vec<_> = options.into_iter().map(|x| x.into()).collect();
        let handle = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_CreateHostedDedicatedServerListenSocket(
                self.sockets,
                local_virtual_port as _,
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

    /// Indicate our desire to be ready participate in authenticated communications.
    /// If we are currently not ready, then steps will be taken to obtain the necessary
    /// certificates.   (This includes a certificate for us, as well as any CA certificates
    /// needed to authenticate peers.)
    ///
    /// You can call this at program init time if you know that you are going to
    /// be making authenticated connections, so that we will be ready immediately when
    /// those connections are attempted.  (Note that essentially all connections require
    /// authentication, with the exception of ordinary UDP connections with authentication
    /// disabled using k_ESteamNetworkingConfig_IP_AllowWithoutAuth.)  If you don't call
    /// this function, we will wait until a feature is utilized that that necessitates
    /// these resources.
    ///
    /// You can also call this function to force a retry, if failure has occurred.
    /// Once we make an attempt and fail, we will not automatically retry.
    /// In this respect, the behavior of the system after trying and failing is the same
    /// as before the first attempt: attempting authenticated communication or calling
    /// this function will call the system to attempt to acquire the necessary resources.
    ///
    /// You can use GetAuthenticationStatus or listen for SteamNetAuthenticationStatus_t
    /// to monitor the status.
    ///
    /// Returns the current value that would be returned from GetAuthenticationStatus.
    pub fn init_authentication(
        &self,
    ) -> Result<NetworkingAvailability, NetworkingAvailabilityError> {
        unsafe { sys::SteamAPI_ISteamNetworkingSockets_InitAuthentication(self.sockets).try_into() }
    }

    /// Create a new poll group.
    ///
    /// You should destroy the poll group when you are done using DestroyPollGroup
    pub fn create_poll_group(&self) -> NetPollGroup {
        let poll_group =
            unsafe { sys::SteamAPI_ISteamNetworkingSockets_CreatePollGroup(self.sockets) };
        NetPollGroup {
            handle: poll_group,
            sockets: self.sockets,
            inner: self.inner.clone(),
            message_buffer: Vec::new(),
        }
    }

    pub fn get_authentication_status(
        &self,
    ) -> Result<NetworkingAvailability, NetworkingAvailabilityError> {
        let mut details: sys::SteamNetAuthenticationStatus_t = unsafe { std::mem::zeroed() };
        let auth = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_GetAuthenticationStatus(
                self.sockets,
                &mut details,
            )
        };

        auth.try_into()
    }

    /// Returns basic information about the high-level state of the connection.
    ///
    /// Returns false if the connection handle is invalid.
    pub fn get_connection_info(
        &self,
        connection: &NetConnection,
    ) -> Result<NetConnectionInfo, bool> {
        let mut info: sys::SteamNetConnectionInfo_t = unsafe { std::mem::zeroed() };
        let was_successful = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_GetConnectionInfo(
                self.sockets,
                connection.handle,
                &mut info,
            )
        };
        if was_successful {
            Ok(NetConnectionInfo { inner: info })
        } else {
            Err(false)
        }
    }

    /// Returns a small set of information about the real-time state of the connection and the queue status of each lane.
    ///
    /// On entry, lanes specifies the length of the lanes array. This may be 0 if you do not wish to receive any lane data. It's OK for this to be smaller than the total number of configured lanes.
    ///
    /// pLanes points to an array that will receive lane-specific info. It can be NULL if this is not needed.
    pub fn get_realtime_connection_status(
        &self,
        connection: &NetConnection,
        lanes: i32,
    ) -> Result<
        (
            NetConnectionRealTimeInfo,
            Vec<NetConnectionRealTimeLaneStatus>,
        ),
        SteamError,
    > {
        let mut info: sys::SteamNetConnectionRealTimeStatus_t = unsafe { std::mem::zeroed() };
        let mut p_lanes: Vec<sys::SteamNetConnectionRealTimeLaneStatus_t> =
            Vec::with_capacity(lanes as usize);
        let result = unsafe {
            // Get a reference to the uninitialized part of our Vec's buffer
            let uninitialized = p_lanes.spare_capacity_mut();
            let status = sys::SteamAPI_ISteamNetworkingSockets_GetConnectionRealTimeStatus(
                self.sockets,
                connection.handle,
                &mut info,
                lanes,
                uninitialized.as_mut_ptr().cast(),
            );
            // Tell the Vec that we've manually initialized some elements
            p_lanes.set_len(lanes as usize);
            status
        };
        crate::to_steam_result(result).map(|_| {
            (
                NetConnectionRealTimeInfo { inner: info },
                p_lanes
                    .into_iter()
                    .map(|x| NetConnectionRealTimeLaneStatus { inner: x })
                    .collect(),
            )
        })
    }
    /// Configure multiple outbound messages streams ("lanes") on a connection, and control head-of-line blocking between them. Messages within a given lane are always sent in the order they are queued, but messages from different lanes may be sent out of order. Each lane has its own message number sequence. The first message sent on each lane will be assigned the number 1.
    ///
    /// Each lane has a "priority". Lower priority lanes will only be processed when all higher-priority lanes are empty. The magnitudes of the priority values are not relevant, only their sort order. Higher numeric values take priority over lower numeric values.
    ///
    /// Each lane also is assigned a weight, which controls the approximate proportion of the bandwidth that will be consumed by the lane, relative to other lanes of the same priority. (This is assuming the lane stays busy. An idle lane does not build up "credits" to be be spent once a message is queued.) This value is only meaningful as a proportion, relative to other lanes with the same priority. For lanes with different priorities, the strict priority order will prevail, and their weights relative to each other are not relevant. Thus, if a lane has a unique priority value, the weight value for that lane is not relevant.
    ///
    /// Example: 3 lanes, with priorities { 0, 10, 10 } and weights { (NA), 20, 5 }. Messages sent on the first will always be sent first, before messages in the other two lanes. Its weight value is irrelevant, since there are no other lanes with priority=0. The other two lanes will share bandwidth, with the second and third lanes sharing bandwidth using a ratio of approximately 4:1. (The weights { NA, 4, 1 } would be equivalent.)
    pub fn configure_connection_lanes(
        &self,
        connection: &NetConnection,
        num_lanes: i32,
        lane_priorities: &[i32],
        lane_weights: &[u16],
    ) -> Result<(), SteamError> {
        let result = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_ConfigureConnectionLanes(
                self.sockets,
                connection.handle,
                num_lanes,
                lane_priorities.as_ptr(),
                lane_weights.as_ptr(),
            )
        };
        crate::to_steam_result(result)
    }

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

/// A socket that will continually listen for client connections.
/// Call `events()` to receive incoming connection.
/// You should regularly check for events and answer `ConnectionRequests` requests immediately or the socket will
/// appear as unresponsive to the client.
///
/// If a Listen Socket goes out of scope while there are still connections, but new requests will be rejected immediately.
///
/// Listen Socket Events will only be available if steam callback are regularly called.
pub struct ListenSocket {
    inner: Arc<InnerSocket>,
    _callback_handle: Arc<CallbackHandle>,
    receiver: Receiver<ListenSocketEvent>,
}

impl ListenSocket {
    pub(crate) fn new(
        handle: sys::HSteamListenSocket,
        sockets: *mut sys::ISteamNetworkingSockets,
        inner: Arc<Inner>,
    ) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let inner_socket = Arc::new(InnerSocket {
            sockets,
            handle,
            inner: inner.clone(),
        });
        inner
            .networking_sockets_data
            .lock()
            .unwrap()
            .sockets
            .insert(handle, (Arc::downgrade(&inner_socket), sender));
        let callback_handle =
            networking_sockets_callback::get_or_create_connection_callback(inner.clone(), sockets);
        ListenSocket {
            inner: inner_socket,
            _callback_handle: callback_handle,
            receiver,
        }
    }

    /// Tries to receive a pending event. This will never block.
    ///
    /// You should answer ConnectionRequests immediately or the server will appear as unresponsive.
    pub fn try_receive_event(&self) -> Option<ListenSocketEvent> {
        self.receiver.try_recv().ok()
    }

    /// Receive the next event. This will block until the next event is received.
    ///
    /// You should answer ConnectionRequests immediately or the server will appear as unresponsive.
    pub fn receive_event(&self) -> ListenSocketEvent {
        self.receiver
            .recv()
            .expect("all senders were closed, even though the listen socket is still in use")
    }

    /// Returns an iterator for ListenSocketEvents that will block until the next event is received
    ///
    /// You should answer ConnectionRequests immediately or the server will appear as unresponsive.
    pub fn events<'a>(&'a self) -> impl Iterator<Item = ListenSocketEvent> + 'a {
        self.receiver.iter()
    }

    /// Send one or more messages without copying the message payload.
    /// This is the most efficient way to send messages. To use this
    /// function, you must first allocate a message object using
    /// ISteamNetworkingUtils::AllocateMessage.  (Do not declare one
    /// on the stack or allocate your own.)
    ///
    /// You should fill in the message payload.  You can either let
    /// it allocate the buffer for you and then fill in the payload,
    /// or if you already have a buffer allocated, you can just point
    /// m_pData at your buffer and set the callback to the appropriate function
    /// to free it.  Note that if you use your own buffer, it MUST remain valid
    /// until the callback is executed.  And also note that your callback can be
    /// invoked at ant time from any thread (perhaps even before SendMessages
    /// returns!), so it MUST be fast and threadsafe.
    ///
    /// You MUST also fill in:
    /// - m_conn - the handle of the connection to send the message to
    /// - m_nFlags - bitmask of k_nSteamNetworkingSend_xxx flags.
    ///
    /// All other fields are currently reserved and should not be modified.
    ///
    /// The library will take ownership of the message structures.  They may
    /// be modified or become invalid at any time, so you must not read them
    /// after passing them to this function.
    ///
    /// Returns the message number or Steam error for each sent message.
    pub fn send_messages(
        &self,
        messages: impl IntoIterator<Item = NetworkingMessage>,
    ) -> Vec<SteamResult<MessageNumber>> {
        let messages: Vec<_> = messages.into_iter().map(|x| x.take_message()).collect();
        let mut results = vec![0; messages.len()];
        unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_SendMessages(
                self.inner.sockets,
                messages.len() as _,
                messages.as_ptr(),
                results.as_mut_ptr(),
            );
            // Error codes are returned as negative numbers, while positive numbers are message numbers
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

/// Inner struct that keeps sockets alive as long as there is still a connection alive
pub(crate) struct InnerSocket {
    pub(crate) sockets: *mut sys::ISteamNetworkingSockets,
    pub(crate) handle: sys::HSteamListenSocket,
    pub(crate) inner: Arc<Inner>,
}

unsafe impl Send for InnerSocket {}
unsafe impl Sync for InnerSocket {}

impl Drop for InnerSocket {
    fn drop(&mut self) {
        // There's no documentation for this return value, so it's most likely false when hSocket is invalid
        // The handle should always be valid in our case.
        let _was_successful = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_CloseListenSocket(self.sockets, self.handle)
        };

        if let None = self
            .inner
            .networking_sockets_data
            .lock()
            .unwrap()
            .sockets
            .remove(&self.handle)
        {
            eprintln!("error while dropping InnerSocket: socket was already removed")
        }
    }
}

pub struct NetPollGroup {
    handle: sys::HSteamNetPollGroup,
    sockets: *mut sys::ISteamNetworkingSockets,
    inner: Arc<Inner>,
    message_buffer: Vec<*mut SteamNetworkingMessage_t>,
}

unsafe impl Send for NetPollGroup {}
unsafe impl Sync for NetPollGroup {}

impl NetPollGroup {
    pub fn receive_messages(&mut self, batch_size: usize) -> Vec<NetworkingMessage> {
        if self.message_buffer.capacity() < batch_size {
            // reserve(additional) ensures capacity >= len + additional.
            // Since the buffer is always drained between calls, len == 0,
            // so reserve(batch_size) guarantees capacity >= batch_size.
            self.message_buffer.reserve(batch_size);
        }

        unsafe {
            let count = sys::SteamAPI_ISteamNetworkingSockets_ReceiveMessagesOnPollGroup(
                self.sockets,
                self.handle,
                self.message_buffer.as_mut_ptr(),
                batch_size as _,
            ) as usize;
            self.message_buffer.set_len(count);
        }

        self.message_buffer
            .drain(..)
            .map(|x| NetworkingMessage {
                message: x,
                _inner: self.inner.clone(),
            })
            .collect()
    }
}

impl Drop for NetPollGroup {
    fn drop(&mut self) {
        let _was_successful = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_DestroyPollGroup(self.sockets, self.handle)
        };
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
