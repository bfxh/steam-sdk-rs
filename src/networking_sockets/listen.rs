//! Opening a listening socket: by IP, for P2P, and the hosted-dedicated-server variant. All
//! three answer asynchronously through a callback and hand back a `ListenSocket`.

use super::*;

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
        // The entries stay alive across this call, because a string option's bytes belong to
        // the entry it came from: Steam "Just saves your pointer.  Does NOT make a copy of the
        // string" (`steamnetworkingtypes.h:1787`) and dereferences it while consuming this array.
        let held: Vec<_> = options.into_iter().collect();
        let options: Vec<_> = held.iter().map(NetworkingConfigEntry::as_raw).collect();
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
        // The entries stay alive across this call, because a string option's bytes belong to
        // the entry it came from: Steam "Just saves your pointer.  Does NOT make a copy of the
        // string" (`steamnetworkingtypes.h:1787`) and dereferences it while consuming this array.
        let held: Vec<_> = options.into_iter().collect();
        let options: Vec<_> = held.iter().map(NetworkingConfigEntry::as_raw).collect();
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
        // The entries stay alive across this call, because a string option's bytes belong to
        // the entry it came from: Steam "Just saves your pointer.  Does NOT make a copy of the
        // string" (`steamnetworkingtypes.h:1787`) and dereferences it while consuming this array.
        let held: Vec<_> = options.into_iter().collect();
        let options: Vec<_> = held.iter().map(NetworkingConfigEntry::as_raw).collect();
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
}
