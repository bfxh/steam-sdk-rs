//! Files as handles: `SteamFile` metadata and access, the `std::io` writer and reader pairs
//! (with their `Seek`), the batch guard that ends a Steam write batch on drop, and the info
//! record Steam fills in.

use super::*;
use crate::utils::written_bytes;

/// Ends the Steam write batch on drop.
pub struct FileWriteBatch {
    // `pub(super)`: `begin_write_batch` in `api.rs` is the only thing that ever builds the guard.
    pub(super) rs: *mut sys::ISteamRemoteStorage,
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
            SteamFileWriter { file: self, handle }
        }
    }

    pub fn read(self) -> SteamFileReader {
        unsafe {
            SteamFileReader {
                offset: 0,
                size: sys::SteamAPI_ISteamRemoteStorage_GetFileSize(self.rs, self.name.as_ptr())
                    as usize,
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
}

impl std::io::Write for SteamFileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        unsafe {
            if sys::SteamAPI_ISteamRemoteStorage_FileWriteStreamWriteChunk(
                self.file.rs,
                self.handle,
                buf.as_ptr().cast(),
                buf.len() as _,
            ) {
                Ok(buf.len())
            } else {
                Err(std::io::ErrorKind::Other.into())
            }
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for SteamFileWriter {
    fn drop(&mut self) {
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
        use std::cmp::min;
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
