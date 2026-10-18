//! A socket that is listening: the handle, the events it yields (polled, blocking, or as an
//! iterator), and the send that accepts connections coming in on it.

use super::*;

/// A socket that will continually listen for client connections.
/// Call `events()` to receive incoming connection.
/// You should regularly check for events and answer `ConnectionRequests` requests immediately or the socket will
/// appear as unresponsive to the client.
///
/// If a Listen Socket goes out of scope while there are still connections, but new requests will be rejected immediately.
///
/// Listen Socket Events will only be available if steam callback are regularly called.
pub struct ListenSocket {
    inner: Arc<InnerSocket>,
    _callback_handle: Arc<CallbackHandle>,
    receiver: Receiver<ListenSocketEvent>,
}

impl ListenSocket {
    pub(crate) fn new(
        handle: sys::HSteamListenSocket,
        sockets: *mut sys::ISteamNetworkingSockets,
        inner: Arc<Inner>,
    ) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        let inner_socket = Arc::new(InnerSocket {
            sockets,
            handle,
            inner: inner.clone(),
        });
        inner
            .networking_sockets_data
            .lock()
            .unwrap()
            .sockets
            .insert(handle, (Arc::downgrade(&inner_socket), sender));
        let callback_handle =
            networking_sockets_callback::get_or_create_connection_callback(inner.clone(), sockets);
        ListenSocket {
            inner: inner_socket,
            _callback_handle: callback_handle,
            receiver,
        }
    }

    /// Tries to receive a pending event. This will never block.
    ///
    /// You should answer ConnectionRequests immediately or the server will appear as unresponsive.
    pub fn try_receive_event(&self) -> Option<ListenSocketEvent> {
        self.receiver.try_recv().ok()
    }

    /// Receive the next event. This will block until the next event is received.
    ///
    /// You should answer ConnectionRequests immediately or the server will appear as unresponsive.
    pub fn receive_event(&self) -> ListenSocketEvent {
        self.receiver
            .recv()
            .expect("all senders were closed, even though the listen socket is still in use")
    }

    /// Returns an iterator for ListenSocketEvents that will block until the next event is received
    ///
    /// You should answer ConnectionRequests immediately or the server will appear as unresponsive.
    pub fn events<'a>(&'a self) -> impl Iterator<Item = ListenSocketEvent> + 'a {
        self.receiver.iter()
    }

    /// Send one or more messages without copying the message payload.
    /// This is the most efficient way to send messages. To use this
    /// function, you must first allocate a message object using
    /// ISteamNetworkingUtils::AllocateMessage.  (Do not declare one
    /// on the stack or allocate your own.)
    ///
    /// You should fill in the message payload.  You can either let
    /// it allocate the buffer for you and then fill in the payload,
    /// or if you already have a buffer allocated, you can just point
    /// m_pData at your buffer and set the callback to the appropriate function
    /// to free it.  Note that if you use your own buffer, it MUST remain valid
    /// until the callback is executed.  And also note that your callback can be
    /// invoked at ant time from any thread (perhaps even before SendMessages
    /// returns!), so it MUST be fast and threadsafe.
    ///
    /// You MUST also fill in:
    /// - m_conn - the handle of the connection to send the message to
    /// - m_nFlags - bitmask of k_nSteamNetworkingSend_xxx flags.
    ///
    /// All other fields are currently reserved and should not be modified.
    ///
    /// The library will take ownership of the message structures.  They may
    /// be modified or become invalid at any time, so you must not read them
    /// after passing them to this function.
    ///
    /// Returns the message number or Steam error for each sent message.
    pub fn send_messages(
        &self,
        messages: impl IntoIterator<Item = NetworkingMessage>,
    ) -> Vec<SteamResult<MessageNumber>> {
        let messages: Vec<_> = messages.into_iter().map(|x| x.take_message()).collect();
        let mut results = vec![0; messages.len()];
        unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_SendMessages(
                self.inner.sockets,
                messages.len() as _,
                messages.as_ptr(),
                results.as_mut_ptr(),
            );
            // Error codes are returned as negative numbers, while positive numbers are message numbers
            results.into_iter().map(super::send_outcome).collect()
        }
    }
}
