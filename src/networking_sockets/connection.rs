//! A live connection handle: the send, query and close operations that need a
//! `HSteamNetConnection`, plus the RAII drop that releases it back to Steam.
//!
//! Split out of the socket module because this is the type a caller talks to per peer, and its
//! method list grows independently of the listener and poll-group plumbing next door.

use super::*;
use crate::utils::c_string_prefix;

pub struct NetConnection {
    pub(crate) handle: sys::HSteamNetConnection,
    sockets: *mut sys::ISteamNetworkingSockets,
    inner: Arc<Inner>,
    socket: Option<Arc<InnerSocket>>,
    _callback_handle: Option<Arc<CallbackHandle>>,
    event_receiver: Option<Receiver<NetConnectionEvent>>,
    message_buffer: Vec<*mut SteamNetworkingMessage_t>,
    is_handled: bool,
}

unsafe impl Send for NetConnection {}
unsafe impl Sync for NetConnection {}

impl NetConnection {
    pub(crate) fn new(
        handle: sys::HSteamNetConnection,
        sockets: *mut sys::ISteamNetworkingSockets,
        inner: Arc<Inner>,
        socket: Arc<InnerSocket>,
    ) -> Self {
        NetConnection {
            handle,
            sockets,
            inner,
            socket: Some(socket),
            _callback_handle: None,
            event_receiver: None,
            message_buffer: Vec::new(),
            is_handled: false,
        }
    }

