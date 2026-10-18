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

/// Turns a `ReceiveMessages*` return value into a length `set_len` may trust.
///
/// The count arrives as a signed `c_int`, and Steam's contract for the connection version is
/// "Returns the number of messages returned into your array, up to nMaxMessages.  If the connection
/// handle is invalid, -1 is returned" (`isteamnetworkingsockets.h:320-321`);
/// `ReceiveMessagesOnPollGroup` claims "the same guarantees are present as mentioned in
/// ReceiveMessagesOnConnection" (`:552-553`). Cast through, `-1 as usize` is ~1.8e19: `set_len` would
/// then describe a longer `Vec` than any allocation, which is UB the moment the messages are drained.
/// An over-large count is clamped to the capacity too, since that is all the array could hold.
///
/// This is not a defensive-only path: `create_poll_group` returns whatever Steam hands back without
/// rejecting `k_HSteamNetPollGroup_Invalid`, so a failed create reaches this call with an invalid
/// handle -- exactly the case the header says answers -1.
fn message_count(returned: std::os::raw::c_int, capacity: usize) -> usize {
    usize::try_from(returned).unwrap_or(0).min(capacity)
}

impl NetPollGroup {
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
        let count = message_count(returned, self.message_buffer.capacity());
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

#[cfg(test)]
mod tests {
    //! Pure half of the receive path: the count clamp. The FFI call itself needs a running client,
    //! but every decision made from its return value does not, so the decisions are split out and
    //! judged here rather than registered as uncovered.
    use super::message_count;

    #[test]
    fn a_valid_count_becomes_that_length() {
        assert_eq!(message_count(3, 8), 3);
        assert_eq!(message_count(0, 8), 0);
        assert_eq!(message_count(8, 8), 8);
    }

    /// Steam's contract says "If the connection handle is invalid, -1 is returned"
    /// (`isteamnetworkingsockets.h:321`). Cast through,
    /// it used to become ~1.8e19 and `set_len` would then describe a `Vec` longer than any allocation.
    #[test]
    fn the_invalid_handle_minus_one_reads_as_no_messages() {
        assert_eq!(message_count(-1, 8), 0);
        assert_eq!(message_count(i32::MIN, 8), 0);
    }

    /// A count above the capacity is truncated to it: Steam may not write more pointers than the array
    /// holds, and if the number says otherwise the array is the authority.
    #[test]
    fn an_over_large_count_is_clamped_to_the_capacity() {
        assert_eq!(message_count(99, 8), 8);
        assert_eq!(message_count(i32::MAX, 0), 0);
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
