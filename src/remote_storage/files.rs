//! Files as handles: `SteamFile` metadata and access, the `std::io` writer and reader pairs
//! (with their `Seek`), the batch guard that ends a Steam write batch on drop, and the info
//! record Steam fills in.

use super::*;
use crate::utils::written_bytes;
use std::cmp::min;
use std::os::raw::c_int;

/// How many bytes one chunk call may name.
///
/// Steam's own heading over these is "file operations that cause network IO"
/// (`isteamremotestorage.h:214`), and the chunk call takes `int32 cubData` (`:216`), while
/// `std::io::Write` is allowed to write fewer bytes than it was asked for. So the honest answer to a
/// buffer wider than Steam's `int32` is a short write that the caller repeats -- not the silent
/// `as i32` that used to turn such a length into a negative one at Steam's end.
fn chunk_fit(len: usize) -> usize {
    min(len, c_int::MAX as usize)
}

/// The size Steam reports for a cloud file, under its heading "file information"
/// (`isteamremotestorage.h:220`), from an `int32` return (`:223`).
///
/// Measured: the vendored SDK declares no sentinel for that value -- no `INVALID_FILE_SIZE` anywhere in
/// `steamworks-sys/lib/steam/public/steam` -- so a negative is not a number this crate may read as a
/// size, and `as usize` on an `int32` sign-extends: a missing file used to hand a reader a size near
/// 1.8e18, one it would never walk to the end. `FileExists` (`:221`) is the predicate that answers
/// whether the file is there at all; zero here records only that no size came back.
fn reader_size(raw: c_int) -> usize {
    usize::try_from(raw).unwrap_or(0)
}

/// Whether dropping a writer should still close the Steam stream.
///
/// Both sit under Steam's heading "file operations that cause network IO"
/// (`isteamremotestorage.h:214`) and are two separate calls -- `FileWriteStreamClose` at `:217`,
/// `FileWriteStreamCancel` at `:218`. Doing both to one handle asks Steam about a stream it has
/// already been told to throw away; doing neither leaks it.
fn closes_on_drop(cancelled: bool) -> bool {
    !cancelled
}

/// Ends the Steam write batch on drop.
pub struct FileWriteBatch {
    // `pub(super)`: `begin_write_batch` in `api.rs` is the only thing that ever builds the guard.
    pub(super) rs: *mut sys::ISteamRemoteStorage,
    // Declared last so it is released last: this wrapper's own `Drop` calls Steam, and the
    // client's `SteamAPI_Shutdown` runs when the last `Arc<Inner>` goes (`src/lib.rs`).
    pub(super) _inner: Arc<Inner>,
}

impl Drop for FileWriteBatch {
    fn drop(&mut self) {
        unsafe {
            sys::SteamAPI_ISteamRemoteStorage_EndFileWriteBatch(self.rs);
        }
    }
}

/// A handle for a possible steam cloud file
pub struct SteamFile {
    pub(crate) rs: *mut sys::ISteamRemoteStorage,
    pub(crate) util: *mut sys::ISteamUtils,
    pub(crate) _inner: Arc<Inner>,
    // `pub(super)`: `api.rs` constructs the handle from the validated name the caller passed, so the
    // split left the field and its only constructor in sibling files.
    pub(super) name: CString,
}

