//! What Steam reports happening to connections and listen sockets: the status-change
//! callback payload, the event enums, and the request / connected / disconnected structs a
//! game is handed before a connection exists.

use super::*;

/// This callback is posted whenever a connection is created, destroyed, or changes state.
/// The m_info field will contain a complete description of the connection at the time the
/// change occurred and the callback was posted.  In particular, m_eState will have the
/// new connection state.
///
/// You will usually need to listen for this callback to know when:
/// - A new connection arrives on a listen socket.
///   m_info.m_hListenSocket will be set, m_eOldState = k_ESteamNetworkingConnectionState_None,
///   and m_info.m_eState = k_ESteamNetworkingConnectionState_Connecting.
///   See ISteamNetworkigSockets::AcceptConnection.
/// - A connection you initiated has been accepted by the remote host.
///   m_eOldState = k_ESteamNetworkingConnectionState_Connecting, and
///   m_info.m_eState = k_ESteamNetworkingConnectionState_Connected.
///   Some connections might transition to k_ESteamNetworkingConnectionState_FindingRoute first.
/// - A connection has been actively rejected or closed by the remote host.
///   m_eOldState = k_ESteamNetworkingConnectionState_Connecting or k_ESteamNetworkingConnectionState_Connected,
///   and m_info.m_eState = k_ESteamNetworkingConnectionState_ClosedByPeer.  m_info.m_eEndReason
///   and m_info.m_szEndDebug will have for more details.
///   NOTE: upon receiving this callback, you must still destroy the connection using
///   ISteamNetworkingSockets::CloseConnection to free up local resources.  (The details
///   passed to the function are not used in this case, since the connection is already closed.)
/// - A problem was detected with the connection, and it has been closed by the local host.
///   The most common failure is timeout, but other configuration or authentication failures
///   can cause this.  m_eOldState = k_ESteamNetworkingConnectionState_Connecting or
///   k_ESteamNetworkingConnectionState_Connected, and m_info.m_eState = k_ESteamNetworkingConnectionState_ProblemDetectedLocally.
///   m_info.m_eEndReason and m_info.m_szEndDebug will have for more details.
///   NOTE: upon receiving this callback, you must still destroy the connection using
///   ISteamNetworkingSockets::CloseConnection to free up local resources.  (The details
///   passed to the function are not used in this case, since the connection is already closed.)
///
/// Remember that callbacks are posted to a queue, and networking connections can
/// change at any time.  It is possible that the connection has already changed
/// state by the time you process this callback.
///
/// Also note that callbacks will be posted when connections are created and destroyed by your own API calls.
#[derive(Debug, Clone)]
pub struct NetConnectionStatusChanged {
    /// The handle of the connection that has changed state
    // (only important for the ListenSocketEvent, so it can stay for now in the crate visibility)
    pub(crate) connection: sys::HSteamNetConnection,
    /// Full connection info
    pub connection_info: NetConnectionInfo,

    // Debug is intentionally ignored during dead-code analysis
    #[allow(dead_code)]
    /// Previous state.  (Current state is in m_info.m_eState)
    pub old_state: NetworkingConnectionState,
}

impl_callback!(cb: SteamNetConnectionStatusChangedCallback_t => NetConnectionStatusChanged {
    Self {
        connection: cb.m_hConn,
        connection_info: cb.m_info.into(),
        old_state: cb.m_eOldState.try_into().unwrap(),
    }
});

