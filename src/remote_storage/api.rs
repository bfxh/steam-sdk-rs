//! `RemoteStorage`'s own methods -- quota, enumeration, the one-shot `read_file` and
//! `write_file`, the legacy Workshop surface it still forwards, and the file-exists/query
//! calls. Everything that returns a handle or a stream lives in `files`.

use super::*;

/// How many cloud files the enumeration may reserve for.
///
/// `GetFileCount` is declared "virtual int32 GetFileCount() = 0;" (`isteamremotestorage.h:228`): a
/// signed count. `files` used to compare it against -1 literally and cast everything else straight
/// into `Vec::with_capacity(count as usize)`, so any other negative answer -- -2, `INT_MIN` -- became
/// ~1.8e19 elements and the allocation aborts the caller's thread. The sibling enumeration in this
/// same file, `local_file_changes`, already asked `count <= 0` of the identically signed
/// "virtual int32 GetLocalFileChangeCount() = 0;" (`:315`); both now go through one guard, so the
/// asymmetry cannot come back.
fn file_count(raw: ::std::os::raw::c_int) -> usize {
    usize::try_from(raw).unwrap_or(0)
}

impl RemoteStorage {
    /// Toggles whether the steam cloud is enabled for the application
    pub fn set_cloud_enabled_for_app(&self, enabled: bool) {
        unsafe {
            sys::SteamAPI_ISteamRemoteStorage_SetCloudEnabledForApp(self.rs, enabled);
        }
    }

