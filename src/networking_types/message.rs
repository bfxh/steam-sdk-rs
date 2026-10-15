//! `NetworkingMessage` -- one datagram in flight: the buffer it owns, how the payload is
//! read and written, and handing it back to Steam on drop.

use super::*;

pub struct NetworkingMessage {
    pub(crate) message: *mut sys::SteamNetworkingMessage_t,

    // Not sure if this is necessary here, we may not need a Manager to use free on messages
    pub(crate) _inner: Arc<Inner>,
}

impl NetworkingMessage {
    /// For messages received on connections: what connection did this come from?
    /// For outgoing messages: what connection to send it to?
    /// Not used when using the ISteamNetworkingMessages interface
    #[allow(dead_code)]
    pub(crate) fn connection(&self) -> Option<sys::HSteamNetConnection> {
        let handle = unsafe { (*self.message).m_conn };
        if handle == sys::k_HSteamNetConnection_Invalid {
            None
        } else {
            Some(handle)
        }
    }

    /// Set the target connection for the connection.
    /// Make sure you don't close or drop the `NetConnection` before sending your message.
    ///
    /// Use this with `ListenSocket::send_messages` for efficient sending.
    pub fn set_connection(&mut self, connection: &NetConnection) {
        unsafe { (*self.message).m_conn = connection.handle }
    }

    /// For inbound messages: Who sent this to us?
    /// For outbound messages on connections: not used.
    /// For outbound messages on the ad-hoc ISteamNetworkingMessages interface: who should we send this to?
    pub fn identity_peer(&self) -> NetworkingIdentity {
        unsafe {
            let ident = &mut (*self.message).m_identityPeer;
            NetworkingIdentity { inner: *ident }
        }
    }

    /// The identity of the sender or, the receiver when used with the NetworkingMessages interface.
    pub fn set_identity_peer(&mut self, identity: NetworkingIdentity) {
        unsafe { (*self.message).m_identityPeer = identity.inner }
    }

    /// For messages received on connections, this is the user data
    /// associated with the connection.
    ///
    /// This is *usually* the same as calling GetConnection() and then
    /// fetching the user data associated with that connection, but for
    /// the following subtle differences:
    ///
    /// - This user data will match the connection's user data at the time
    ///   is captured at the time the message is returned by the API.
    ///   If you subsequently change the userdata on the connection,
    ///   this won't be updated.
    /// - This is an inline call, so it's *much* faster.
    /// - You might have closed the connection, so fetching the user data
    ///   would not be possible.
    ///
    /// Not used when sending messages,
    pub fn connection_user_data(&self) -> i64 {
        unsafe { (*self.message).m_nConnUserData }
    }

    /// Message number assigned by the sender.
    /// This is not used for outbound messages
    pub fn message_number(&self) -> MessageNumber {
        unsafe { MessageNumber((*self.message).m_nMessageNumber as u64) }
    }

    /// Bitmask of k_nSteamNetworkingSend_xxx flags.
    /// For received messages, only the k_nSteamNetworkingSend_Reliable bit is valid.
    /// For outbound messages, all bits are relevant
    pub fn send_flags(&self) -> SendFlags {
        unsafe {
            SendFlags::from_bits((*self.message).m_nFlags)
                .expect("send flags could not be converted to rust representation")
        }
    }

    pub fn set_send_flags(&mut self, send_flags: SendFlags) {
        unsafe { (*self.message).m_nFlags = send_flags.bits() }
    }

    /// Bitmask of k_nSteamNetworkingSend_xxx flags.
    /// For received messages, only the k_nSteamNetworkingSend_Reliable bit is valid.
    /// For outbound messages, all bits are relevant
    pub fn channel(&self) -> i32 {
        unsafe { (*self.message).m_nChannel }
    }

    pub fn set_channel(&mut self, channel: i32) {
        unsafe {
            (*self.message).m_nChannel = channel;
        }
    }

    /// Message payload
    pub fn data(&self) -> &[u8] {
        unsafe {
            std::slice::from_raw_parts(
                (*self.message).m_pData as _,
                (*self.message).m_cbSize as usize,
            )
        }
    }

    pub fn copy_data_into_buffer(&mut self, data: &[u8]) -> Result<(), MessageError> {
        unsafe {
            if (*self.message).m_pData.is_null() {
                return Err(MessageError::NullBuffer);
            }

            if ((*self.message).m_cbSize as usize) < data.len() {
                return Err(MessageError::BufferTooSmall);
            }

            ((*self.message).m_pData as *mut u8).copy_from(data.as_ptr(), data.len());
        }

        Ok(())
    }

    /// Set a new buffer for the message.
    ///
    /// Returns `Err(MessageError::BufferAlreadySet)` if the current buffer is not NULL.
    pub fn set_data(&mut self, data: Vec<u8>) -> Result<(), MessageError> {
        unsafe {
            if !(*self.message).m_pData.is_null() {
                return Err(MessageError::BufferAlreadySet);
            }

            let mut data = data.into_boxed_slice();
            (*self.message).m_pData = data.as_mut_ptr() as *mut c_void;
            (*self.message).m_cbSize = data.len() as _;
            (*self.message).m_pfnFreeData = Some(free_rust_message_buffer);
            std::mem::forget(data);
        }

        Ok(())
    }

    /// Arbitrary user data that you can use when sending messages using
    /// ISteamNetworkingUtils::AllocateMessage and ISteamNetworkingSockets::SendMessage.
    /// (The callback you set in m_pfnFreeData might use this field.)
    ///
    /// Not used for received messages.
    pub fn user_data(&self) -> i64 {
        unsafe { (*self.message).m_nUserData }
    }

    /// Arbitrary user data that you can use when sending messages using
    /// ISteamNetworkingUtils::AllocateMessage and ISteamNetworkingSockets::SendMessage.
    /// (The callback you set in m_pfnFreeData might use this field.)
    ///
    /// Not used for received messages.
    pub fn set_user_data(&mut self, user_data: i64) {
        unsafe {
            (*self.message).m_nUserData = user_data;
        }
    }

    /// Return the message pointer and prevent rust from releasing it
    pub(crate) fn take_message(mut self) -> *mut sys::SteamNetworkingMessage_t {
        let message = self.message;
        self.message = std::ptr::null_mut();
        message
    }
}

extern "C" fn free_rust_message_buffer(message: *mut sys::SteamNetworkingMessage_t) {
    // Panic in code called by C is undefined behaviour
    if let Err(e) = catch_unwind(|| unsafe {
        let data = (*message).m_pData as *mut u8;
        let len = (*message).m_cbSize as usize;
        // `set_data` forgot a `Box<[u8]>`, so the pointer has to come back as the
        // same fat pointer: `Box::from_raw` of the bare `*mut c_void` would free a
        // `m_cbSize`-byte allocation with a one-byte layout.
        let _ = Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, len));
    }) {
        eprintln!("{:?}", e);
    }
}

impl Drop for NetworkingMessage {
    fn drop(&mut self) {
        if !self.message.is_null() {
            unsafe { sys::SteamAPI_SteamNetworkingMessage_t_Release(self.message) }
        }
    }
}

#[derive(Debug, Error)]
pub enum MessageError {
    #[error("failed to write data to message, the buffer is not set")]
    NullBuffer,
    #[error("copied data is too large for the current buffer")]
    BufferTooSmall,
    #[error("cannot set a new buffer, the message already has a valid buffer")]
    BufferAlreadySet,
}
