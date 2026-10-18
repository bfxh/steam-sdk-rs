//! The enums that describe *whether* a connection or a subsystem is usable: the high-level
//! connection state, the availability ladder, and the failure answers this binding refuses
//! to name on Steam's numeric enums.

use super::*;

/// High level connection status
///
/// Marked non-exhaustive on purpose: the list is a reading of one SDK version's header, and the answer
/// for "Steam sent something else" is now a variant of its own (`Unknown`), so this enum may grow names
/// without a later build of this crate being unable to represent what it was handed.
#[non_exhaustive]
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
    /// Steam has closed the connection on our side and, from an API perspective, it is closed: no more
    /// data may be sent or received. It still lingers so that an inbound packet can be answered with the
    /// proper reason for the close rather than looking like a timeout.
    ///
    /// Declared as "k_ESteamNetworkingConnectionState_FinWait = -1," (`steamnetworkingtypes.h:430`). This
    /// variant is new here: the enum used to stop at `ProblemDetectedLocally`, so a status change out of
    /// this state had no name -- which is what made one callback path abort and another drop the event.
    FinWait,
    /// The local side has closed the connection but the last reliable data is still being flushed; the
    /// peer has not yet been told the connection is closing, and any data it sends in the meantime is
    /// discarded because the application can no longer read it back.
    ///
    /// Declared as "k_ESteamNetworkingConnectionState_Linger = -2" (`steamnetworkingtypes.h:446`), with
    /// the transition out of it documented as ending in `FinWait`.
    Linger,
    /// "Connection is completely inactive and ready to be destroyed"
    /// (`steamnetworkingtypes.h:448`), declared as "k_ESteamNetworkingConnectionState_Dead = -3,"
    /// (`steamnetworkingtypes.h:449`).
    ///
    /// A connection in this state is finished, but Steam still reports it as the *previous* state of a
    /// status change -- which is exactly the value this crate used to have no answer for.
    Dead,
    /// A number Steam sent that this SDK's header has no name for; the number is kept verbatim.
    ///
    /// The state enum arrives as a plain discriminant inside
    /// `SteamNetConnectionStatusChangedCallback_t`, and bindgen generates it non-exhaustively, so a value
    /// outside the declared list is possible in principle -- including the `__Force32Bit` filler the C
    /// enum uses to pin its width, which is not a state at all and is kept here rather than guessed at.
    /// Code that needs the number can read it back out; code that wants to hand a state *to* Steam cannot,
    /// because there is no constant to name it with (see the `TryFrom` below).
    Unknown(i32),
}

/// The connection-state table, in one place, so both directions agree on it.
///
/// Total by construction: every discriminant the header declares gets a variant of its own, and anything
/// else is kept as `Unknown`. This is what replaced `cb.m_eOldState.try_into().unwrap()` at the status
/// callback -- a number like `Dead` or `FinWait` was a guaranteed panic there, because the table stopped
/// at `ProblemDetectedLocally`.
pub(crate) fn state_from_raw(
    raw: sys::ESteamNetworkingConnectionState,
) -> NetworkingConnectionState {
    use sys::ESteamNetworkingConnectionState::*;
    match raw {
        k_ESteamNetworkingConnectionState_None => NetworkingConnectionState::None,
        k_ESteamNetworkingConnectionState_Connecting => NetworkingConnectionState::Connecting,
        k_ESteamNetworkingConnectionState_FindingRoute => NetworkingConnectionState::FindingRoute,
        k_ESteamNetworkingConnectionState_Connected => NetworkingConnectionState::Connected,
        k_ESteamNetworkingConnectionState_ClosedByPeer => NetworkingConnectionState::ClosedByPeer,
        k_ESteamNetworkingConnectionState_ProblemDetectedLocally => {
            NetworkingConnectionState::ProblemDetectedLocally
        }
        k_ESteamNetworkingConnectionState_FinWait => NetworkingConnectionState::FinWait,
        k_ESteamNetworkingConnectionState_Linger => NetworkingConnectionState::Linger,
        k_ESteamNetworkingConnectionState_Dead => NetworkingConnectionState::Dead,
        other => NetworkingConnectionState::Unknown(other as i32),
    }
}

#[cfg(test)]
mod state_table_tests {
    //! The connection-state table on its own, without a client. What is being pinned is that the table
    //! covers the whole list Steam declares: the three negative states used to be missing, and a status
    //! change whose previous state was one of them reached `try_into().unwrap()` in the callback.
    use super::{state_from_raw, InvalidConnectionState, NetworkingConnectionState as State};
    use steamworks_sys as sys;
    use sys::ESteamNetworkingConnectionState as Raw;

    #[test]
    fn every_state_steam_declares_has_a_name_of_its_own() {
        let pairs = [
            (Raw::k_ESteamNetworkingConnectionState_None, State::None),
            (
                Raw::k_ESteamNetworkingConnectionState_Connecting,
                State::Connecting,
            ),
            (
                Raw::k_ESteamNetworkingConnectionState_FindingRoute,
                State::FindingRoute,
            ),
            (
                Raw::k_ESteamNetworkingConnectionState_Connected,
                State::Connected,
            ),
            (
                Raw::k_ESteamNetworkingConnectionState_ClosedByPeer,
                State::ClosedByPeer,
            ),
            (
                Raw::k_ESteamNetworkingConnectionState_ProblemDetectedLocally,
                State::ProblemDetectedLocally,
            ),
            (
                Raw::k_ESteamNetworkingConnectionState_FinWait,
                State::FinWait,
            ),
            (Raw::k_ESteamNetworkingConnectionState_Linger, State::Linger),
            (Raw::k_ESteamNetworkingConnectionState_Dead, State::Dead),
        ];
        for (raw, named) in pairs {
            assert_eq!(state_from_raw(raw), named, "{:?}", raw as i32);
        }
        assert_eq!(pairs.len(), 9, "the header declares nine states");
    }

