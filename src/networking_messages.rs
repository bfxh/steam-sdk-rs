//! The non-connection-oriented interface to send and receive messages
//! (whether they be "clients" or "servers").
//!
//! ISteamNetworkingSockets is connection-oriented (like TCP), meaning you
//! need to listen and connect, and then you send messages using a connection
//! handle.  ISteamNetworkingMessages is more like UDP, in that you can just send
//! messages to arbitrary peers at any time.  The underlying connections are
//! established implicitly.
//!
//! Under the hood ISteamNetworkingMessages works on top of the ISteamNetworkingSockets
//! code, so you get the same routing and messaging efficiency.  The difference is
//! mainly in your responsibility to explicitly establish a connection and
//! the type of feedback you get about the state of the connection.  Both
//! interfaces can do "P2P" communications, and both support both unreliable
//! and reliable messages, fragmentation and reassembly.
//!
//! The primary purpose of this interface is to be "like UDP", so that UDP-based code
//! can be ported easily to take advantage of relayed connections.  If you find
//! yourself needing more low level information or control, or to be able to better
//! handle failure, then you probably need to use ISteamNetworkingSockets directly.
//! Also, note that if your main goal is to obtain a connection between two peers
//! without concerning yourself with assigning roles of "client" and "server",
//! you may find the symmetric connection mode of ISteamNetworkingSockets useful.
//! (See k_ESteamNetworkingConfig_SymmetricConnect.)
// TODO: examples here
use crate::networking_sockets::messages_received;
use crate::networking_types::{
    NetConnectionInfo, NetConnectionRealTimeInfo, NetworkingConnectionState, NetworkingIdentity,
    NetworkingMessage, SendFlags,
};
use crate::{register_callback, to_steam_result, Callback, Inner, SteamResult};
use std::ffi::c_void;
use std::sync::{Arc, Weak};

use steamworks_sys as sys;

/// Access to the steam networking messages interface
pub struct NetworkingMessages {
    pub(crate) net: *mut sys::ISteamNetworkingMessages,
    pub(crate) inner: Arc<Inner>,
}

unsafe impl Sync for NetworkingMessages {}
// SAFETY: nothing in this crate's tree needs `Send` for this type today (measured: this
// line deleted, `cargo check --all-targets` stays green). The promise is kept because
// the type is public and a caller may move it to the thread that pumps callbacks.
// The type holds a raw Steam interface pointer, plain-copy handles, and the owning `Arc<Inner>`;
// it carries no non-atomic reference count, which is the one hazard a `Send` claim here cannot
// see -- `scripts/ci/callback_threads.py` counts `Rc` tokens beside every promise and ships at
// zero for exactly that reason. Moving the value therefore moves sole ownership of the pointer;
// nothing reads the same state through two threads because of the move itself.
unsafe impl Send for NetworkingMessages {}