    pub(crate) fn new_independent(
        handle: sys::HSteamNetConnection,
        sockets: *mut sys::ISteamNetworkingSockets,
        inner: Arc<Inner>,
    ) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        crate::utils::mutex_guard(&inner.networking_sockets_data)
            .independent_connections
            .insert(handle, sender);
        let callback =
            networking_sockets_callback::get_or_create_connection_callback(inner.clone(), sockets);
        NetConnection {
            handle,
            sockets,
            inner,
            socket: None,
            _callback_handle: Some(callback),
            event_receiver: Some(receiver),
            message_buffer: Vec::new(),
            is_handled: false,
        }
    }

    /// Create a NetConnection without a callback for internal use (e.g. instantly rejecting connection requests to dropped sockets)
    /// Don't use this for exposed connections, it is not set up correctly.
    pub(crate) fn new_internal(
        handle: sys::HSteamNetConnection,
        sockets: *mut sys::ISteamNetworkingSockets,
        inner: Arc<Inner>,
    ) -> Self {
        NetConnection {
            handle,
            sockets,
            inner,
            socket: None,
            _callback_handle: None,
            event_receiver: None,
            message_buffer: Vec::new(),
            is_handled: false,
        }
    }

    pub fn info(&self) -> Result<NetConnectionInfo, InvalidHandle> {
        let mut steam_net_conn_info = std::mem::MaybeUninit::uninit();

        let was_successful = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_GetConnectionInfo(
                self.sockets,
                self.handle,
                steam_net_conn_info.as_mut_ptr(),
            )
        };

        if was_successful {
            let steam_net_conn_info = unsafe { steam_net_conn_info.assume_init() };
            Ok(NetConnectionInfo::from(steam_net_conn_info))
        } else {
            Err(InvalidHandle)
        }
    }

    /// The FakeIP address Steam knows this connection's peer by.
    ///
    /// Only a connection made through the FakeIP system has an answer, and which of the two kinds
    /// Steam reports is its own choice: if the peer "had a global FakeIP at the time the connection
    /// was established" that is the one returned, and otherwise Steam allocates one "unique locally"
    /// from its own address space (`isteamnetworkingsockets.h:880-885`). The second kind is why two
    /// connections to the same host can differ: "since the namespace is limited, this cannot be
    /// guaranteed" (`:890`), Steam only tries to be consistent (`:887-889`).
    ///
    /// The answer is a [`std::net::SocketAddr`] because that is the shape every address in this
    /// crate speaks; Steam's out parameter is the general `SteamNetworkingIPAddr`, which carries
    /// IPv4 in its mapped form, and the decode is `crate::networking_types`' own.
    ///
    /// Both refusals come back as [`SteamError`] instead of as an empty address, and this is the one
    /// handle-taking query here that does not answer [`InvalidHandle`]: Steam reports a bad handle as
    /// a status -- "k_EResultInvalidParam: invalid connection handle" -- while the ordinary case of a
    /// connection that was never made through the FakeIP system is its own code,
    /// "k_EResultIPNotFound: This connection wasn't made using FakeIP system" (`:892-894`). A caller
    /// that treats the two alike will read a closed connection as merely a non-FakeIP one.
    pub fn remote_fake_ip(&self) -> SteamResult<SocketAddr> {
        // Zero-filled before the call so the struct is initialised bytes whatever Steam does with
        // it: on the error paths below nothing is read out of it, and on the success path every byte
        // this wrapper reads is a byte Steam wrote.
        let mut raw = std::mem::MaybeUninit::<sys::SteamNetworkingIPAddr>::zeroed();
        let code = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_GetRemoteFakeIPForConnection(
                self.sockets,
                self.handle,
                raw.as_mut_ptr(),
            )
        };
        crate::to_steam_result(code)?;
        Ok(SteamIpAddr::from(unsafe { raw.assume_init() }).to_socket_addr())
    }

    /// Clear the poll group for a connection.
    ///
    /// Returns `Err(InvalidHandle)` when `connection` is invalid.
    pub fn clear_poll_group(&self) -> Result<(), InvalidHandle> {
        let was_successful = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_SetConnectionPollGroup(
                self.sockets,
                self.handle,
                sys::k_HSteamNetPollGroup_Invalid,
            )
        };

        if was_successful {
            Ok(())
        } else {
            Err(InvalidHandle)
        }
    }

    /// Accept an incoming connection that has been received on a listen socket.
    /// This is internally used in `ConnectionRequest` and should not be called on regular connections.
    ///
    /// When a connection attempt is received (perhaps after a few basic handshake
    /// packets have been exchanged to prevent trivial spoofing), a connection interface
    /// object is created in the k_ESteamNetworkingConnectionState_Connecting state
    /// and a SteamNetConnectionStatusChangedCallback_t is posted.  At this point, your
    /// application MUST either accept or close the connection.  (It may not ignore it.)
    /// Accepting the connection will transition it either into the connected state,
    /// or the finding route state, depending on the connection type.
    ///
    /// You should take action within a second or two, because accepting the connection is
    /// what actually sends the reply notifying the client that they are connected.  If you
    /// delay taking action, from the client's perspective it is the same as the network
    /// being unresponsive, and the client may timeout the connection attempt.  In other
    /// words, the client cannot distinguish between a delay caused by network problems
    /// and a delay caused by the application.
    ///
    /// This means that if your application goes for more than a few seconds without
    /// processing callbacks (for example, while loading a map), then there is a chance
    /// that a client may attempt to connect in that interval and fail due to timeout.
    ///
    /// If the application does not respond to the connection attempt in a timely manner,
    /// and we stop receiving communication from the client, the connection attempt will
    /// be timed out locally, transitioning the connection to the
    /// k_ESteamNetworkingConnectionState_ProblemDetectedLocally state.  The client may also
    /// close the connection before it is accepted, and a transition to the
    /// k_ESteamNetworkingConnectionState_ClosedByPeer is also possible depending the exact
    /// sequence of events.
    ///
    /// Returns k_EResultInvalidParam if the handle is invalid.
    /// Returns k_EResultInvalidState if the connection is not in the appropriate state.
    /// (Remember that the connection state could change in between the time that the
    /// notification being posted to the queue and when it is received by the application.)
    ///
    /// A note about connection configuration options.  If you need to set any configuration
    /// options that are common to all connections accepted through a particular listen
    /// socket, consider setting the options on the listen socket, since such options are
    /// inherited automatically.  If you really do need to set options that are connection
    /// specific, it is safe to set them on the connection before accepting the connection.
    pub(crate) fn accept(mut self) -> SteamResult {
        self.handle_connection();
        let result = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_AcceptConnection(self.sockets, self.handle)
        };
        crate::to_steam_result(result)
    }

    /// Disconnects from the remote host and invalidates the connection handle.
    /// Any unread data on the connection is discarded.
    ///
    /// nReason is an application defined code that will be received on the other
    /// end and recorded (when possible) in backend analytics.  The value should
    /// come from a restricted range.  (See ESteamNetConnectionEnd.)  If you don't need
    /// to communicate any information to the remote host, and do not want analytics to
    /// be able to distinguish "normal" connection terminations from "exceptional" ones,
    /// You may pass zero, in which case the generic value of
    /// k_ESteamNetConnectionEnd_App_Generic will be used.
    ///
    /// pszDebug is an optional human-readable diagnostic string that will be received
    /// by the remote host and recorded (when possible) in backend analytics.
    ///
    /// If you wish to put the socket into a "linger" state, where an attempt is made to
    /// flush any remaining sent data, use bEnableLinger=true.  Otherwise reliable data
    /// is not flushed.
    ///
    /// If the connection has already ended and you are just freeing up the
    /// connection interface, the reason code, debug string, and linger flag are
    /// ignored.
    pub fn close(
        mut self,
        reason: NetConnectionEnd,
        debug_string: Option<&str>,
        enable_linger: bool,
    ) -> bool {
        let held = debug_string_prefix(debug_string);
        let debug_string_ptr = held
            .as_ref()
            .map_or(std::ptr::null(), |string| string.as_ptr());
        self.handle_connection();
        unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_CloseConnection(
                self.sockets,
                self.handle,
                reason.into(),
                debug_string_ptr,
                enable_linger,
            )
        }
    }

    /// Fetch connection user data.  Returns -1 if handle is invalid
    /// or if you haven't set any userdata on the connection.
    pub fn connection_user_data(&self) -> Result<i64, InvalidHandle> {
        let user_data = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_GetConnectionUserData(self.sockets, self.handle)
        };
        if user_data == -1 {
            // I'm not sure if a connection can become invalid on its own, so returning a result may be unnecessary
            Err(InvalidHandle)
        } else {
            Ok(user_data)
        }
    }

    /// Set connection user data.  the data is returned in the following places
    /// - You can query it using GetConnectionUserData.
    /// - The SteamNetworkingmessage_t structure.
    /// - The SteamNetConnectionInfo_t structure.  (Which is a member of SteamNetConnectionStatusChangedCallback_t.)
    ///
    /// Returns false if the handle is invalid.
    pub fn set_connection_user_data(&self, user_data: i64) -> Result<(), InvalidHandle> {
        let was_successful = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_SetConnectionUserData(
                self.sockets,
                self.handle,
                user_data,
            )
        };
        if was_successful {
            Ok(())
        } else {
            Err(InvalidHandle)
        }
    }

    /// Set a name for the connection, used mostly for debugging
    pub fn set_connection_name(&self, name: &str) {
        let name = c_string_prefix(name);
        unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_SetConnectionName(
                self.sockets,
                self.handle,
                name.as_ptr(),
            )
        }
    }

    /// Send a message to the remote host on the specified connection.
    ///
    /// nSendFlags determines the delivery guarantees that will be provided,
    /// when data should be buffered, etc.  E.g. k_nSteamNetworkingSend_Unreliable
    ///
    /// Note that the semantics we use for messages are not precisely
    /// the same as the semantics of a standard "stream" socket.
    /// (SOCK_STREAM)  For an ordinary stream socket, the boundaries
    /// between chunks are not considered relevant, and the sizes of
    /// the chunks of data written will not necessarily match up to
    /// the sizes of the chunks that are returned by the reads on
    /// the other end.  The remote host might read a partial chunk,
    /// or chunks might be coalesced.  For the message semantics
    /// used here, however, the sizes WILL match.  Each send call
    /// will match a successful read call on the remote host
    /// one-for-one.  If you are porting existing stream-oriented
    /// code to the semantics of reliable messages, your code should
    /// work the same, since reliable message semantics are more
    /// strict than stream semantics.  The only caveat is related to
    /// performance: there is per-message overhead to retain the
    /// message sizes, and so if your code sends many small chunks
    /// of data, performance will suffer. Any code based on stream
    /// sockets that does not write excessively small chunks will
    /// work without any changes.
    ///
    /// The pOutMessageNumber is an optional pointer to receive the
    /// message number assigned to the message, if sending was successful.
    ///
    /// Returns:
    /// - k_EResultInvalidParam: invalid connection handle, or the individual message is too big.
    ///   (See k_cbMaxSteamNetworkingSocketsMessageSizeSend)
    /// - k_EResultInvalidState: connection is in an invalid state
    /// - k_EResultNoConnection: connection has ended
    /// - k_EResultIgnored: You used k_nSteamNetworkingSend_NoDelay, and the message was dropped because
    ///   we were not ready to send it.
    /// - k_EResultLimitExceeded: there was already too much data queued to be sent.
    ///   (See k_ESteamNetworkingConfig_SendBufferSize)
    pub fn send_message(&self, data: &[u8], send_flags: SendFlags) -> SteamResult<MessageNumber> {
        unsafe {
            let mut out_message_number = 0i64;
            let result = sys::SteamAPI_ISteamNetworkingSockets_SendMessageToConnection(
                self.sockets,
                self.handle,
                data.as_ptr() as _,
                data.len() as _,
                send_flags.bits(),
                &mut out_message_number,
            );
            crate::to_steam_result(result).map(|_| MessageNumber(out_message_number as u64))
        }
    }

    /// The connection's debug name, as Steam last heard [`Self::set_connection_name`].
    ///
    /// This used to be `unimplemented!()`, so every call panicked -- the name a wrapper can set, no
    /// wrapper could read. Steam's side is "Set a name for the connection, used mostly for debugging"
    /// and "Fetch connection name.  Returns false if handle is invalid"
    /// (`isteamnetworkingsockets.h:225-229`); the fetch takes the buffer *and its size* from the
    /// caller, and the headers give no maximum for the name itself -- `k_cchSteamNetworkingMaxConnectionAppName`
    /// is the app name, not this one. So `CONNECTION_NAME_BYTES` is this crate's ceiling, not Steam's:
    /// a name set longer than 256 bytes comes back cut at 256, and nothing in the answer says so.
    pub fn connection_name(&self) -> Result<String, InvalidHandle> {
        let mut buffer = [0 as std::os::raw::c_char; CONNECTION_NAME_BYTES];
        let ok = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_GetConnectionName(
                self.sockets,
                self.handle,
                buffer.as_mut_ptr(),
                CONNECTION_NAME_BYTES as ::std::os::raw::c_int,
            )
        };
        name_answer(ok, &buffer)
    }

    /// Flush any messages waiting on the Nagle timer and send them
    /// at the next transmission opportunity (often that means right now).
    ///
    /// If Nagle is enabled (it's on by default) then when calling
    /// SendMessageToConnection the message will be buffered, up to the Nagle time
    /// before being sent, to merge small messages into the same packet.
    /// (See k_ESteamNetworkingConfig_NagleTime)
    ///
    /// Returns:
    /// k_EResultInvalidParam: invalid connection handle
    /// k_EResultInvalidState: connection is in an invalid state
    /// k_EResultNoConnection: connection has ended
    /// k_EResultIgnored: We weren't (yet) connected, so this operation has no effect.
    pub fn flush_messages(&self) -> SteamResult {
        unsafe {
            let result = sys::SteamAPI_ISteamNetworkingSockets_FlushMessagesOnConnection(
                self.sockets,
                self.handle,
            );
            crate::to_steam_result(result)
        }
    }

    /// Internal function used by `receive_messages` and `receive_messages_noalloc`.
    ///
    /// Steam's documented answer for an invalid handle is -1 ("If the connection handle is invalid,
    /// -1 is returned." -- `isteamnetworkingsockets.h:321`), which is why this one reports
    /// `InvalidHandle` instead of reading zero messages; the upper bound comes from
    /// `messages_received`, the same clamp the poll group and the Fake UDP port use, so a count above
    /// the slots offered cannot widen the buffer past what the caller reserved.
    fn receive_messages_internal(&mut self, batch_size: usize) -> Result<usize, InvalidHandle> {
        debug_assert!(self.message_buffer.capacity() >= batch_size);
        self.message_buffer.clear();
        let message_count = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_ReceiveMessagesOnConnection(
                self.sockets,
                self.handle,
                self.message_buffer.as_mut_ptr(),
                batch_size as _,
            )
        };
        if message_count < 0 {
            return Err(InvalidHandle);
        }
        let count = messages_received(message_count, batch_size);
        unsafe {
            self.message_buffer.set_len(count);
        }
        Ok(count)
    }

    /// Fetch the next available message(s) from the connection, if any.
    /// Returns the number of messages returned into your array, up to nMaxMessages.
    /// If the connection handle is invalid, -1 is returned.
    ///
    /// The order of the messages returned in the array is relevant.
    /// Reliable messages will be received in the order they were sent (and with the
    /// same sizes --- see SendMessageToConnection for on this subtle difference from a stream socket).
    ///
    /// Unreliable messages may be dropped, or delivered out of order with respect to
    /// each other or with respect to reliable messages.  The same unreliable message
    /// may be received multiple times.
    pub fn receive_messages(
        &mut self,
        batch_size: usize,
    ) -> Result<Vec<NetworkingMessage>, InvalidHandle> {
        if self.message_buffer.capacity() < batch_size {
            // reserve(additional) ensures capacity >= len + additional.
            // Since the buffer is always drained between calls, len == 0,
            // so reserve(batch_size) guarantees capacity >= batch_size.
            self.message_buffer.reserve(batch_size);
        }

        self.receive_messages_internal(batch_size)?;

        Ok(self
            .message_buffer
            .drain(..)
            .map(|x| NetworkingMessage {
                message: x,
                _inner: self.inner.clone(),
            })
            .collect())
    }

    /// Like `receive_messages`, but puts the results inside `dest`
    pub fn receive_messages_into(
        &mut self,
        dest: &mut Vec<NetworkingMessage>,
        batch_size: usize,
    ) -> Result<(), InvalidHandle> {
        if self.message_buffer.capacity() < batch_size {
            // reserve(additional) ensures capacity >= len + additional.
            // Since the buffer is always drained between calls, len == 0,
            // so reserve(batch_size) guarantees capacity >= batch_size.
            self.message_buffer.reserve(batch_size);
        }

        self.receive_messages_internal(batch_size)?;

        dest.reserve_exact(batch_size);
        for message in self.message_buffer.drain(..) {
            dest.push(NetworkingMessage {
                message,
                _inner: self.inner.clone(),
            });
        }

        Ok(())
    }

    /// Receives messages like `receive_messages`, but does not allocate a Vec to get the results.
    ///
    /// Instead, whenever a message is received, the closure is called with a `NetworkingMessage` for every message.
    ///
    /// All messages available in the queue will always be received in one call and you don't have to worry about batch size.
    pub fn receive_messages_with(&mut self, mut f: impl FnMut(NetworkingMessage)) {
        const MIN_CAPACITY: usize = 32;
        let mut cap = self.message_buffer.capacity();
        if cap < MIN_CAPACITY {
            self.message_buffer.reserve_exact(MIN_CAPACITY - cap);
            cap = self.message_buffer.capacity();
        }

        loop {
            let Ok(message_count) = self.receive_messages_internal(cap) else {
                return;
            };

            for msg in self.message_buffer.drain(..) {
                f(NetworkingMessage {
                    message: msg,
                    _inner: self.inner.clone(),
                })
            }
            if message_count < cap {
                break;
            }
        }
    }

    /// Assign a connection to a poll group.  Note that a connection may only belong to a
    /// single poll group.  Adding a connection to a poll group implicitly removes it from
    /// any other poll group it is in.
    ///
    /// You can call `clear_connection_poll_group` to remove a connection from its current
    /// poll group without adding it to a new poll group.
    ///
    /// If there are received messages currently pending on the connection, an attempt
    /// is made to add them to the queue of messages for the poll group in approximately
    /// the order that would have applied if the connection was already part of the poll
    /// group at the time that the messages were received.
    ///
    /// Returns false if the connection handle is invalid, or if the poll group handle
    /// is invalid (and not k_HSteamNetPollGroup_Invalid).
    pub fn set_poll_group(&self, poll_group: &NetPollGroup) {
        let was_successful = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_SetConnectionPollGroup(
                self.sockets,
                self.handle,
                poll_group.handle,
            )
        };
        debug_assert!(was_successful);
    }

    /// Tries to receive a pending event. This will never block.
    ///
    /// Some `NetConnection` do not receive events through this method but instead through the ListenSocket,
    /// in that case this will always return `None`
    pub fn try_receive_event(&self) -> Option<NetConnectionEvent> {
        self.event_receiver
            .as_ref()
            .and_then(|receiver| receiver.try_recv().ok())
    }

    /// Returns an iterator for ListenSocketEvents that will block until the next event is received
    ///
    /// Some `NetConnection` do not receive events through this method but instead through the ListenSocket,
    /// in that case this will always return `None`. Otherwise, this will return an iterator that empties
    /// the queue of events.
    pub fn try_events<'a>(&'a self) -> Option<impl Iterator<Item = NetConnectionEvent> + 'a> {
        self.event_receiver
            .as_ref()
            .map(|receiver| receiver.try_iter())
    }

    pub fn run_callbacks(&self) {
        unsafe { sys::SteamAPI_ISteamNetworkingSockets_RunCallbacks(self.sockets) }
    }

    /// Set the connection state to be handled externally. The struct will no longer close the connection on drop.
    pub(crate) fn handle_connection(&mut self) {
        self.is_handled = true
    }
}

