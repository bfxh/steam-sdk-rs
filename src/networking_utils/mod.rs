//! `NetworkingUtils` -- the `ISteamNetworkingUtils` interface, split by what it answers about:
//! relay reachability in `relay.rs`, ping estimates in `ping.rs`, and the FakeIP questions in
//! `fake_ip.rs`. The struct itself and the message allocator that belongs to no question stay here.

use crate::networking_types::NetworkingMessage;
use crate::Inner;
use std::sync::Arc;

use steamworks_sys as sys;

mod fake_ip;
mod ping;
mod relay;

pub use fake_ip::*;
pub use ping::*;
pub use relay::*;

/// Access to the steam networking sockets interface
pub struct NetworkingUtils {
    pub(crate) utils: *mut sys::ISteamNetworkingUtils,
    pub(crate) inner: Arc<Inner>,
}

// SAFETY: nothing in this crate's tree needs `Send` for this type today (measured: this
// line deleted, `cargo check --all-targets` stays green). The promise is kept because
// the type is public and a caller may move it to the thread that pumps callbacks.
// The type holds a raw Steam interface pointer, plain-copy handles, and the owning `Arc<Inner>`;
// it carries no non-atomic reference count, which is the one hazard a `Send` claim here cannot
// see -- `scripts/ci/callback_threads.py` counts `Rc` tokens beside every promise and ships at
// zero for exactly that reason. Moving the value therefore moves sole ownership of the pointer;
// nothing reads the same state through two threads because of the move itself.
unsafe impl Send for NetworkingUtils {}
unsafe impl Sync for NetworkingUtils {}

impl NetworkingUtils {
    /// Allocate and initialize a message object.  Usually the reason
    /// you call this is to pass it to ISteamNetworkingSockets::SendMessages.
    /// The returned object will have all of the relevant fields cleared to zero.
    ///
    /// Optionally you can also request that this system allocate space to
    /// hold the payload itself.  If cbAllocateBuffer is nonzero, the system
    /// will allocate memory to hold a payload of at least cbAllocateBuffer bytes.
    /// m_pData will point to the allocated buffer, m_cbSize will be set to the
    /// size, and m_pfnFreeData will be set to the proper function to free up
    /// the buffer.
    ///
    /// If cbAllocateBuffer=0, then no buffer is allocated.  m_pData will be NULL,
    /// m_cbSize will be zero, and m_pfnFreeData will be NULL.  You will need to
    /// set each of these.
    pub fn allocate_message(&self, buffer_size: usize) -> NetworkingMessage {
        unsafe {
            let message =
                sys::SteamAPI_ISteamNetworkingUtils_AllocateMessage(self.utils, buffer_size as _);
            NetworkingMessage {
                message,
                _inner: self.inner.clone(),
            }
        }
    }
}