impl SteamFile {
    /// Deletes the file locally and remotely.
    ///
    /// Returns whether a file was actually deleted
    pub fn delete(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_FileDelete(self.rs, self.name.as_ptr()) }
    }
    /// Deletes the file remotely whilst keeping it locally.
    ///
    /// Returns whether a file was actually forgotten
    pub fn forget(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_FileForget(self.rs, self.name.as_ptr()) }
    }

    /// Returns whether a file exists
    pub fn exists(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_FileExists(self.rs, self.name.as_ptr()) }
    }

    /// Returns whether a file is persisted in the steam cloud
    pub fn is_persisted(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_FilePersisted(self.rs, self.name.as_ptr()) }
    }

    /// Returns the timestamp of the file
    pub fn timestamp(&self) -> i64 {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_GetFileTimestamp(self.rs, self.name.as_ptr()) }
    }

    /// Set which platforms the file should be available on
    pub fn set_sync_platforms(&self, platforms: RemoteStoragePlatforms) {
        unsafe {
            sys::SteamAPI_ISteamRemoteStorage_SetSyncPlatforms(
                self.rs,
                self.name.as_ptr(),
                platforms.into(),
            );
        }
    }

    /// Returns the platforms the file is available on
    pub fn get_sync_platforms(&self) -> RemoteStoragePlatforms {
        let bits = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_GetSyncPlatforms(self.rs, self.name.as_ptr())
        };
        RemoteStoragePlatforms::from_bits_truncate(bits.0 as _)
    }

    pub fn write(self) -> SteamFileWriter {
        unsafe {
            let handle =
                sys::SteamAPI_ISteamRemoteStorage_FileWriteStreamOpen(self.rs, self.name.as_ptr());
            SteamFileWriter {
                file: self,
                handle,
                cancelled: false,
            }
        }
    }

    pub fn read(self) -> SteamFileReader {
        unsafe {
            SteamFileReader {
                offset: 0,
                size: reader_size(sys::SteamAPI_ISteamRemoteStorage_GetFileSize(
                    self.rs,
                    self.name.as_ptr(),
                )),
                file: self,
            }
        }
    }

    pub fn share(&self, cb: impl FnOnce(Result<u64, SteamError>) + 'static + Send) {
        let api_call =
            unsafe { sys::SteamAPI_ISteamRemoteStorage_FileShare(self.rs, self.name.as_ptr()) };
        unsafe {
            register_call_result::<sys::RemoteStorageFileShareResult_t, _>(
                &self._inner,
                api_call,
                move |v, io_error| {
                    if io_error {
                        cb(Err(SteamError::IOFailure));
                        return;
                    }

                    cb(to_steam_result(v.m_eResult).map(|()| v.m_hFile))
                },
            )
        }
    }
}

/// A write handle for a steam cloud file
pub struct SteamFileWriter {
    file: SteamFile,
    handle: sys::UGCFileWriteStreamHandle_t,
    // Set only when Steam itself says the cancel went through; see [`closes_on_drop`].
    cancelled: bool,
}

impl std::io::Write for SteamFileWriter {
    /// Writes up to `c_int::MAX` bytes of `buf` and says how many, so a caller that handed over a
    /// buffer Steam cannot size in one chunk simply writes it again; `chunk_fit` is the rule.
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let wanted = chunk_fit(buf.len());
        let named = i32::try_from(wanted).unwrap_or(c_int::MAX);
        unsafe {
            if sys::SteamAPI_ISteamRemoteStorage_FileWriteStreamWriteChunk(
                self.file.rs,
                self.handle,
                buf.as_ptr().cast(),
                named,
            ) {
                Ok(wanted)
            } else {
                Err(std::io::ErrorKind::Other.into())
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl SteamFileWriter {
    /// Throws the stream away without committing what was buffered.
    ///
    /// Steam keeps this and closing apart, both under "file operations that cause network IO"
    /// (`isteamremotestorage.h:214`): `FileWriteStreamClose` at `:217`, then
    /// `FileWriteStreamCancel` at `:218`. What differs is what reaches the cloud: a close
    /// commits the bytes, a cancel discards them. `true` is Steam having accepted the discard, and
    /// only then does this wrapper stop the drop from also closing; a `false` means Steam still holds
    /// a live stream, and leaving it unclosed is the leak.
    pub fn cancel(&mut self) -> bool {
        let ok = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_FileWriteStreamCancel(self.file.rs, self.handle)
        };
        self.cancelled = ok;
        ok
    }
}

impl Drop for SteamFileWriter {
    fn drop(&mut self) {
        if !closes_on_drop(self.cancelled) {
            return;
        }
        unsafe {
            sys::SteamAPI_ISteamRemoteStorage_FileWriteStreamClose(self.file.rs, self.handle);
        }
    }
}

/// A read handle for a steam cloud file
pub struct SteamFileReader {
    file: SteamFile,
    offset: usize,
    size: usize,
}

impl std::io::Read for SteamFileReader {
    /// Reads the next bytes of the file into `buf`.
    ///
    /// Steam reports how many bytes it wrote, and that number is also the size of the copy
    /// `FileReadAsyncComplete` is about to make into the caller's buffer -- the parameter list is
    /// "void *pvBuffer, uint32 cubToRead" (`isteamremotestorage.h:206`) -- so the count goes through
    /// `written_bytes` before either the copy or the offset uses it. The bound is the number of bytes
    /// this side asked for, which is all the buffer can hold.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() || self.offset >= self.size {
            return Ok(0);
        }
        let len = min(buf.len(), self.size - self.offset);
        unsafe {
            let api_call = sys::SteamAPI_ISteamRemoteStorage_FileReadAsync(
                self.file.rs,
                self.file.name.as_ptr(),
                self.offset as _,
                len as _,
            );

            let mut failed = false;
            while !sys::SteamAPI_ISteamUtils_IsAPICallCompleted(
                self.file.util,
                api_call,
                &mut failed,
            ) {
                std::thread::yield_now();
            }
            if failed {
                return Err(std::io::ErrorKind::Other.into());
            }
            let mut callback: sys::RemoteStorageFileReadAsyncComplete_t = std::mem::zeroed();
            sys::SteamAPI_ISteamUtils_GetAPICallResult(
                self.file.util,
                api_call,
                (&mut callback) as *mut _ as *mut _,
                std::mem::size_of::<sys::RemoteStorageFileReadAsyncComplete_t>() as _,
                1332,
                &mut failed,
            );

            if to_steam_result(callback.m_eResult).is_err() {
                return Err(std::io::ErrorKind::Other.into());
            }
            let kept = written_bytes(callback.m_cubRead, len);
            sys::SteamAPI_ISteamRemoteStorage_FileReadAsyncComplete(
                self.file.rs,
                callback.m_hFileReadAsync,
                buf.as_mut_ptr().cast(),
                kept as _,
            );

            self.offset += kept;
            Ok(kept)
        }
    }
}