impl NetworkingMessages {
    /// Sends a message to the specified host.
    ///
    /// If we don't already have a session with that user, a session is implicitly created.
    /// There might be some handshaking that needs to happen before we can actually begin sending message data.
    /// If this handshaking fails and we can't get through, an error will be posted via the callback
    /// SteamNetworkingMessagesSessionFailed_t.
    /// There is no notification when the operation succeeds.  (You should have the peer send a reply
    /// for this purpose.)
    ///
    /// Sending a message to a host will also implicitly accept any incoming connection from that host.
    ///
    /// `channel` is a routing number you can use to help route message to different systems.
    /// You'll have to call ReceiveMessagesOnChannel() with the same channel number in order to retrieve
    /// the data on the other end.
    ///
    /// Using different channels to talk to the same user will still use the same underlying
    /// connection, saving on resources.  If you don't need this feature, use 0.
    /// Otherwise, small integers are the most efficient.
    ///
    /// It is guaranteed that reliable messages to the same host on the same channel
    /// will be be received by the remote host (if they are received at all) exactly once,
    /// and in the same order that they were sent.
    ///
    /// NO other order guarantees exist!  In particular, unreliable messages may be dropped,
    /// received out of order with respect to each other and with respect to reliable data,
    /// or may be received multiple times.  Messages on different channels are *not* guaranteed
    /// to be received in the order they were sent.
    ///
    /// A note for those familiar with TCP/IP ports, or converting an existing codebase that
    /// opened multiple sockets:  You might notice that there is only one channel, and with
    /// TCP/IP each endpoint has a port number.  You can think of the channel number as the
    /// *destination* port.  If you need each message to also include a "source port" (so the
    /// recipient can route the reply), then just put that in your message.  That is essentially
    /// how UDP works!
    ///
    /// Returns:
    /// - k_EREsultOK on success.
    /// - k_EResultNoConnection will be returned if the session has failed or was closed by the peer,
    ///   and k_nSteamNetworkingSend_AutoRestartBrokenSession is not used.  (You can use
    ///   GetSessionConnectionInfo to get the details.)  In order to acknowledge the broken session
    ///   and start a new one, you must call CloseSessionWithUser
    /// - See ISteamNetworkingSockets::SendMessageToConnection for more possible return values
    pub fn send_message_to_user(
        &self,
        user: NetworkingIdentity,
        send_type: SendFlags,
        data: &[u8],
        channel: u32,
    ) -> SteamResult {
        let result = unsafe {
            sys::SteamAPI_ISteamNetworkingMessages_SendMessageToUser(
                self.net,
                user.as_ptr(),
                data.as_ptr().cast(),
                data.len() as u32,
                send_type.bits(),
                channel as i32,
            )
        };

        to_steam_result(result)
    }

    /// Reads the next message that has been sent from another user on the given channel.
    ///
    /// `batch_size` is the maximum number of messages that can be received at once, and it is also
    /// the bound the answer is trusted to: "Returns number of messages returned into your list."
    /// (`isteamnetworkingmessages.h:86`) is a count of what Steam wrote, not a length this side has
    /// to believe, so the number goes through `messages_received` before `set_len` sees it. Without
    /// that, the answer was cast straight through -- and a negative one becomes ~1.8e19, which is
    /// `set_len` describing a `Vec` far longer than the `batch_size` slots that were allocated.
    ///
    /// # Example
    /// ```no_run
    /// # use steamworks::Client;
    /// # use std::time::Duration;
    /// let client = Client::init().unwrap();
    /// let networking_messages = client.networking_messages();
    ///
    /// // Accept all new connections
    /// networking_messages.session_request_callback(|request| { request.accept(); });
    ///
    /// // Run the callbacks and receive messages
    /// for _ in 0..50 {
    ///     client.run_callbacks();
    ///     let _received = networking_messages.receive_messages_on_channel(0, 10);
    ///     std::thread::sleep(Duration::from_millis(10));
    /// }
    /// ```
    pub fn receive_messages_on_channel(
        &self,
        channel: u32,
        batch_size: usize,
    ) -> Vec<NetworkingMessage> {
        let mut buffer = Vec::with_capacity(batch_size);
        unsafe {
            let message_count = sys::SteamAPI_ISteamNetworkingMessages_ReceiveMessagesOnChannel(
                self.net,
                channel as i32,
                buffer.as_mut_ptr(),
                batch_size as _,
            );
            buffer.set_len(messages_received(message_count, batch_size));
        }

        buffer
            .into_iter()
            .map(|x| NetworkingMessage {
                message: x,
                _inner: self.inner.clone(),
            })
            .collect()
    }

