//! Reaching out to somebody else: a direct IP connection and a P2P connection addressed by
//! networking identity.

use super::*;

impl NetworkingSockets {
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
            // The entries stay alive across this call, because a string option's bytes belong to
            // the entry it came from: Steam "Just saves your pointer.  Does NOT make a copy of the
            // string" (`steamnetworkingtypes.h:1787`) and dereferences it while consuming this array.
            let held: Vec<_> = options.into_iter().collect();
            let options: Vec<_> = held.iter().map(NetworkingConfigEntry::as_raw).collect();
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
            // The entries stay alive across this call, because a string option's bytes belong to
            // the entry it came from: Steam "Just saves your pointer.  Does NOT make a copy of the
            // string" (`steamnetworkingtypes.h:1787`) and dereferences it while consuming this array.
            let held: Vec<_> = options.into_iter().collect();
            let options: Vec<_> = held.iter().map(NetworkingConfigEntry::as_raw).collect();
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
}