impl Drop for NetConnection {
    fn drop(&mut self) {
        if !self.is_handled {
            let debug_string = CString::new("Handle was dropped").unwrap();
            let _was_successful = unsafe {
                sys::SteamAPI_ISteamNetworkingSockets_CloseConnection(
                    self.sockets,
                    self.handle,
                    NetConnectionEnd::App(AppNetConnectionEnd::generic_normal()).into(),
                    debug_string.as_ptr(),
                    false,
                )
            };

            if self.socket.is_none() {
                crate::utils::mutex_guard(&self.inner.networking_sockets_data)
                    .independent_connections
                    .remove(&self.handle)
                    .expect("internal connection was removed before being dropped");
            }
        }
    }
}

/// The debug string handed to Steam: the bytes up to the first nul, which is where a C string ends.
///
/// `CString::new(value).unwrap()` used to sit at the top of `close()`, and `value` is a
/// caller-supplied `Option<&str>`, so pasting a log line with a nul inside crashed the crate from safe
/// code. The rule is `crate::utils::c_string_prefix`, shared with the other fire-and-forget setters;
/// Steam's parameter is "const char *pszDebug" (`isteamnetworkingsockets.h:191`), so the nul already
/// ended the string as far as Steam was concerned and everything after it was never going to be read.
///
/// `None` for `None`, and an empty string keeps its own terminator: the caller asked for an empty
/// debug string, not for no debug string.
fn debug_string_prefix(value: Option<&str>) -> Option<CString> {
    Some(c_string_prefix(value?))
}

