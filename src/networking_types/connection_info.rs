//! What a connection looks like right now: its identity, its state, and the round-trip and
//! lane measurements Steam keeps updating.
//!
//! Split out because these three types are the read side of a live connection -- they wrap
//! Steam's own `*_t` structs and their `Debug` and `From` impls -- while the rest of the module
//! is enums, error codes and event payloads.

use super::*;

/// Struct to get info about this specific connection.
///
/// Things you might want to get:
/// * Identity (SteamID or IP Address)
/// * Connection State: connecting, ongoing, disconnected by peer, ...
/// * End Reason if there is one
/// * Custom User Data, as an i64
#[derive(Clone)]
pub struct NetConnectionInfo {
    pub(crate) inner: sys::SteamNetConnectionInfo_t,
}

#[allow(dead_code)]
impl NetConnectionInfo {
    /// Return the network identity of the remote peer.
    ///
    /// Depending on the connection type and phase of the connection, it may be unknown, in which case `None` is returned.
    /// If `Some` is returned, the return value is a valid `NetworkingIdentity`.
    pub fn identity_remote(&self) -> Option<NetworkingIdentity> {
        let identity = NetworkingIdentity::from(self.inner.m_identityRemote);
        if identity.is_valid() {
            Some(identity)
        } else {
            None
        }
    }

    pub fn user_data(&self) -> i64 {
        self.inner.m_nUserData
    }

    pub fn listen_socket(&self) -> Option<sys::HSteamNetConnection> {
        let handle = self.inner.m_hListenSocket;
        if handle == sys::k_HSteamListenSocket_Invalid {
            None
        } else {
            Some(handle)
        }
    }

    pub fn state(&self) -> Result<NetworkingConnectionState, InvalidConnectionState> {
        self.inner.m_eState.try_into()
    }

    pub fn end_reason(&self) -> Option<NetConnectionEnd> {
        if self.inner.m_eEndReason == ESteamNetConnectionEnd::k_ESteamNetConnectionEnd_Invalid as _
        {
            None
        } else {
            Some(
                self.inner
                    .m_eEndReason
                    .try_into()
                    .expect("Unknown end reason could not be converted"),
            )
        }
    }
}

impl Debug for NetConnectionInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetConnectionInfo")
            .field("identity_remote", &self.identity_remote())
            .field("user_data", &self.user_data())
            .field("listen_socket", &self.listen_socket())
            .field("state", &self.state())
            .field("end_reason", &self.end_reason())
            .finish()
    }
}

impl From<sys::SteamNetConnectionInfo_t> for NetConnectionInfo {
    fn from(info: steamworks_sys::SteamNetConnectionInfo_t) -> Self {
        Self { inner: info }
    }
}

/// SteamNetConnectionRealTimeStatus_t structure
#[derive(Clone)]
pub struct NetConnectionRealTimeInfo {
    pub(crate) inner: sys::SteamNetConnectionRealTimeStatus_t,
}

impl NetConnectionRealTimeInfo {
    pub fn connection_state(&self) -> Result<NetworkingConnectionState, InvalidConnectionState> {
        self.inner.m_eState.try_into()
    }

    // ping in ms
    pub fn ping(&self) -> i32 {
        self.inner.m_nPing
    }

    /// Connection quality measured locally, 0...1.  (Percentage of packets delivered)
    pub fn connection_quality_local(&self) -> f32 {
        self.inner.m_flConnectionQualityLocal
    }

    /// Packet delivery success rate as observed from remote host
    pub fn connection_quality_remote(&self) -> f32 {
        self.inner.m_flConnectionQualityRemote
    }

    /// Current data rates from recent history
    pub fn out_packets_per_sec(&self) -> f32 {
        self.inner.m_flOutPacketsPerSec
    }

    /// Current data rates from recent history
    pub fn out_bytes_per_sec(&self) -> f32 {
        self.inner.m_flOutBytesPerSec
    }
    /// Current data rates from recent history
    pub fn in_packets_per_sec(&self) -> f32 {
        self.inner.m_flInPacketsPerSec
    }

    /// Current data rates from recent history
    pub fn in_bytes_per_sec(&self) -> f32 {
        self.inner.m_flInBytesPerSec
    }

    /// Estimate rate that we believe that we can send data to our peer.
    /// Note that this could be significantly higher than m_flOutBytesPerSec,
    /// meaning the capacity of the channel is higher than you are sending data.
    /// (That's OK!)
    pub fn send_rate_bytes_per_sec(&self) -> i32 {
        self.inner.m_nSendRateBytesPerSecond
    }
    /// Number of bytes pending to be sent.  This is data that you have recently
    /// requested to be sent but has not yet actually been put on the wire.  The
    /// reliable number ALSO includes data that was previously placed on the wire,
    /// but has now been scheduled for re-transmission.  Thus, it's possible to
    /// observe m_cbPendingReliable increasing between two checks, even if no
    /// calls were made to send reliable data between the checks.  Data that is
    /// awaiting the Nagle delay will appear in these numbers.
    pub fn pending_unreliable(&self) -> i32 {
        self.inner.m_cbPendingUnreliable
    }
    /// Number of bytes pending to be sent.  This is data that you have recently
    /// requested to be sent but has not yet actually been put on the wire.  The
    /// reliable number ALSO includes data that was previously placed on the wire,
    /// but has now been scheduled for re-transmission.  Thus, it's possible to
    /// observe m_cbPendingReliable increasing between two checks, even if no
    /// calls were made to send reliable data between the checks.  Data that is
    /// awaiting the Nagle delay will appear in these numbers.
    pub fn pending_reliable(&self) -> i32 {
        self.inner.m_cbPendingReliable
    }