/// Where a seek lands, in the one reading `std::io::Seek` gives it.
///
/// `End(o)` is `size + o`, `Current(o)` is `offset + o`, `Start(o)` is `o`, and the position stays
/// addressable: not before the start, not past the end. `None` means `InvalidInput`.
///
/// The arithmetic used to be `(size - 1 - o)` for `End` -- the sign of `o` inverted and one subtracted
/// -- so `End(0)` landed a byte short of EOF and `End(-4)` landed *past* the end (`size + 3`), and a
/// negative result cast to `usize` became an enormous offset. The reader's own guard then computes
/// `size - offset`, which underflows on that value: a panic in a debug build, and in a release build a
/// length of ~1.8e19 handed to `FileReadAsync`. `Current(o)` with `o` negative enough walked the same
/// wrap. This version cannot: every branch is checked arithmetic against the same two bounds, and the
/// end itself is a legal position because reading there returns zero bytes, which is what EOF means.
fn seek_target(pos: std::io::SeekFrom, offset: usize, size: usize) -> Option<usize> {
    use std::io::SeekFrom;
    let target: i64 = match pos {
        SeekFrom::Start(o) => i64::try_from(o).ok()?,
        SeekFrom::Current(o) => i64::try_from(offset).ok()?.checked_add(o)?,
        SeekFrom::End(o) => i64::try_from(size).ok()?.checked_add(o)?,
    };
    if target < 0 {
        return None;
    }
    let target = usize::try_from(target).ok()?;
    (target <= size).then_some(target)
}

impl std::io::Seek for SteamFileReader {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        let target = seek_target(pos, self.offset, self.size)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        self.offset = target;
        Ok(self.offset as u64)
    }
}

/// Name and size information about a file in the steam cloud
#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct SteamFileInfo {
    /// The file name
    pub name: String,
    /// The size of the file in bytes
    pub size: u64,
}

#[cfg(test)]
mod files_tests {
    //! Pure half of `Seek`: where a seek lands. The reader itself needs a Steam file, but the
    //! arithmetic does not, so it is judged here instead of registered as uncovered.
    use super::seek_target;
    use std::io::SeekFrom;

    const SIZE: usize = 100;

    /// `End(o)` is `size + o`. The old arithmetic answered `size - 1 - o`, which inverted the sign of
    /// `o` and subtracted one: `End(0)` landed a byte short of EOF and `End(-4)` landed past the end.
    #[test]
    fn end_is_relative_to_the_end_not_mirrored_around_it() {
        assert_eq!(seek_target(SeekFrom::End(0), 0, SIZE), Some(100));
        assert_eq!(seek_target(SeekFrom::End(-4), 0, SIZE), Some(96));
        assert_eq!(seek_target(SeekFrom::End(-100), 0, SIZE), Some(0));
        assert_eq!(seek_target(SeekFrom::End(4), 0, SIZE), None);
        assert_eq!(seek_target(SeekFrom::End(-101), 0, SIZE), None);
    }