/// How much room this crate hands `GetConnectionName` for the connection's debug name.
///
/// Steam sets the name with "void SetConnectionName( HSteamNetConnection hPeer, const char *pszName )"
/// (`isteamnetworkingsockets.h:226`) and reads it back with "char *pszName, int nMaxLen"
/// (`:229`) -- the buffer and its size come from the caller, and the vendored headers state no limit
/// for the name itself. The only comparable ceilings are for other strings:
/// "k_cchSteamNetworkingMaxConnectionAppName = 32" (`steamnetworkingtypes.h:663`) and
/// "k_cbMaxGameServerName = 64" (`matchmakingtypes.h:21`). So this number is ours, set well above
/// both, and a name longer than it comes back truncated with no signal that anything was cut.
const CONNECTION_NAME_BYTES: usize = 256;

/// Steam's answer to a name fetch, turned into a Rust one.
///
/// The header gives one bit -- "Fetch connection name.  Returns false if handle is invalid"
/// (`isteamnetworkingsockets.h:228`). Everything else is read out of the buffer by `fixed_string`,
/// which stops at the first nul and lossy-decodes -- the same rule every other fixed-size Steam text
/// field uses here. A buffer with no nul anywhere is the truncated case: Steam filled all of it, and
/// what it held is all this side gets.
fn name_answer(ok: bool, buffer: &[std::os::raw::c_char]) -> Result<String, InvalidHandle> {
    if !ok {
        return Err(InvalidHandle);
    }
    Ok(crate::utils::fixed_string(buffer))
}

