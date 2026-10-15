//! The enums that describe *whether* a connection or a subsystem is usable: the high-level
//! connection state, the availability ladder, and the failure answers this binding refuses
//! to name on Steam's numeric enums.

use super::*;

/// High level connection status
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum NetworkingConnectionState {
    /// Dummy value used to indicate an error condition in the API.
    /// Specified connection doesn't exist or has already been closed.
    None,
    /// We are trying to establish whether peers can talk to each other,
    /// whether they WANT to talk to each other, perform basic auth,
    /// and exchange crypt keys.
    ///
    /// - For connections on the "client" side (initiated locally):
    ///   We're in the process of trying to establish a connection.
    ///   Depending on the connection type, we might not know who they are.
    ///   Note that it is not possible to tell if we are waiting on the
    ///   network to complete handshake packets, or for the application layer
    ///   to accept the connection.
    ///
    /// - For connections on the "server" side (accepted through listen socket):
    ///   We have completed some basic handshake and the client has presented
    ///   some proof of identity.  The connection is ready to be accepted
    ///   using AcceptConnection().
    ///
    /// In either case, any unreliable packets sent now are almost certain
    /// to be dropped.  Attempts to receive packets are guaranteed to fail.
    /// You may send messages if the send mode allows for them to be queued.
    /// but if you close the connection before the connection is actually
    /// established, any queued messages will be discarded immediately.
    /// (We will not attempt to flush the queue and confirm delivery to the
    /// remote host, which ordinarily happens when a connection is closed.)
    Connecting,
    /// Some connection types use a back channel or trusted 3rd party
    /// for earliest communication.  If the server accepts the connection,
    /// then these connections switch into the rendezvous state.  During this
    /// state, we still have not yet established an end-to-end route (through
    /// the relay network), and so if you send any messages unreliable, they
    /// are going to be discarded.
    FindingRoute,
    /// We've received communications from our peer (and we know
    /// who they are) and are all good.  If you close the connection now,
    /// we will make our best effort to flush out any reliable sent data that
    /// has not been acknowledged by the peer.  (But note that this happens
    /// from within the application process, so unlike a TCP connection, you are
    /// not totally handing it off to the operating system to deal with it.)
    Connected,
    /// Connection has been closed by our peer, but not closed locally.
    /// The connection still exists from an API perspective.  You must close the
    /// handle to free up resources.  If there are any messages in the inbound queue,
    /// you may retrieve them.  Otherwise, nothing may be done with the connection
    /// except to close it.
    ///
    /// This stats is similar to CLOSE_WAIT in the TCP state machine.
    ClosedByPeer,
    /// A disruption in the connection has been detected locally.  (E.g. timeout,
    /// local internet connection disrupted, etc.)
    ///
    /// The connection still exists from an API perspective.  You must close the
    /// handle to free up resources.
    ///
    /// Attempts to send further messages will fail.  Any remaining received messages
    /// in the queue are available.
    ProblemDetectedLocally,
}

impl From<NetworkingConnectionState> for sys::ESteamNetworkingConnectionState {
    fn from(state: NetworkingConnectionState) -> Self {
        match state {
            NetworkingConnectionState::None => sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_None,
            NetworkingConnectionState::Connecting => sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_Connecting,
            NetworkingConnectionState::FindingRoute => sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_FindingRoute,
            NetworkingConnectionState::Connected => sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_Connected,
            NetworkingConnectionState::ClosedByPeer => sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_ClosedByPeer,
            NetworkingConnectionState::ProblemDetectedLocally => sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_ProblemDetectedLocally,
        }
    }
}

impl TryFrom<sys::ESteamNetworkingConnectionState> for NetworkingConnectionState {
    type Error = InvalidConnectionState;

    fn try_from(state: sys::ESteamNetworkingConnectionState) -> Result<Self, Self::Error> {
        match state {
            sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_None => Ok(NetworkingConnectionState::None),
            sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_Connecting => Ok(NetworkingConnectionState::Connecting),
            sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_FindingRoute => Ok(NetworkingConnectionState::FindingRoute),
            sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_Connected => Ok(NetworkingConnectionState::Connected),
            sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_ClosedByPeer => Ok(NetworkingConnectionState::ClosedByPeer),
            sys::ESteamNetworkingConnectionState::k_ESteamNetworkingConnectionState_ProblemDetectedLocally => Ok(NetworkingConnectionState::ProblemDetectedLocally),
            _ => Err(InvalidConnectionState)
        }
    }
}

#[derive(Debug, Error)]
#[error("Invalid state")]
pub struct InvalidConnectionState;

pub type NetworkingAvailabilityResult = Result<NetworkingAvailability, NetworkingAvailabilityError>;

/// Describe the status of a particular network resource
#[derive(Debug, Eq, PartialEq, Hash, Copy, Clone)]
pub enum NetworkingAvailability {
    /// We don't know because we haven't ever checked/tried
    NeverTried,
    /// We're waiting on a dependent resource to be acquired.  (E.g. we cannot obtain a cert until we are logged into Steam.  We cannot measure latency to relays until we have the network config.)
    Waiting,
    /// We're actively trying now, but are not yet successful.
    Attempting,
    /// Resource is online/available
    Current,
}

/// Describe a error of a particular network resource
/// In general, we will not automatically retry unless you take some action that
/// depends on of requests this resource, such as querying the status, attempting
/// to initiate a connection, receive a connection, etc.  If you do not take any
#[derive(Debug, Error, Eq, PartialEq, Hash, Copy, Clone)]
pub enum NetworkingAvailabilityError {
    /// Internal dummy/sentinal. The network resource is probably not initialized yet
    #[error("unknown")]
    Unknown,
    /// A dependent resource is missing, so this service is unavailable.  (E.g. we cannot talk to routers because Internet is down or we don't have the network config.)
    #[error("A dependent resource is missing, so this service is unavailable.")]
    CannotTry,
    /// We have tried for enough time that we would expect to have been successful by now.  We have never been successful
    #[error("We have tried for enough time that we would expect to have been successful by now.  We have never been successful")]
    Failed,
    /// We tried and were successful at one time, but now it looks like we have a problem
    #[error("We tried and were successful at one time, but now it looks like we have a problem")]
    Previously,
    /// We previously failed and are currently retrying
    #[error("We previously failed and are currently retrying")]
    Retrying,
}

impl TryFrom<sys::ESteamNetworkingAvailability> for NetworkingAvailability {
    type Error = NetworkingAvailabilityError;

    fn try_from(value: sys::ESteamNetworkingAvailability) -> Result<Self, Self::Error> {
        match value {
            sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_Unknown => {
                Err(NetworkingAvailabilityError::Unknown)
            }
            sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_CannotTry => {
                Err(NetworkingAvailabilityError::CannotTry)
            }
            sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_Failed => {
                Err(NetworkingAvailabilityError::Failed)
            }
            sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_Previously => {
                Err(NetworkingAvailabilityError::Previously)
            }
            sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_Retrying => {
                Err(NetworkingAvailabilityError::Retrying)
            }
            sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_NeverTried => {
                Ok(NetworkingAvailability::NeverTried)
            }
            sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_Waiting => {
                Ok(NetworkingAvailability::Waiting)
            }
            sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_Attempting => {
                Ok(NetworkingAvailability::Attempting)
            }
            sys::ESteamNetworkingAvailability::k_ESteamNetworkingAvailability_Current => {
                Ok(NetworkingAvailability::Current)
            }
            _ => panic!("invalid networking availability {:?}", value),
        }
    }
}

#[derive(Debug, Error)]
#[error("integer value could not be converted to enum")]
pub struct InvalidEnumValue;
