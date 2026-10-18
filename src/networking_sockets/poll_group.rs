//! Groups you poll instead of threading: the raw socket holder, the group handle with its
//! `Drop`, the batched receive, and the one `NetworkingSockets` method that mints a group.

use super::*;

/// Inner struct that keeps sockets alive as long as there is still a connection alive
pub(crate) struct InnerSocket {
    pub(crate) sockets: *mut sys::ISteamNetworkingSockets,
    pub(crate) handle: sys::HSteamListenSocket,
    pub(crate) inner: Arc<Inner>,
}

unsafe impl Send for InnerSocket {}

unsafe impl Sync for InnerSocket {}

impl Drop for InnerSocket {
    fn drop(&mut self) {
        // There's no documentation for this return value, so it's most likely false when hSocket is invalid
        // The handle should always be valid in our case.
        let _was_successful = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_CloseListenSocket(self.sockets, self.handle)
        };

        if let None = self
            .inner
            .networking_sockets_data
            .lock()
            .unwrap()
            .sockets
            .remove(&self.handle)
        {
            eprintln!("error while dropping InnerSocket: socket was already removed")
        }
    }
}

pub struct NetPollGroup {
    // `pub(super)`: `connection.rs` attaches a connection to the group by reading this handle, and the
    // split left the two in sibling files.
    pub(super) handle: sys::HSteamNetPollGroup,
    sockets: *mut sys::ISteamNetworkingSockets,
    inner: Arc<Inner>,
    message_buffer: Vec<*mut SteamNetworkingMessage_t>,
}

unsafe impl Send for NetPollGroup {}

unsafe impl Sync for NetPollGroup {}

impl NetPollGroup {
    /// Drains up to `batch_size` messages from the group.
    ///
    /// The count clamp is `super::messages_received`, shared with the Fake UDP port's receive so that
    /// one contract has one implementation. What is specific to this caller is that `batch_size` is
    /// both the number handed to Steam as `nMaxMessages` and the bound the clamp uses -- the buffer
    /// is reserved to at least that, so the array can always hold what is believed -- and that
    /// `create_poll_group` hands back an unvalidated handle, so the invalid-handle answer of -1 is
    /// reachable here rather than merely theoretical.
    pub fn receive_messages(&mut self, batch_size: usize) -> Vec<NetworkingMessage> {
        if self.message_buffer.capacity() < batch_size {
            // reserve(additional) ensures capacity >= len + additional.
            // Since the buffer is always drained between calls, len == 0,
            // so reserve(batch_size) guarantees capacity >= batch_size.
            self.message_buffer.reserve(batch_size);
        }

        let returned = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_ReceiveMessagesOnPollGroup(
                self.sockets,
                self.handle,
                self.message_buffer.as_mut_ptr(),
                batch_size as _,
            )
        };
        let count = messages_received(returned, batch_size);
        unsafe { self.message_buffer.set_len(count) };

        self.message_buffer
            .drain(..)
            .map(|x| NetworkingMessage {
                message: x,
                _inner: self.inner.clone(),
            })
            .collect()
    }
}

impl Drop for NetPollGroup {
    fn drop(&mut self) {
        let _was_successful = unsafe {
            sys::SteamAPI_ISteamNetworkingSockets_DestroyPollGroup(self.sockets, self.handle)
        };
    }
}

impl NetworkingSockets {
    /// Create a new poll group.
    ///
    /// You should destroy the poll group when you are done using DestroyPollGroup
    pub fn create_poll_group(&self) -> NetPollGroup {
        let poll_group =
            unsafe { sys::SteamAPI_ISteamNetworkingSockets_CreatePollGroup(self.sockets) };
        NetPollGroup {
            handle: poll_group,
            sockets: self.sockets,
            inner: self.inner.clone(),
            message_buffer: Vec::new(),
        }
    }
}