#[cfg(test)]
mod connection_tests {
    //! Pure half of `close()`: which bytes Steam is handed. The FFI call itself needs a live
    //! connection, so only this decision can be judged here; the wiring is stated, not tested.
    use super::debug_string_prefix;
    use super::name_answer;
    use super::InvalidHandle;
    use std::ffi::CString;
    use std::os::raw::c_char;

    fn name_buffer(text: &str, bytes: usize) -> Vec<c_char> {
        let mut buffer = vec![0 as c_char; bytes];
        for (slot, byte) in buffer.iter_mut().zip(text.as_bytes()) {
            *slot = *byte as c_char;
        }
        buffer
    }

    #[test]
    fn a_refused_fetch_is_an_invalid_handle() {
        // `Err(InvalidHandle)` in pattern position binds a *variable* (rustc warns it is unused), so the
        // braces are load-bearing: without them this asserted nothing about which error came back.
        assert!(matches!(
            name_answer(false, &name_buffer("lobby-7", 16)),
            Err(InvalidHandle { .. })
        ));
    }

    #[test]
    fn the_name_ends_where_steam_put_a_terminator() {
        assert_eq!(
            name_answer(true, &name_buffer("lobby-7\0rest", 16)).unwrap(),
            "lobby-7"
        );
        // A buffer Steam left zeroed is a connection with no name set, not a name made of nuls.
        assert_eq!(name_answer(true, &vec![0 as c_char; 8]).unwrap(), "");
    }

