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
    ///
    /// Bits this crate has no name for are dropped rather than fatal. `m_nFlags` arrives in a struct
    /// Steam fills, so the value is not the caller's: reading it with `from_bits(..).expect(..)` made
    /// a safe getter abort inside the *caller's* frame on a bit Steam or a peer chose. Steam documents
    /// "For received messages, only the k_nSteamNetworkingSend_Reliable bit is valid."
    /// (`steamnetworkingtypes.h:908-909`), which is a statement about today's build, not a promise that
    /// no new bit can ever appear -- and `AutoRestartBrokenSession` is exactly how a new bit shows up.
    pub fn send_flags(&self) -> SendFlags {
        unsafe { SendFlags::from_bits_truncate((*self.message).m_nFlags) }
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

    /// Message payload.
    ///
    /// A message with no buffer reads as empty instead of as a slice over the pointer it carries.
    /// Two reasons, both about values that are not the caller's:
    ///
    /// - `ISteamNetworkingUtils::AllocateMessage` states "If cbAllocateBuffer=0, then no buffer is
    ///   allocated.  m_pData will be NULL, m_cbSize will be zero" (`isteamnetworkingutils.h:38-40`), and
    ///   `slice::from_raw_parts` requires a non-null, aligned pointer *even at length zero* -- so the
    ///   old unconditional call was undefined behaviour on a state the header documents as normal,
    ///   reached from a safe method (`networking_sockets/mod.rs` and `addresses.rs` allocate at size 0).
    /// - `m_cbSize` is a signed `c_int`. Cast to `usize`, a negative value becomes ~2^64, which is both
    ///   an impossible slice length and the thing that defeats the capacity check below.
    pub fn data(&self) -> &[u8] {
        let (data, size) = unsafe {
            (
                (*self.message).m_pData as *const u8,
                (*self.message).m_cbSize,
            )
        };
        if data.is_null() || size <= 0 {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(data, size as usize) }
    }

    /// Copies `data` into the message's own buffer.
    ///
    /// The capacity read out of the struct is treated as unsigned-or-absent: a negative `m_cbSize`
    /// refuses the write rather than permitting it. Casting it to `usize` inverted the comparison --
    /// `-1 as usize` exceeds every possible length, so the guard passed and the copy wrote past a
    /// buffer that was never that large.
    pub fn copy_data_into_buffer(&mut self, data: &[u8]) -> Result<(), MessageError> {
        unsafe {
            if (*self.message).m_pData.is_null() {
                return Err(MessageError::NullBuffer);
            }

            let capacity = usize::try_from((*self.message).m_cbSize)
                .map_err(|_| MessageError::BufferTooSmall)?;
            if capacity < data.len() {
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

#[cfg(test)]
mod tests {
    //! No Steam client here: each case builds the C struct in Rust memory, in a state the headers
    //! document, and reads it through the wrapper. The message is `forget`-ed because `Drop` would
    //! otherwise hand a synthetic pointer to `SteamNetworkingMessage_t_Release`.
    use super::*;
    use crate::{Callbacks, Inner, Manager, NetworkingSocketsData};
    use std::ffi::c_void;
    use std::sync::{Arc, Mutex, Weak};

    /// A leaked `SteamNetworkingMessage_t` (so the pointer stays valid for the test's whole life)
    /// wrapped as a `NetworkingMessage`, with the three fields the wrappers read.
    fn synthetic(size: i32, payload: *mut c_void, flags: i32) -> NetworkingMessage {
        let raw: &mut sys::SteamNetworkingMessage_t =
            Box::leak(Box::new(unsafe { std::mem::zeroed() }));
        raw.m_cbSize = size;
        raw.m_pData = payload;
        raw.m_nFlags = flags;
        let inner = Arc::new(Inner {
            manager: Manager::Client,
            callbacks: Callbacks::new(),
            networking_sockets_data: Mutex::new(NetworkingSocketsData {
                sockets: Default::default(),
                independent_connections: Default::default(),
                fake_udp_ports: Default::default(),
                connection_callback: Weak::new(),
            }),
        });
        NetworkingMessage {
            message: raw as *mut _,
            _inner: inner,
        }
    }

    /// `isteamnetworkingutils.h:38-40`: "If cbAllocateBuffer=0, then no buffer is allocated.
    /// m_pData will be NULL, m_cbSize will be zero". `data()` used to build a slice from that
    /// unconditionally, which rustc's own precondition check aborts on.
    #[test]
    fn a_message_with_no_buffer_reads_as_empty() {
        let msg = synthetic(0, std::ptr::null_mut(), 0);
        assert_eq!(msg.data(), &[] as &[u8]);
        std::mem::forget(msg);
    }

    /// A size Steam reports as negative must not become a 2^64-byte slice.
    #[test]
    fn a_negative_size_reads_as_empty_rather_than_as_everything() {
        let mut owned = vec![1u8, 2, 3].into_boxed_slice();
        let msg = synthetic(-1, owned.as_mut_ptr() as *mut c_void, 0);
        assert_eq!(msg.data(), &[] as &[u8]);
        std::mem::forget(msg);
        assert_eq!(&*owned, &[1, 2, 3]);
    }

    /// The capacity guard compared `m_cbSize as usize` against the write length, so -1 read as
    /// ~1.8e19 and *anything* fit: an 8-byte write into a 4-byte buffer returned `Ok` and wrote past
    /// the allocation.
    #[test]
    fn a_negative_capacity_refuses_the_write() {
        let mut buffer = vec![7u8; 4].into_boxed_slice();
        let mut msg = synthetic(-1, buffer.as_mut_ptr() as *mut c_void, 0);
        let outcome = msg.copy_data_into_buffer(&[0u8; 8]);
        assert!(
            matches!(outcome, Err(MessageError::BufferTooSmall)),
            "a corrupt capacity must refuse, got {outcome:?}"
        );
        assert_eq!(
            &*buffer,
            &[7, 7, 7, 7],
            "nothing may be written on the refused path"
        );
        std::mem::forget(msg);
    }

    /// A correctly-sized write still goes through, so the guard above is a sign check and not a ban.
    #[test]
    fn a_capacity_that_fits_still_writes() {
        let mut buffer = vec![0u8; 8].into_boxed_slice();
        let mut msg = synthetic(8, buffer.as_mut_ptr() as *mut c_void, 0);
        assert!(msg.copy_data_into_buffer(&[9, 9, 9, 9]).is_ok());
        assert_eq!(&buffer[..4], &[9, 9, 9, 9]);
        assert!(matches!(
            msg.copy_data_into_buffer(&[0u8; 9]),
            Err(MessageError::BufferTooSmall)
        ));
        std::mem::forget(msg);
    }

    /// `m_nFlags` arrives from Steam, not from the caller: `from_bits(..).expect(..)` aborted in the
    /// caller's frame on an unnamed bit. The named bits must survive the truncation, or this would be
    /// a silent data-loss fix rather than a hardening one.
    #[test]
    fn an_unnamed_send_bit_is_dropped_while_named_bits_survive() {
        let unnamed = 1 << 20;
        let mut msg = synthetic(
            0,
            std::ptr::null_mut(),
            unnamed | sys::k_nSteamNetworkingSend_Reliable,
        );
        assert_eq!(msg.send_flags(), SendFlags::RELIABLE);
        msg.set_send_flags(SendFlags::RELIABLE | SendFlags::NO_NAGLE);
        assert_eq!(
            unsafe { (*msg.message).m_nFlags },
            sys::k_nSteamNetworkingSend_Reliable | sys::k_nSteamNetworkingSend_NoNagle
        );
        std::mem::forget(msg);
    }
}
