//! The one rule for believing a byte count Steam reported writing into memory we handed over.
//!
//! It exists because two interfaces answer the same way -- `ISteamUtils::GetVoice` (see
//! `src/user/voice.rs`) and `ISteamRemoteStorage::FileReadAsyncComplete` (see
//! `src/remote_storage/files.rs`) -- and in both, taking the number at its word means reading, or
//! handing the caller, bytes nobody wrote.

/// How many bytes Steam reported writing, believed only as far as the buffer allows.
///
/// The buffer's own length is the only bound worth applying here, and it is the tight one: what
/// Steam was told can never exceed it, so a count above `capacity` would mean Steam ignored a size
/// argument we already know it was handed. Trusting the number further would hand the caller bytes
/// nobody wrote -- and in `FileReadAsyncComplete` the same number is the size of the copy Steam is
/// about to make into the caller's buffer, so believing it is also how the write gets out of bounds.
pub(crate) fn written_bytes(written: u32, capacity: usize) -> usize {
    (written as usize).min(capacity)
}