    /// Returns whether the steam cloud is enabled for the application
    ///
    /// # Note
    ///
    /// This is independent from the account wide setting
    pub fn is_cloud_enabled_for_app(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_IsCloudEnabledForApp(self.rs) }
    }

    /// Returns whether the steam cloud is enabled for the account
    ///
    /// # Note
    ///
    /// This is independent from the application setting
    pub fn is_cloud_enabled_for_account(&self) -> bool {
        unsafe { sys::SteamAPI_ISteamRemoteStorage_IsCloudEnabledForAccount(self.rs) }
    }

    /// Returns information about all files in the cloud storage
    pub fn files(&self) -> Vec<SteamFileInfo> {
        unsafe {
            let count = file_count(sys::SteamAPI_ISteamRemoteStorage_GetFileCount(self.rs));
            if count == 0 {
                return Vec::new();
            }
            let mut files = Vec::with_capacity(count);
            for idx in 0..count {
                let mut size = 0;
                let name = CStr::from_ptr(sys::SteamAPI_ISteamRemoteStorage_GetFileNameAndSize(
                    self.rs, idx as _, &mut size,
                ));
                files.push(SteamFileInfo {
                    name: name.to_string_lossy().into_owned(),
                    size: size as u64,
                })
            }

            files
        }
    }

    /// Returns a handle to a steam cloud file
    ///
    /// The file does not have to exist.
    pub fn file(&self, name: &str) -> SteamFile {
        SteamFile {
            rs: self.rs,
            util: self.util,
            _inner: self.inner.clone(),
            name: CString::new(name).unwrap(),
        }
    }

    /// Writes a whole file in one synchronous call.
    ///
    /// Where [`SteamFile::write`] streams through Steam, this is the one-shot
    /// form Steam documents for small files. The FFI length parameter is an
    /// int32, so an input that cannot be expressed in it is rejected rather
    /// than silently truncated.
    pub fn write_file(&self, name: &str, data: &[u8]) -> Result<(), SteamError> {
        let file = cloud_file_name(name)?;
        let len = cloud_chunk_len(data.len())?;
        // Steam takes a payload pointer even for a zero length write. Handing
        // it null is not something we can show is safe, so an empty write gets
        // an aligned, never-dereferenced address instead.
        let data_ptr: *const c_void = if data.is_empty() {
            std::ptr::NonNull::<u8>::dangling().as_ptr() as *const c_void
        } else {
            data.as_ptr() as *const c_void
        };
        let ok = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_FileWrite(self.rs, file.as_ptr(), data_ptr, len)
        };
        if ok {
            Ok(())
        } else {
            Err(SteamError::IOFailure)
        }
    }

    /// Reads up to `max_bytes` of a cloud file in one synchronous call, and
    /// returns what was actually read.
    ///
    /// Steam reports a missing file, a buffer that was too small and a failed
    /// read all as a negative count, so that is surfaced as
    /// [`SteamError::FileNotFound`] rather than as an empty file.
    pub fn read_file(&self, name: &str, max_bytes: usize) -> Result<Vec<u8>, SteamError> {
        let file = cloud_file_name(name)?;
        let to_read = cloud_chunk_len(max_bytes)?;
        let mut buf = vec![0u8; max_bytes];
        let read = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_FileRead(
                self.rs,
                file.as_ptr(),
                buf.as_mut_ptr() as *mut c_void,
                to_read,
            )
        };
        if read < 0 {
            return Err(SteamError::FileNotFound);
        }
        buf.truncate(read as usize);
        Ok(buf)
    }

    /// Cloud storage quota as `(total_bytes, available_bytes)`.
    ///
    /// `None` when Steam cannot answer, which is what it does while cloud saves
    /// are off for this app -- so `None` is not "zero bytes left".
    pub fn quota(&self) -> Option<(u64, u64)> {
        let mut total = 0u64;
        let mut available = 0u64;
        let ok = unsafe {
            sys::SteamAPI_ISteamRemoteStorage_GetQuota(self.rs, &mut total, &mut available)
        };
        ok.then_some((total, available))
    }

    /// Cloud files that changed locally since the previous call.
    ///
    /// Steam hands back its own buffer for each name, so every name is copied
    /// out before the next query.
    pub fn local_file_changes(&self) -> Vec<LocalFileChange> {
        let count = file_count(unsafe {
            sys::SteamAPI_ISteamRemoteStorage_GetLocalFileChangeCount(self.rs)
        });
        if count == 0 {
            return Vec::new();
        }
        let mut changes = Vec::with_capacity(count);
        for index in 0..count {
            let mut change =
                sys::ERemoteStorageLocalFileChange::k_ERemoteStorageLocalFileChange_Invalid;
            let mut path_kind =
                sys::ERemoteStorageFilePathType::k_ERemoteStorageFilePathType_Invalid;
            let raw = unsafe {
                sys::SteamAPI_ISteamRemoteStorage_GetLocalFileChange(
                    self.rs,
                    index as _,
                    &mut change,
                    &mut path_kind,
                )
            };
            let path = if raw.is_null() {
                String::new()
            } else {
                unsafe { CStr::from_ptr(raw).to_string_lossy().into_owned() }
            };
            changes.push(LocalFileChange {
                path,
                change: change.into(),
                path_kind: path_kind.into(),
            });
        }
        changes
    }

    /// Starts a group of file writes that Steam syncs once when the returned
    /// guard is dropped.
    ///
    /// Fails when Steam refuses the batch, so a caller cannot mistake a refused
    /// batch for a fast one.
    pub fn begin_write_batch(&self) -> Result<FileWriteBatch, SteamError> {
        if unsafe { sys::SteamAPI_ISteamRemoteStorage_BeginFileWriteBatch(self.rs) } {
            Ok(FileWriteBatch {
                rs: self.rs,
                _inner: self.inner.clone(),
            })
        } else {
            Err(SteamError::IOFailure)
        }
    }
}

#[cfg(test)]
mod tests {
    //! The count guard, judged without a client. Both enumeration calls need Steam to produce a
    //! count, but the decision taken *from* the count is pure, and that decision is what used to abort.
    use super::file_count;

    /// The old code singled out -1 and cast everything else, so -2 or `INT_MIN` reached
    /// `Vec::with_capacity(count as usize)` and became ~1.8e19 elements -- an abort inside a safe
    /// method, from a value the caller never supplied.
    #[test]
    fn any_negative_count_enumerates_nothing() {
        assert_eq!(file_count(-1), 0);
        assert_eq!(file_count(-2), 0);
        assert_eq!(file_count(i32::MIN), 0);
    }

    /// The other half: a real count is not rounded, narrowed or re-interpreted on the way through.
    #[test]
    fn a_real_count_survives_the_guard_unchanged() {
        assert_eq!(file_count(0), 0);
        assert_eq!(file_count(1), 1);
        assert_eq!(file_count(37), 37);
        assert_eq!(file_count(i32::MAX), usize::try_from(i32::MAX).unwrap());
    }
}