    #[test]
    fn start_and_current_land_where_the_trait_says() {
        assert_eq!(seek_target(SeekFrom::Start(30), 0, SIZE), Some(30));
        // The end itself is addressable: reading there returns zero bytes, which is EOF.
        assert_eq!(seek_target(SeekFrom::Start(100), 0, SIZE), Some(100));
        assert_eq!(seek_target(SeekFrom::Start(101), 0, SIZE), None);
        assert_eq!(seek_target(SeekFrom::Current(10), 60, SIZE), Some(70));
        assert_eq!(seek_target(SeekFrom::Current(-60), 60, SIZE), Some(0));
        assert_eq!(seek_target(SeekFrom::Current(-61), 60, SIZE), None);
        assert_eq!(seek_target(SeekFrom::Current(50), 60, SIZE), None);
    }

    /// Before this, `Current(o)` and `End(o)` were cast through `isize`/`usize` and a negative result
    /// wrapped to ~1.8e19 rather than failing -- and the reader then computed `size - offset` on that
    /// value.
    #[test]
    fn extremes_do_not_wrap_or_panic() {
        assert_eq!(seek_target(SeekFrom::Start(u64::MAX), 0, SIZE), None);
        assert_eq!(seek_target(SeekFrom::End(i64::MAX), 0, SIZE), None);
        assert_eq!(seek_target(SeekFrom::End(i64::MIN), 0, SIZE), None);
        assert_eq!(seek_target(SeekFrom::Current(i64::MAX), 0, SIZE), None);
        assert_eq!(seek_target(SeekFrom::Current(i64::MIN), 0, SIZE), None);
        assert_eq!(seek_target(SeekFrom::Start(0), 0, 0), Some(0));
        assert_eq!(seek_target(SeekFrom::Current(1), 0, 0), None);
    }

    #[test]
    fn every_accepted_position_is_addressable() {
        for delta in -140i64..140 {
            for (kind, at) in [
                (SeekFrom::End(delta), 0usize),
                (SeekFrom::Current(delta), 40usize),
            ] {
                if let Some(target) = seek_target(kind, at, SIZE) {
                    assert!(
                        target <= SIZE,
                        "{kind:?} at {at} accepted {target}, past the end"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod stream_tests {
    //! The three numbers the stream writer and reader hand to or take from Steam, and the one
    //! lifecycle decision that drops a call. The FFI itself needs a client: whether Steam accepts a
    //! cancel, and what it does to a stream afterwards, is registered as unverified.

    use super::{chunk_fit, closes_on_drop, reader_size};
    use std::os::raw::c_int;

    /// A buffer Steam cannot size in one `int32` is written in part, which `std::io::Write` allows,
    /// rather than through a cast that would name a negative length.
    #[test]
    fn a_chunk_steam_cannot_size_is_cut_to_what_it_can() {
        assert_eq!(chunk_fit(0), 0);
        assert_eq!(chunk_fit(4096), 4096);
        assert_eq!(chunk_fit(c_int::MAX as usize), c_int::MAX as usize);
        assert_eq!(chunk_fit(c_int::MAX as usize + 1), c_int::MAX as usize);
        assert_ne!(chunk_fit(usize::MAX), usize::MAX);
    }

    /// Steam's `int32` size arrives without a declared sentinel, so a negative reads as "no size",
    /// never as the 1.8e18 that a bare `as usize` would have produced.
    #[test]
    fn a_negative_size_is_zero_and_not_an_enormous_file() {
        assert_eq!(reader_size(0), 0);
        assert_eq!(reader_size(1024), 1024);
        assert_eq!(reader_size(-1), 0);
        assert_eq!(reader_size(c_int::MIN), 0);
        assert_ne!(reader_size(-1), usize::MAX);
    }

    /// Closing after a successful cancel would ask Steam about a stream it already threw away;
    /// skipping the close after a refused one would leak the stream. The direction matters.
    #[test]
    fn only_a_cancelled_stream_skips_its_close() {
        assert!(!closes_on_drop(true));
        assert!(closes_on_drop(false));
    }
}