    /// The regression this variant exists for: `Dead`, `FinWait` and `Linger` are ordinary previous
    /// states, and reading them used to be the error case -- which the status callback unwrapped.
    #[test]
    fn a_teardown_previous_state_is_a_state_not_an_error() {
        for raw in [
            Raw::k_ESteamNetworkingConnectionState_FinWait,
            Raw::k_ESteamNetworkingConnectionState_Linger,
            Raw::k_ESteamNetworkingConnectionState_Dead,
        ] {
            let read = State::try_from(raw);
            assert!(read.is_ok(), "{:?} must not be an error", raw as i32);
            assert!(
                !matches!(read.unwrap(), State::None),
                "{:?} is not the dummy value",
                raw as i32
            );
        }
    }

    /// The width-pinning filler the C enum declares is not a connection state, and the table keeps the
    /// number instead of guessing which state it "probably" means.
    #[test]
    fn an_undeclared_discriminant_keeps_its_number() {
        assert_eq!(
            state_from_raw(Raw::k_ESteamNetworkingConnectionState__Force32Bit),
            State::Unknown(i32::MAX)
        );
    }

    #[test]
    fn named_states_round_trip_through_the_raw_enum() {
        for raw in [
            Raw::k_ESteamNetworkingConnectionState_None,
            Raw::k_ESteamNetworkingConnectionState_Connecting,
            Raw::k_ESteamNetworkingConnectionState_FindingRoute,
            Raw::k_ESteamNetworkingConnectionState_Connected,
            Raw::k_ESteamNetworkingConnectionState_ClosedByPeer,
            Raw::k_ESteamNetworkingConnectionState_ProblemDetectedLocally,
            Raw::k_ESteamNetworkingConnectionState_FinWait,
            Raw::k_ESteamNetworkingConnectionState_Linger,
            Raw::k_ESteamNetworkingConnectionState_Dead,
        ] {
            let named = state_from_raw(raw);
            assert_eq!(Raw::try_from(named).ok(), Some(raw), "{:?}", raw as i32);
        }
    }

    /// The one thing the outgoing direction genuinely cannot do: hand back a number it has no constant
    /// for. Reachable, which is why this direction is a `TryFrom` and not the `From` it used to be.
    #[test]
    fn an_unknown_number_refuses_to_be_sent_back_to_steam() {
        let err = Raw::try_from(State::Unknown(4242)).unwrap_err();
        assert!(format!("{:?}", err).contains("Invalid"));
        assert!(matches!(
            Raw::try_from(State::Unknown(4242)),
            Err(InvalidConnectionState)
        ));
    }
}

impl TryFrom<NetworkingConnectionState> for sys::ESteamNetworkingConnectionState {
    type Error = InvalidConnectionState;

    /// Hand a state back to Steam. `Unknown` has no answer: the number arrived from Steam, and there is
    /// no constant in this SDK's header to carry it outward, so the conversion refuses rather than
    /// substituting a neighbour. This direction used to be an infallible `From`, which could not have
    /// said that -- it had no `Unknown` to be asked about.
    fn try_from(state: NetworkingConnectionState) -> Result<Self, Self::Error> {
        use sys::ESteamNetworkingConnectionState::*;
        Ok(match state {
            NetworkingConnectionState::None => k_ESteamNetworkingConnectionState_None,
            NetworkingConnectionState::Connecting => k_ESteamNetworkingConnectionState_Connecting,
            NetworkingConnectionState::FindingRoute => {
                k_ESteamNetworkingConnectionState_FindingRoute
            }
            NetworkingConnectionState::Connected => k_ESteamNetworkingConnectionState_Connected,
            NetworkingConnectionState::ClosedByPeer => {
                k_ESteamNetworkingConnectionState_ClosedByPeer
            }
            NetworkingConnectionState::ProblemDetectedLocally => {
                k_ESteamNetworkingConnectionState_ProblemDetectedLocally
            }
            NetworkingConnectionState::FinWait => k_ESteamNetworkingConnectionState_FinWait,
            NetworkingConnectionState::Linger => k_ESteamNetworkingConnectionState_Linger,
            NetworkingConnectionState::Dead => k_ESteamNetworkingConnectionState_Dead,
            NetworkingConnectionState::Unknown(_) => return Err(InvalidConnectionState),
        })
    }
}

impl TryFrom<sys::ESteamNetworkingConnectionState> for NetworkingConnectionState {
    type Error = InvalidConnectionState;

    /// Kept fallible in its signature, because `NetConnectionInfo::state()` and
    /// `NetConnectionError::UnknownType` name `InvalidConnectionState`, and retiring that here would be a
    /// wider break than the one being made. The error arm is unreachable: `state_from_raw` names every
    /// discriminant Steam declares and keeps the rest as `Unknown`.
    fn try_from(state: sys::ESteamNetworkingConnectionState) -> Result<Self, Self::Error> {
        Ok(state_from_raw(state))
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