    /// The truncated case, and the reason the ceiling is documented: Steam filled the whole buffer and
    /// the name went on past it. Nothing in the answer says so, so the caller gets the prefix.
    #[test]
    fn a_name_that_filled_the_buffer_comes_back_as_what_fit() {
        assert_eq!(
            name_answer(true, &name_buffer("abcdefghijklmnop", 16)).unwrap(),
            "abcdefghijklmnop"
        );
    }

    #[test]
    fn no_debug_string_stays_no_debug_string() {
        assert!(debug_string_prefix(None).is_none());
    }

    #[test]
    fn a_plain_string_is_handed_over_unchanged() {
        assert_eq!(
            debug_string_prefix(Some("peer said bye")).as_deref(),
            Some(CString::new("peer said bye").unwrap().as_c_str())
        );
        assert_eq!(
            debug_string_prefix(Some("")).as_deref(),
            Some(CString::new("").unwrap().as_c_str())
        );
    }

    /// The panic this replaces: `CString::new("a\0b").unwrap()` aborts the caller's frame on a value
    /// the caller chose. Steam reads a `const char *`, so "a" is the whole string to it anyway.
    #[test]
    fn an_interior_nul_truncates_instead_of_panicking() {
        assert_eq!(
            debug_string_prefix(Some("a\0b")).as_deref(),
            Some(CString::new("a").unwrap().as_c_str())
        );
        assert_eq!(
            debug_string_prefix(Some("\0first")).as_deref(),
            Some(CString::new("").unwrap().as_c_str())
        );
        assert_eq!(
            debug_string_prefix(Some("clean\0")).as_deref(),
            Some(CString::new("clean").unwrap().as_c_str())
        );
    }
}
