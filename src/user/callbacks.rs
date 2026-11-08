//! The payloads `ISteamUser` fires on its own: the connection state changes and the
//! micro-transaction authorisation.

use super::*;

// Called when a microtransaction authorization response is received
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MicroTxnAuthorizationResponse {
    pub app_id: AppId,
    pub order_id: u64,
    pub authorized: bool,
}

impl_callback!(cb: MicroTxnAuthorizationResponse_t => MicroTxnAuthorizationResponse {
    Self {
        app_id: cb.m_unAppID.into(),
        order_id: cb.m_ulOrderID.into(),
        authorized: cb.m_bAuthorized == 1,
    }
});

/// Called when a connection to the Steam servers is made.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamServersConnected;

impl_callback!(_cb: SteamServersConnected_t => SteamServersConnected {
    Self
});

/// Called when the connection to the Steam servers is lost.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamServersDisconnected {
    /// The reason we were disconnected from the Steam servers
    pub reason: SteamResult,
}

impl_callback!(cb: SteamServersDisconnected_t => SteamServersDisconnected {
    Self {
        reason: to_steam_result(cb.m_eResult),
    }
});

/// Called when the connection to the Steam servers fails.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamServerConnectFailure {
    /// The reason we failed to connect to the Steam servers
    pub reason: SteamResult,
    /// Whether we are still retrying the connection.
    pub still_retrying: bool,
}

impl_callback!(cb: SteamServerConnectFailure_t => SteamServerConnectFailure {
    Self {
        reason: to_steam_result(cb.m_eResult),
        still_retrying: cb.m_bStillRetrying,
    }
});