impl NetConnectionStatusChanged {
    pub(crate) fn into_listen_socket_event(
        self,
        socket: Arc<InnerSocket>,
    ) -> Result<ListenSocketEvent, NetConnectionError> {
        match self.connection_info.state() {
            Ok(NetworkingConnectionState::None) => {
                Err(UnhandledType(NetworkingConnectionState::None))
            }
            Ok(NetworkingConnectionState::Connecting) => {
                if let Some(remote) = self.connection_info.identity_remote() {
                    Ok(ListenSocketEvent::Connecting(ConnectionRequest {
                        remote,
                        user_data: self.connection_info.user_data(),
                        connection: NetConnection::new(
                            self.connection,
                            socket.sockets,
                            socket.inner.clone(),
                            socket,
                        ),
                    }))
                } else {
                    return Err(NetConnectionError::InvalidRemote);
                }
            }
            Ok(NetworkingConnectionState::FindingRoute) => {
                Err(UnhandledType(NetworkingConnectionState::FindingRoute))
            }
            Ok(NetworkingConnectionState::Connected) => {
                if let Some(remote) = self.connection_info.identity_remote() {
                    Ok(ListenSocketEvent::Connected(ConnectedEvent {
                        remote,
                        user_data: self.connection_info.user_data(),
                        connection: NetConnection::new(
                            self.connection,
                            socket.sockets,
                            socket.inner.clone(),
                            socket.clone(),
                        ),
                    }))
                } else {
                    return Err(NetConnectionError::InvalidRemote);
                }
            }
            Ok(NetworkingConnectionState::ClosedByPeer)
            | Ok(NetworkingConnectionState::ProblemDetectedLocally) => {
                if let Some(remote) = self.connection_info.identity_remote() {
                    Ok(ListenSocketEvent::Disconnected(DisconnectedEvent {
                        remote,
                        user_data: self.connection_info.user_data(),
                        // `NetConnectionInfo::end_reason` is `None` for one input only: the code Steam
                        // itself spells `k_ESteamNetConnectionEnd_Invalid`, which is how it says no reason
                        // was recorded. The disconnection is the fact the listener needs, so the event is
                        // delivered carrying that sentinel instead of aborting the callback that was
                        // asked to route it.
                        end_reason: self
                            .connection_info
                            .end_reason()
                            .unwrap_or(NetConnectionEnd::Invalid),
                    }))
                } else {
                    return Err(NetConnectionError::InvalidRemote);
                }
            }
            Err(err) => Err(NetConnectionError::UnknownType(err)),
        }
    }
}

pub enum ListenSocketEvent {
    Connecting(ConnectionRequest),
    Connected(ConnectedEvent),
    Disconnected(DisconnectedEvent),
}

#[derive(Debug)]
pub struct NetConnectionEvent {
    pub new_state: NetworkingConnectionState,
    pub old_state: NetworkingConnectionState,
}

pub struct ConnectionRequest {
    remote: NetworkingIdentity,
    user_data: i64,
    connection: NetConnection,
}

impl ConnectionRequest {
    pub fn remote(&self) -> NetworkingIdentity {
        self.remote.clone()
    }

    pub fn user_data(&self) -> i64 {
        self.user_data
    }

    pub fn accept(self) -> SteamResult {
        self.connection.accept()
    }

    pub fn reject(self, end_reason: NetConnectionEnd, debug_string: Option<&str>) -> bool {
        self.connection.close(end_reason, debug_string, false)
    }
}

pub struct ConnectedEvent {
    remote: NetworkingIdentity,
    user_data: i64,
    connection: NetConnection,
}

impl ConnectedEvent {
    pub fn remote(&self) -> NetworkingIdentity {
        self.remote.clone()
    }
    pub fn user_data(&self) -> i64 {
        self.user_data
    }
    pub fn connection(&self) -> &NetConnection {
        &self.connection
    }

    pub fn take_connection(self) -> NetConnection {
        self.connection
    }
}

pub struct DisconnectedEvent {
    remote: NetworkingIdentity,
    user_data: i64,
    end_reason: NetConnectionEnd,
}

impl DisconnectedEvent {
    pub fn remote(&self) -> NetworkingIdentity {
        self.remote.clone()
    }
    pub fn user_data(&self) -> i64 {
        self.user_data
    }
    pub fn end_reason(&self) -> NetConnectionEnd {
        self.end_reason
    }
}

#[derive(Debug, Error)]
pub(crate) enum NetConnectionError {
    #[error("internal event type has no corresponding external event")]
    UnhandledType(NetworkingConnectionState),
    #[error("invalid event type")]
    UnknownType(InvalidConnectionState),
    #[error("invalid remote")]
    InvalidRemote,
}
