//! A live connection handle: the send, query and close operations that need a
//! `HSteamNetConnection`, plus the RAII drop that releases it back to Steam.
//!
//! Split out of the socket module because this is the type a caller talks to per peer, and its
//! method list grows independently of the listener and poll-group plumbing next door.

use super::*;

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
        inner
            .networking_sockets_data
            .lock()
            .unwrap()
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
        let debug_string = debug_string.map(|x| CString::new(x).unwrap());
        let debug_string_ptr = match debug_string {
            None => std::ptr::null(),
            Some(s) => s.as_ptr(),
        };
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
        let name = CString::new(name).unwrap();
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

    /// Fetch connection name.  Returns false if handle is invalid
    pub fn connection_name(&self) -> Result<(), InvalidHandle> {
        unimplemented!()
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
        unsafe {
            self.message_buffer.set_len(message_count as usize);
        }
        Ok(message_count as usize)
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
                self.inner
                    .networking_sockets_data
                    .lock()
                    .unwrap()
                    .independent_connections
                    .remove(&self.handle)
                    .expect("internal connection was removed before being dropped");
            }
        }
    }
}