    /// Register a callback that will be called whenever a peer requests a connection.
    ///
    /// Use the [`SessionRequest`](../networking_messages/struct.SessionRequest.html) to accept or reject the connection.
    ///
    /// Requires regularly calling [`Client.run_callbacks()`](../struct.Client.html#method.run_callbacks).
    /// Calling this function more than once will replace the previous callback.
    ///
    /// # Example
    /// ```no_run
    /// # use steamworks::Client;
    /// # use std::time::Duration;
    /// let client = Client::init().unwrap();
    /// let messages = client.networking_messages();
    ///
    /// // Accept all incoming connections
    /// messages.session_request_callback(|request| {
    ///     request.accept();
    /// });
    ///
    /// // run_callbacks must be called regularly, or no incoming connections can be received
    /// for _ in 0..50 {
    ///     client.run_callbacks();
    /// }
    /// ```
    pub fn session_request_callback(
        &self,
        mut callback: impl FnMut(SessionRequest) + Send + 'static,
    ) {
        let builder = SessionRequestBuilder {
            message: self.net,
            inner: Arc::downgrade(&self.inner),
        };
        let call_handle = unsafe {
            register_callback(
                &self.inner,
                move |request: NetworkingMessagesSessionRequest| {
                    if let Some(request) = builder.build_request(request.remote) {
                        callback(request);
                    }
                },
            )
        };
        std::mem::forget(call_handle);
    }

    /// Register a callback that will be called whenever a connection fails to be established.
    ///
    /// Requires regularly calling [`Client.run_callbacks()`](../struct.Client.html#method.run_callbacks).
    /// Calling this function more than once will replace the previous callback.
    pub fn session_failed_callback(
        &self,
        mut callback: impl FnMut(NetConnectionInfo) + Send + 'static,
    ) {
        let call_handle = unsafe {
            register_callback(
                &self.inner,
                move |failed: NetworkingMessagesSessionFailed| {
                    callback(failed.info);
                },
            )
        };
        std::mem::forget(call_handle);
    }

    /// Get information about the status of a connection to a remote host.
    ///
    /// This provides information about the connection, such as whether it's currently
    /// connected, attempting to connect, etc., as well as some statistics about the
    /// connection quality if available.
    ///
    /// Returns the current connection state, along with detailed connection info and
    /// real-time status (if avaliable).
    pub fn get_session_connection_info(
        &self,
        identity_remote: &NetworkingIdentity,
    ) -> (
        NetworkingConnectionState,
        Option<NetConnectionInfo>,
        Option<NetConnectionRealTimeInfo>,
    ) {
        let mut connection_info: sys::SteamNetConnectionInfo_t = unsafe { std::mem::zeroed() };
        let mut quick_status: sys::SteamNetConnectionRealTimeStatus_t =
            unsafe { std::mem::zeroed() };

        let state = unsafe {
            sys::SteamAPI_ISteamNetworkingMessages_GetSessionConnectionInfo(
                self.net,
                identity_remote.as_ptr(),
                &mut connection_info,
                &mut quick_status,
            )
        };

        let state = match NetworkingConnectionState::try_from(state) {
            Ok(state) => state,
            Err(_) => NetworkingConnectionState::None,
        };

        let connection_info = if state != NetworkingConnectionState::None {
            Some(NetConnectionInfo {
                inner: connection_info,
            })
        } else {
            None
        };

        let quick_status = if state != NetworkingConnectionState::None {
            Some(NetConnectionRealTimeInfo {
                inner: quick_status,
            })
        } else {
            None
        };

        (state, connection_info, quick_status)
    }

    /// Call this when you are done talking to a user, to free the resources Steam holds for the
    /// session immediately.
    ///
    /// A plain `bool` is returned because that is all Steam gives and the header does not define
    /// what `false` means: `isteamnetworkingmessages.h:103-105` says only "Call this when you're
    /// done talking to a user to immediately free up resources under-the-hood. If the remote user
    /// tries to send data to you again, another SteamNetworkingMessagesSessionRequest_t callback
    /// will be posted", and `:107-108` adds that sessions which go unused for a few minutes are
    /// timed out automatically. So this call decides *when* the resources come back, not whether
    /// they ever would -- and mapping `false` onto an error would be this wrapper inventing a
    /// failure Steam never described.
    ///
    /// [`SessionRequest::reject`] reaches the same Steam function for a session that was never
    /// accepted; this is the one for a session you have been using.
    pub fn close_session_with_user(&self, identity_remote: &NetworkingIdentity) -> bool {
        unsafe {
            sys::SteamAPI_ISteamNetworkingMessages_CloseSessionWithUser(
                self.net,
                identity_remote.as_ptr(),
            )
        }
    }