    /// Number of bytes of reliable data that has been placed the wire, but
    /// for which we have not yet received an acknowledgment, and thus we may
    /// have to re-transmit.
    pub fn sent_unacked_reliable(&self) -> i32 {
        self.inner.m_cbSentUnackedReliable
    }

    /// If you asked us to send a message right now, how long would that message
    /// sit in the queue before we actually started putting packets on the wire?
    /// (And assuming Nagle does not cause any packets to be delayed.)
    ///
    /// In general, data that is sent by the application is limited by the
    /// bandwidth of the channel.  If you send data faster than this, it must
    /// be queued and put on the wire at a metered rate.  Even sending a small amount
    /// of data (e.g. a few MTU, say ~3k) will require some of the data to be delayed
    /// a bit.
    ///
    /// In general, the estimated delay will be approximately equal to
    ///
    /// `( m_cbPendingUnreliable+m_cbPendingReliable ) / m_nSendRateBytesPerSecond`
    ///
    /// plus or minus one MTU.  It depends on how much time has elapsed since the last
    /// packet was put on the wire.  For example, the queue might have *just* been emptied,
    /// and the last packet placed on the wire, and we are exactly up against the send
    /// rate limit.  In that case we might need to wait for one packet's worth of time to
    /// elapse before we can send again.  On the other extreme, the queue might have data
    /// in it waiting for Nagle.  (This will always be less than one packet, because as soon
    /// as we have a complete packet we would send it.)  In that case, we might be ready
    /// to send data now, and this value will be 0.
    pub fn queued_send_bytes(&self) -> i64 {
        self.inner.m_usecQueueTime
    }
}

impl Debug for NetConnectionRealTimeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NetQuickConnectionInfo")
            .field("connection_state", &self.connection_state())
            .field("ping", &self.ping())
            .field("connection_quality_local", &self.connection_quality_local())
            .field(
                "connection_quality_remote",
                &self.connection_quality_remote(),
            )
            .field("out_packets_per_sec", &self.out_packets_per_sec())
            .field("out_bytes_per_sec", &self.out_bytes_per_sec())
            .field("in_packets_per_sec", &self.in_packets_per_sec())
            .field("in_bytes_per_sec", &self.in_bytes_per_sec())
            .field("send_rate_bytes_per_sec", &self.send_rate_bytes_per_sec())
            .field("pending_unreliable", &self.pending_unreliable())
            .field("pending_reliable", &self.pending_reliable())
            .field("sent_unacked_reliable", &self.sent_unacked_reliable())
            .field("queued_send_bytes", &self.queued_send_bytes())
            .finish()
    }
}

impl From<sys::SteamNetConnectionRealTimeStatus_t> for NetConnectionRealTimeInfo {
    fn from(info: steamworks_sys::SteamNetConnectionRealTimeStatus_t) -> Self {
        Self { inner: info }
    }
}

/// Quick status of a particular lane
#[derive(Clone)]
pub struct NetConnectionRealTimeLaneStatus {
    pub(crate) inner: sys::SteamNetConnectionRealTimeLaneStatus_t,
}

impl NetConnectionRealTimeLaneStatus {
    /// Number of bytes pending to be sent.  This is data that you have recently
    /// requested to be sent but has not yet actually been put on the wire.  The
    /// reliable number ALSO includes data that was previously placed on the wire,
    /// but has now been scheduled for re-transmission.  Thus, it's possible to
    /// observe m_cbPendingReliable increasing between two checks, even if no
    /// calls were made to send reliable data between the checks.  Data that is
    /// awaiting the Nagle delay will appear in these numbers.
    /// Lane-specific, for global look at NetConnectionRealTimeInfo.
    pub fn pending_unreliable(&self) -> i32 {
        self.inner.m_cbPendingUnreliable
    }
    /// Number of bytes pending to be sent.  This is data that you have recently
    /// requested to be sent but has not yet actually been put on the wire.  The
    /// reliable number ALSO includes data that was previously placed on the wire,
    /// but has now been scheduled for re-transmission.  Thus, it's possible to
    /// observe m_cbPendingReliable increasing between two checks, even if no
    /// calls were made to send reliable data between the checks.  Data that is
    /// awaiting the Nagle delay will appear in these numbers.
    /// Lane-specific, for global look at NetConnectionRealTimeInfo.
    pub fn pending_reliable(&self) -> i32 {
        self.inner.m_cbPendingReliable
    }
    /// Number of bytes of reliable data that has been placed the wire, but
    /// for which we have not yet received an acknowledgment, and thus we may
    /// have to re-transmit.
    /// Lane-specific, for global look at NetConnectionRealTimeInfo.
    pub fn sent_unacked_reliable(&self) -> i32 {
        self.inner.m_cbSentUnackedReliable
    }
    /// Lane-specific queue time.  This value takes into consideration lane priorities
    /// and weights, and how much data is queued in each lane, and attempts to predict
    /// how any data currently queued will be sent out.
    pub fn queued_send_bytes(&self) -> i64 {
        self.inner.m_usecQueueTime
    }
}

impl From<sys::SteamNetConnectionRealTimeLaneStatus_t> for NetConnectionRealTimeLaneStatus {
    fn from(info: steamworks_sys::SteamNetConnectionRealTimeLaneStatus_t) -> Self {
        Self { inner: info }
    }
}