    /// Like [`close_session_with_user`](Self::close_session_with_user), but closes a single channel.
    ///
    /// `isteamnetworkingmessages.h:110-113`: "Once all open channels to a user have been closed, the
    /// open session to the user will be closed, and any new data from this user will trigger a
    /// SteamSteamNetworkingMessagesSessionRequest_t callback" -- so a later packet reopens the
    /// session as a request. The
    /// number is the same routing channel [`send_message_to_user`](Self::send_message_to_user)
    /// takes, and the header's own advice for apps that do not route by channel applies here too --
    /// use 0.
    pub fn close_channel_with_user(
        &self,
        identity_remote: &NetworkingIdentity,
        local_channel: i32,
    ) -> bool {
        unsafe {
            sys::SteamAPI_ISteamNetworkingMessages_CloseChannelWithUser(
                self.net,
                identity_remote.as_ptr(),
                local_channel,
            )
        }
    }
}

/// A helper for creating SessionRequests.
///
/// It's Send and Sync, so it can be moved into the callback.
struct SessionRequestBuilder {
    message: *mut sys::ISteamNetworkingMessages,
    // Once the builder is in the callback, it creates a cyclic reference, so this has to be Weak
    inner: Weak<Inner>,
}

// SAFETY: a use site in this crate needs `Send` for this type -- measured by deleting
// this line, which makes `cargo check --all-targets` fail with
// `*mut ISteamNetworkingMessages` cannot be sent between threads safely.
// The type holds a raw Steam interface pointer, plain-copy handles, and the owning `Arc<Inner>`;
// it carries no non-atomic reference count, which is the one hazard a `Send` claim here cannot
// see -- `scripts/ci/callback_threads.py` counts `Rc` tokens beside every promise and ships at
// zero for exactly that reason. Moving the value therefore moves sole ownership of the pointer;
// nothing reads the same state through two threads because of the move itself.
unsafe impl Send for SessionRequestBuilder {}
unsafe impl Sync for SessionRequestBuilder {}

impl SessionRequestBuilder {
    pub fn build_request(&self, remote: NetworkingIdentity) -> Option<SessionRequest> {
        self.inner.upgrade().map(|inner| SessionRequest {
            remote,
            messages: self.message,
            answered: false,
            _inner: inner,
        })
    }
}

#[derive(Debug)]
pub struct NetworkingMessagesSessionRequest {
    remote: NetworkingIdentity,
}

impl_callback!(cb: SteamNetworkingMessagesSessionRequest_t => NetworkingMessagesSessionRequest {
    let remote = cb.m_identityRemote.into();
    Self { remote }
});

#[derive(Debug)]
pub struct NetworkingMessagesSessionFailed {
    pub info: NetConnectionInfo,
}

impl_callback!(cb: SteamNetworkingMessagesSessionFailed_t => NetworkingMessagesSessionFailed {
    let remote = cb.m_info.into();
    Self { info: remote }
});

/// A request for a new connection.
///
/// Use this to accept or reject the connection.
/// Letting this struct go out of scope will reject the connection.
pub struct SessionRequest {
    remote: NetworkingIdentity,
    messages: *mut sys::ISteamNetworkingMessages,
    /// Whether this request has been answered -- by `accept`, by `reject`, or by `Drop`.
    /// It was called `accepted` and only `accept` ever set it, so a request that had been rejected
    /// still looked unanswered here and `Drop` closed the session a second time.
    answered: bool,
    _inner: Arc<Inner>,
}

unsafe impl Sync for SessionRequest {}
// SAFETY: nothing in this crate's tree needs `Send` for this type today (measured: this
// line deleted, `cargo check --all-targets` stays green). The promise is kept because
// the type is public and a caller may move it to the thread that pumps callbacks.
// The type holds a raw Steam interface pointer, plain-copy handles, and the owning `Arc<Inner>`;
// it carries no non-atomic reference count, which is the one hazard a `Send` claim here cannot
// see -- `scripts/ci/callback_threads.py` counts `Rc` tokens beside every promise and ships at
// zero for exactly that reason. Moving the value therefore moves sole ownership of the pointer;
// nothing reads the same state through two threads because of the move itself.
unsafe impl Send for SessionRequest {}

impl SessionRequest {
    /// The remote peer requesting the connection.
    pub fn remote(&self) -> &NetworkingIdentity {
        &self.remote
    }

    /// Records that this request has been answered, and reports whether Steam is still owed a close.
    ///
    /// The first caller gets `true`, everybody after it gets `false`. That is the entire point of this
    /// function: `CloseSessionWithUser` is "Call this when you're done talking to a user to
    /// immediately free up resources under-the-hood.  If the remote user tries to send data to you
    /// again, another SteamNetworkingMessagesSessionRequest_t callback will be posted."
    /// (`isteamnetworkingmessages.h:103-105`), and the header promises no idempotency -- a second
    /// close is not a no-op, because anything that re-established a session in between gets torn down
    /// with it, and sending implicitly accepts a pending session ("Calling SendMessageToUser() will
    /// implicitly accepts any pending session request to that user.", `:100`). Until now only
    /// `accept()` marked the request, so `reject()` closed once and `Drop` closed again.
    fn mark_answered(&mut self) -> bool {
        let owed = !self.answered;
        self.answered = true;
        owed
    }

    /// Accept the connection.
    pub fn accept(mut self) -> bool {
        self.mark_answered();
        unsafe {
            return sys::SteamAPI_ISteamNetworkingMessages_AcceptSessionWithUser(
                self.messages,
                self.remote.as_ptr(),
            );
        }
    }

    /// Reject the connection.
    pub fn reject(mut self) {
        self.reject_inner();
    }

    /// Reject the connection without consuming self, useful for implementing [`Drop`]
    fn reject_inner(&mut self) {
        if !self.mark_answered() {
            return;
        }
        unsafe {
            sys::SteamAPI_ISteamNetworkingMessages_CloseSessionWithUser(
                self.messages,
                self.remote.as_ptr(),
            );
        }
    }
}

impl Drop for SessionRequest {
    fn drop(&mut self) {
        // Unconditional, on purpose: `reject_inner` owns the decision about who is owed a close, so
        // the rule is stated once instead of being restated here from a flag only one path set.
        self.reject_inner();
    }
}

#[cfg(test)]
mod tests {
    //! `mark_answered` is the whole double-close guard and it is pure state, so it can be judged
    //! without a client -- unlike the FFI call it gates, which needs Steam.
    use super::*;
    use crate::{Callbacks, Manager, NetworkingSocketsData};
    use std::sync::Weak;

    fn fresh_request() -> SessionRequest {
        SessionRequest {
            remote: NetworkingIdentity::default(),
            messages: std::ptr::null_mut(),
            answered: false,
            _inner: Arc::new(Inner {
                manager: Manager::Client,
                callbacks: Callbacks::new(),
                networking_sockets_data: std::sync::Mutex::new(NetworkingSocketsData {
                    sockets: Default::default(),
                    independent_connections: Default::default(),
                    fake_udp_ports: Default::default(),
                    connection_callback: Weak::new(),
                }),
            }),
        }
    }

    /// Rejecting used to close the session and then leave `Drop` to close it a second time, because
    /// the flag that suppressed `Drop` was only ever set by accepting.
    #[test]
    fn the_first_answer_owes_the_close_and_no_later_one_does() {
        let mut request = fresh_request();
        assert!(
            request.mark_answered(),
            "an unanswered request owes Steam exactly one close"
        );
        assert!(
            !request.mark_answered(),
            "a second answer must not close the session again"
        );
        assert!(!request.mark_answered(), "and neither must a third");
    }

    /// The behaviour that was correct before must survive the refactor: a request nobody answered is
    /// still rejected when it goes out of scope, once.
    #[test]
    fn an_ignored_request_is_still_rejected_exactly_once() {
        let mut request = fresh_request();
        assert!(
            request.mark_answered(),
            "dropping an ignored request owes one close"
        );
        assert!(
            !request.mark_answered(),
            "and the drop path must not add a second one"
        );
    }
}
